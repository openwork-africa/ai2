use super::{Harness, Job, Model};
use futures_util::Stream;
use harness::job::{self, Event};
use harness::state::{is_connected, on_path, workspace_path, State, States};
use harness::{Registry, Setup};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::sync::watch;

const INSTALL: Duration = Duration::from_secs(20 * 60);
const SIGN_IN: Duration = Duration::from_secs(15 * 60);
const KEPT: usize = 5000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Install,
    SignIn,
}

impl Action {
    fn name(self) -> &'static str {
        match self {
            Action::Install => "install",
            Action::SignIn => "sign-in",
        }
    }
}

pub struct Refused {
    pub status: u16,
    pub code: &'static str,
    pub message: String,
}

fn refused(status: u16, code: &'static str, message: String) -> Refused {
    Refused {
        status,
        code,
        message,
    }
}

pub struct Harnesses {
    setups: Vec<Setup>,
    home: PathBuf,
    state: PathBuf,
    jobs: Mutex<Jobs>,
}

#[derive(Default)]
struct Jobs {
    next: u64,
    all: HashMap<String, Arc<Run>>,
    latest: HashMap<String, String>,
}

pub struct Run {
    id: String,
    harness: String,
    action: Action,
    log: Mutex<Log>,
    changed: watch::Sender<usize>,
    input: Mutex<Option<Sender<String>>>,
}

#[derive(Default)]
struct Log {
    base: usize,
    events: VecDeque<Event>,
    ended: Option<(bool, Option<String>)>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
}

fn asks(hint: Option<&str>) -> Option<String> {
    let hint = hint?;
    if hint.contains("API key") {
        Some("key".into())
    } else if hint.to_lowercase().contains("paste") {
        Some("code".into())
    } else {
        None
    }
}

impl Harnesses {
    pub fn new(registry: &Registry, home: PathBuf, state: PathBuf) -> Self {
        Self {
            setups: registry.all().into_iter().cloned().collect(),
            home,
            state,
            jobs: Mutex::default(),
        }
    }

    pub fn public() -> Self {
        let state = std::env::var_os("AI2_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir().join(".ai2"));
        Self::new(&Registry::public(), home_dir(), state)
    }

    fn path(&self) -> String {
        workspace_path(&self.home, &std::env::var("PATH").unwrap_or_default())
    }

    fn setup(&self, id: &str) -> Option<&Setup> {
        self.setups.iter().find(|setup| setup.id == id)
    }

    pub fn list(&self) -> Vec<Harness> {
        let path = self.path();
        let states = States::at(&self.state);
        let jobs = lock(&self.jobs);
        self.setups
            .iter()
            .map(|setup| {
                let known = states.of(&setup.id);
                let state = State {
                    installed: known.installed || on_path(&setup.command, &path),
                    ..known
                };
                Harness {
                    id: setup.id.clone(),
                    label: setup.label.clone(),
                    kind: if setup.is_agent() { "agent" } else { "tool" }.into(),
                    command: setup.command.clone(),
                    logo: setup.logo.clone(),
                    can_sign_in: setup.sign_in.is_some(),
                    sign_in_hint: setup.sign_in_hint.clone(),
                    asks: asks(setup.sign_in_hint.as_deref()),
                    models: setup
                        .models()
                        .iter()
                        .map(|model| Model {
                            id: model.id.clone(),
                            label: model.label.clone(),
                        })
                        .collect(),
                    installed: state.installed,
                    signed_in: state.signed_in,
                    connected: is_connected(setup, state),
                    job: jobs
                        .latest
                        .get(&setup.id)
                        .and_then(|id| jobs.all.get(id))
                        .map(|run| run.summary()),
                }
            })
            .collect()
    }

    pub fn job(&self, id: &str) -> Option<Arc<Run>> {
        lock(&self.jobs).all.get(id).cloned()
    }

    pub fn start(self: &Arc<Self>, id: &str, action: Action) -> Result<Job, Refused> {
        let setup = self
            .setup(id)
            .ok_or_else(|| {
                refused(
                    404,
                    "not_found",
                    format!("There is no harness called {id}."),
                )
            })?
            .clone();
        let line = match action {
            Action::Install => setup.install.clone(),
            Action::SignIn => setup.sign_in.clone().ok_or_else(|| {
                refused(
                    409,
                    "signs_in_on_first_run",
                    format!("{} signs in the first time it runs.", setup.label),
                )
            })?,
        };
        let (to, from) = channel();
        let run = {
            let mut jobs = lock(&self.jobs);
            let busy = jobs
                .latest
                .get(id)
                .and_then(|job| jobs.all.get(job))
                .is_some_and(|run| run.running());
            if busy {
                return Err(refused(
                    409,
                    "already_running",
                    format!("{} is already installing or signing in.", setup.label),
                ));
            }
            jobs.next += 1;
            let run = Arc::new(Run {
                id: format!("{}-{}", setup.id, jobs.next),
                harness: setup.id.clone(),
                action,
                log: Mutex::default(),
                changed: watch::channel(0).0,
                input: Mutex::new(Some(to)),
            });
            jobs.all.insert(run.id.clone(), run.clone());
            jobs.latest.insert(setup.id.clone(), run.id.clone());
            run
        };
        let summary = run.summary();
        let harnesses = self.clone();
        std::thread::spawn(move || {
            let limit = match action {
                Action::Install => INSTALL,
                Action::SignIn => SIGN_IN,
            };
            let ended = job::run(&line, &harnesses.path(), limit, from.into_iter(), |event| {
                run.push(event.clone())
            });
            let (ok, message) = match ended {
                Ok(ended) => (ended.ok, ended.message),
                Err(failed) => {
                    let text = format!("{failed:#}");
                    run.push(Event::Error {
                        error: text.clone(),
                    });
                    (false, Some(text))
                }
            };
            if ok {
                harnesses.remember(&setup, action);
            }
            run.finish(ok, message);
        });
        Ok(summary)
    }

    fn remember(&self, setup: &Setup, action: Action) {
        let states = States::at(&self.state);
        let before = states.of(&setup.id);
        let _ = states.set(
            &setup.id,
            State {
                installed: true,
                signed_in: action == Action::SignIn || before.signed_in,
            },
        );
    }
}

impl Run {
    fn running(&self) -> bool {
        lock(&self.log).ended.is_none()
    }

