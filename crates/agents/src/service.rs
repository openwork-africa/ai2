use std::fmt;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;

use anyhow::Context as _;
use harness::{task_line, Registry};
use serde_json::Value;

use crate::definition::{clean_harness, define, folder, Agent, NewAgent};
use crate::enhance::{enhance, is_short_seed, Outcome, Personality};
use crate::error::{Problem, Result};
use crate::model::{Enhanced, Finished, RunAsk};
use crate::runs::{new_id, FolderLock, Run, RunEvent, RunState, Runs, Stream};
use crate::store::{AgentStore, LocalStore};
use crate::time::stamp;

pub type Ask = Arc<dyn Fn(&str, &str) -> anyhow::Result<String> + Send + Sync>;
pub type Enhancer = Arc<dyn Fn(Option<&str>) -> anyhow::Result<Ask> + Send + Sync>;
pub type Registries = Arc<dyn Fn() -> Registry + Send + Sync>;

#[derive(Debug)]
pub struct Reviewed {
    pub hash: String,
    pub review: Value,
}

pub trait Review: Send + Sync {
    fn review(
        &self,
        folder: &Path,
        objective: &str,
        say: &mut dyn FnMut(String),
    ) -> anyhow::Result<Reviewed>;
}

pub struct Prepared {
    run: Run,
    line: String,
    folder: PathBuf,
    review: bool,
}

impl Prepared {
    pub fn run(&self) -> &Run {
        &self.run
    }

    pub fn line(&self) -> &str {
        &self.line
    }
}

#[derive(Clone)]
pub struct Agents {
    store: Arc<dyn AgentStore>,
    registry: Registries,
    runs: Runs,
    path: Option<String>,
    enhancer: Option<Enhancer>,
    review: Option<Arc<dyn Review>>,
}

impl fmt::Debug for Agents {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Agents")
            .field("runs", &self.runs.root())
            .field("enhancer", &self.enhancer.is_some())
            .field("review", &self.review.is_some())
            .finish_non_exhaustive()
    }
}

impl Agents {
    pub fn new(
        store: Arc<dyn AgentStore>,
        registry: impl Fn() -> Registry + Send + Sync + 'static,
        ai2_home: &Path,
    ) -> Self {
        Self {
            store,
            registry: Arc::new(registry),
            runs: Runs::new(ai2_home),
            path: None,
            enhancer: None,
            review: None,
        }
    }

    pub fn local(ai2_home: &Path, registry: impl Fn() -> Registry + Send + Sync + 'static) -> Self {
        Self::new(Arc::new(LocalStore::new(ai2_home)), registry, ai2_home)
    }

    pub fn with_path(mut self, path: String) -> Self {
        self.path = Some(path);
        self
    }

    pub fn with_enhancer(mut self, enhancer: Enhancer) -> Self {
        self.enhancer = Some(enhancer);
        self
    }

    pub fn with_review(mut self, review: Arc<dyn Review>) -> Self {
        self.review = Some(review);
        self
    }

    pub fn store(&self) -> &dyn AgentStore {
        self.store.as_ref()
    }

    pub fn runs(&self) -> &Runs {
        &self.runs
    }

    pub fn reviews(&self) -> bool {
        self.review.is_some()
    }

    pub fn list(&self) -> Result<Vec<Agent>> {
        Ok(self.store.list()?)
    }

    pub fn get(&self, name: &str) -> Result<Agent> {
        self.store.get(name)?.ok_or_else(|| Problem::no_agent(name))
    }

    pub fn remove(&self, name: &str) -> Result<()> {
        if self.store.remove(name)? {
            Ok(())
        } else {
            Err(Problem::no_agent(name))
        }
    }

    pub fn create(&self, new: &NewAgent, enhancer: Option<&str>) -> Result<Agent> {
        let before = self.store.get(new.name.trim()).ok().flatten();
        let mut agent = define(&(self.registry)(), new, &stamp(), before.as_ref())?;
        self.store.put(&agent)?;
        let short = agent
            .personality
            .as_ref()
            .is_some_and(|personality| is_short_seed(&personality.content));
        if let (true, Some(ask)) = (short, self.ask(enhancer)) {
            let outcome = self.enhanced(agent.personality.as_ref(), &agent.name, Some(ask));
            apply(&mut agent, &outcome);
            self.store.put(&agent)?;
        }
        Ok(agent)
    }

    fn ask(&self, enhancer: Option<&str>) -> Option<Ask> {
        (self.enhancer.as_ref()?)(enhancer).ok()
    }

    fn enhanced(&self, personality: Option<&Personality>, name: &str, ask: Option<Ask>) -> Outcome {
        let model = ask.as_ref().map(|ask| ask.as_ref() as _);
        enhance(personality, name, model, &stamp())
    }

    pub fn enhance_words(&self, words: &str, name: &str, enhancer: Option<&str>) -> Outcome {
        self.enhanced(
            Some(&Personality::written(words.trim())),
            name,
            self.ask(enhancer),
        )
    }

    pub fn enhance(&self, name: &str, enhancer: Option<&str>) -> Result<Enhanced> {
        let agent = self.get(name)?;
        let outcome = self.enhanced(agent.personality.as_ref(), &agent.name, self.ask(enhancer));
        let mut still = self.get(name)?;
        let unchanged = still.personality.as_ref().map(|p| p.content.trim())
            == agent.personality.as_ref().map(|p| p.content.trim());
        if outcome == Outcome::Skipped || !unchanged {
            return Ok(Enhanced::of(&Outcome::Skipped, still));
        }
        apply(&mut still, &outcome);
        still.updated = stamp();
        self.store.put(&still)?;
        Ok(Enhanced::of(&outcome, still))
    }

    pub fn prepare(&self, ask: &RunAsk) -> Result<Prepared> {
        let prompt = ask.prompt.trim();
        if prompt.is_empty() {
            return Err(Problem::invalid(
                "prompt_required",
                "Say what the agent should do.",
            ));
        }
        let given = |value: &Option<String>| value.clone().filter(|text| !text.trim().is_empty());
        let agent = match (given(&ask.agent), given(&ask.harness)) {
            (Some(_), Some(_)) => {
                return Err(Problem::invalid(
                    "agent_or_harness",
                    "Send an agent or a harness, not both.",
                ))
            }
            (None, None) => {
                return Err(Problem::invalid(
                    "agent_or_harness",
                    "Send the agent to run, or a harness.",
                ))
            }
            (Some(name), None) => Some(self.get(name.trim())?),
            (None, Some(_)) => None,
        };
        let harness = agent.as_ref().map_or_else(
            || ask.harness.clone().unwrap_or_default(),
            |a| a.harness.clone(),
        );
        let model = given(&ask.model).or_else(|| agent.as_ref().and_then(|a| a.model.clone()));
        let personality = given(&ask.personality).or_else(|| {
            agent
                .as_ref()
                .and_then(|a| a.personality.as_ref())
                .map(|p| p.content.clone())
        });
        let registry = (self.registry)();
        let (harness, model) = clean_harness(&registry, &harness, model.as_deref())?;
        let setup = registry
            .agent(&harness)
            .ok_or_else(|| Problem::internal(format!("{harness} went away")))?;
        let place = given(&ask.folder)
            .or_else(|| agent.as_ref().and_then(|a| a.folder.clone()))
            .unwrap_or_default();
        let folder = folder(&place)?;
        let line = task_line(
            setup,
            prompt,
            &folder.display().to_string(),
            model.as_deref(),
            personality.as_deref(),
        );
        let id = new_id();
        let run = Run {
            log: self.runs.log(&id)?.display().to_string(),
            id,
            agent: agent.map(|a| a.name),
            harness,
            model,
            folder: folder.display().to_string(),
            prompt: prompt.to_string(),
            state: RunState::Running,
            started: stamp(),
            ended: None,
            exit: None,
            review: None,
            review_error: None,
            error: None,
        };
        self.runs.save(&run)?;
        Ok(Prepared {
            run,
            line,
            folder,
            review: !ask.no_review,
        })
    }