    fn summary(&self) -> Job {
        let log = lock(&self.log);
        Job {
            id: self.id.clone(),
            harness: self.harness.clone(),
            action: self.action.name().into(),
            running: log.ended.is_none(),
            ok: log.ended.as_ref().map(|(ok, _)| *ok),
            message: log.ended.as_ref().and_then(|(_, message)| message.clone()),
        }
    }

    fn push(&self, event: Event) {
        let count = {
            let mut log = lock(&self.log);
            log.events.push_back(event);
            if log.events.len() > KEPT {
                log.events.pop_front();
                log.base += 1;
            }
            log.base + log.events.len()
        };
        self.changed.send_replace(count);
    }

    fn finish(&self, ok: bool, message: Option<String>) {
        lock(&self.input).take();
        let count = {
            let mut log = lock(&self.log);
            log.events.push_back(Event::Done {
                ok,
                message: message.clone(),
            });
            log.ended = Some((ok, message));
            log.base + log.events.len()
        };
        self.changed.send_replace(count);
    }

    pub fn answer(&self, text: String) -> bool {
        match lock(&self.input).as_ref() {
            Some(to) => to.send(text).is_ok(),
            None => false,
        }
    }

    fn since(&self, next: usize) -> (Vec<(usize, Event)>, bool) {
        let log = lock(&self.log);
        let from = next.max(log.base);
        let events = log
            .events
            .iter()
            .enumerate()
            .skip(from - log.base)
            .map(|(at, event)| (log.base + at, event.clone()))
            .collect();
        (events, log.ended.is_some())
    }

    pub fn stream(self: Arc<Self>, after: usize) -> impl Stream<Item = (usize, Event)> {
        let changed = self.changed.subscribe();
        let start = (self, changed, after, VecDeque::new(), false);
        futures_util::stream::unfold(
            start,
            |(run, mut changed, mut next, mut pending, mut ended)| async move {
                loop {
                    if let Some(item) = pending.pop_front() {
                        return Some((item, (run, changed, next, pending, ended)));
                    }
                    if ended {
                        return None;
                    }
                    changed.borrow_and_update();
                    let (events, done) = run.since(next);
                    if let Some((last, _)) = events.last() {
                        next = last + 1;
                    }
                    pending.extend(events);
                    ended = done;
                    if pending.is_empty() && !ended && changed.changed().await.is_err() {
                        ended = true;
                    }
                }
            },
        )
    }
}