    pub fn execute(&self, prepared: Prepared, emit: &mut dyn FnMut(&RunEvent)) -> Result<Finished> {
        let Prepared {
            mut run,
            line,
            review: reviewing,
            folder,
        } = prepared;
        let runs = self.runs.clone();
        let id = run.id.clone();
        let mut send = |event: RunEvent| {
            let _ = runs.append(&id, &event);
            emit(&event);
        };
        let _folder = FolderLock::take(self.runs.root(), &folder, &mut || {
            send(RunEvent::Waiting {
                folder: folder.display().to_string(),
            })
        })?;
        send(RunEvent::Started { run: run.clone() });
        match self.harness(&line, &folder, Path::new(&run.log), &mut send) {
            Ok(exit) => run.exit = exit,
            Err(error) => run.error = Some(format!("{error:#}")),
        }
        run.ended = Some(stamp());
        send(RunEvent::Exited {
            exit: run.exit,
            error: run.error.clone(),
        });
        let mut review = None;
        if let Some(reviewer) = self.review.as_ref().filter(|_| reviewing) {
            run.state = RunState::Reviewing;
            self.runs.save(&run)?;
            let reviewed = reviewer.review(&folder, &run.prompt, &mut |text| {
                send(RunEvent::Review { text })
            });
            match reviewed {
                Ok(Reviewed {
                    hash,
                    review: found,
                }) => {
                    run.review = Some(hash);
                    review = Some(found);
                }
                Err(error) => run.review_error = Some(format!("{error:#}")),
            }
            send(RunEvent::Reviewed {
                hash: run.review.clone(),
                error: run.review_error.clone(),
            });
        }
        run.state = RunState::Ended;
        self.runs.save(&run)?;
        send(RunEvent::Done { run: run.clone() });
        Ok(Finished { run, review })
    }

    pub fn run(&self, ask: &RunAsk, emit: &mut dyn FnMut(&RunEvent)) -> Result<Finished> {
        let prepared = self.prepare(ask)?;
        self.execute(prepared, emit)
    }

    fn harness(
        &self,
        line: &str,
        folder: &Path,
        log: &Path,
        send: &mut dyn FnMut(RunEvent),
    ) -> anyhow::Result<Option<i32>> {
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg(line)
            .current_dir(folder)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(path) = &self.path {
            command.env("PATH", path);
        }
        let mut child = command
            .spawn()
            .with_context(|| format!("starting {line}"))?;
        let mut file =
            std::fs::File::create(log).with_context(|| format!("making {}", log.display()))?;
        let (to, from) = channel();
        pipe(
            child.stdout.take().context("no output")?,
            Stream::Out,
            to.clone(),
        );
        pipe(child.stderr.take().context("no errors")?, Stream::Err, to);
        for (stream, text) in from {
            let _ = writeln!(file, "{text}");
            send(RunEvent::Line { stream, text });
        }
        Ok(child.wait()?.code())
    }
}

fn pipe(output: impl Read + Send + 'static, stream: Stream, to: Sender<(Stream, String)>) {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(output);
        let mut line = Vec::new();
        while reader
            .read_until(b'\n', &mut line)
            .is_ok_and(|read| read > 0)
        {
            let text = String::from_utf8_lossy(&line)
                .trim_end_matches(['\n', '\r'])
                .to_string();
            line.clear();
            if to.send((stream, text)).is_err() {
                break;
            }
        }
    });
}

fn apply(agent: &mut Agent, outcome: &Outcome) {
    match outcome {
        Outcome::Enhanced {
            personality,
            description,
        } => {
            agent.personality = Some(personality.clone());
            if agent.description.is_none() {
                agent.description.clone_from(description);
            }
        }
        Outcome::Failed { personality, .. } => agent.personality = Some(personality.clone()),
        Outcome::Skipped => {}
    }
}
