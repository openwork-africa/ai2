use crate::error::{Problem, Result};
use crate::logs::{Follow, Logs, Sink};
use crate::model::{Api, ApiState, App, AppEvent, Exit, LogLine, Restart, Shown, Status, Stream};
use crate::openapi;
use crate::port;
use crate::process;
use crate::store::{Running, Store};
use crate::time;
use std::collections::{HashMap, VecDeque};
use std::os::unix::process::ExitStatusExt;
use std::process::Stdio;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::sync::{broadcast, watch};
use tokio::task::AbortHandle;

#[derive(Clone, Debug)]
pub struct Settings {
    pub supervise: bool,
    pub grace: Duration,
    pub port_wait: Duration,
    pub port_close: Duration,
    pub backoff: Duration,
    pub backoff_max: Duration,
    pub restart_window: Duration,
    pub restart_limit: usize,
    pub poll: Duration,
    pub recheck: Duration,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            supervise: true,
            grace: Duration::from_secs(5),
            port_wait: Duration::from_secs(30),
            port_close: Duration::from_secs(5),
            backoff: Duration::from_secs(1),
            backoff_max: Duration::from_secs(30),
            restart_window: Duration::from_secs(60),
            restart_limit: 5,
            poll: Duration::from_millis(250),
            recheck: Duration::from_secs(60),
        }
    }
}

impl Settings {
    pub fn detached() -> Settings {
        Settings {
            supervise: false,
            ..Settings::default()
        }
    }
}

struct Live {
    pid: u32,
    since: String,
    started_at: String,
    boot: String,
    adopted: bool,
    exited: watch::Receiver<Option<Exit>>,
    tasks: Vec<AbortHandle>,
}

#[derive(Default)]
struct Track {
    live: Option<Live>,
    generation: u64,
    restarts: u32,
    recent: VecDeque<Instant>,
    last_exit: Option<Exit>,
    crashed: bool,
    stopping: bool,
    starting: bool,
    pending: bool,
}

struct Inner {
    store: Store,
    settings: Settings,
    tracks: Mutex<HashMap<String, Track>>,
    logs: Mutex<HashMap<String, Arc<Logs>>>,
    events: broadcast::Sender<AppEvent>,
}

#[derive(Clone)]
pub struct Supervisor {
    inner: Arc<Inner>,
}

impl Supervisor {
    pub fn new(store: Store) -> Supervisor {
        Supervisor::with(store, Settings::default())
    }

    pub fn with(store: Store, settings: Settings) -> Supervisor {
        Supervisor {
            inner: Arc::new(Inner {
                store,
                settings,
                tracks: Mutex::new(HashMap::new()),
                logs: Mutex::new(HashMap::new()),
                events: broadcast::channel(256).0,
            }),
        }
    }

    pub fn store(&self) -> &Store {
        &self.inner.store
    }

    pub fn settings(&self) -> &Settings {
        &self.inner.settings
    }

    fn tracks(&self) -> MutexGuard<'_, HashMap<String, Track>> {
        self.inner.tracks.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn app_logs(&self, id: &str) -> Arc<Logs> {
        self.inner
            .logs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(id.to_string())
            .or_default()
            .clone()
    }

    pub fn events(&self) -> broadcast::Receiver<AppEvent> {
        self.inner.events.subscribe()
    }

    pub fn subscribe(&self, id: &str) -> broadcast::Receiver<(u64, LogLine)> {
        self.app_logs(id).subscribe()
    }

    pub fn replay(&self, id: &str, after: Option<u64>) -> Vec<(u64, LogLine)> {
        self.app_logs(id).since(after)
    }

    pub fn logs(&self, id: &str, tail: usize) -> Result<Vec<LogLine>> {
        self.store().app(id)?;
        self.store().tail(id, tail)
    }

    pub fn started_here(&self, id: &str) -> bool {
        self.tracks().get(id).is_some_and(|t| t.live.is_some())
    }

    fn emit(&self, id: &str, running: bool) {
        let (started_here, pid) = {
            let tracks = self.tracks();
            let live = tracks.get(id).and_then(|t| t.live.as_ref());
            (live.is_some(), live.map(|l| l.pid))
        };
        let _ = self.inner.events.send(AppEvent {
            id: id.to_string(),
            running,
            started_here,
            pid,
        });
    }

    pub async fn adopt(&self) -> Result<Vec<String>> {
        let mut adopted = Vec::new();
        for (id, record) in self.store().running()? {
            if self.started_here(&id) {
                continue;
            }
            if !process::same(record.pid, &record.started_at, &record.boot) {
                self.store().set_running(&id, None)?;
                continue;
            }
            let (tx, rx) = watch::channel(None);
            let generation = {
                let mut tracks = self.tracks();
                let track = tracks.entry(id.clone()).or_default();
                track.generation += 1;
                track.live = Some(Live {
                    pid: record.pid,
                    since: record.since.clone(),
                    started_at: record.started_at.clone(),
                    boot: record.boot.clone(),
                    adopted: true,
                    exited: rx,
                    tasks: Vec::new(),
                });
                track.generation
            };
            let tasks = vec![
                self.follow(&id),
                self.watch_adopted(&id, generation, &record, tx),
            ];
            match self.tracks().get_mut(&id).and_then(|t| t.live.as_mut()) {
                Some(live) => live.tasks = tasks,
                None => tasks.iter().for_each(AbortHandle::abort),
            }
            adopted.push(id);
        }
        Ok(adopted)
    }

    fn watch_adopted(
        &self,
        id: &str,
        generation: u64,
        record: &Running,
        tx: watch::Sender<Option<Exit>>,
    ) -> AbortHandle {
        let me = self.clone();
        let id = id.to_string();
        let record = record.clone();
        let poll = self.settings().poll;
        tokio::spawn(async move {
            while process::same(record.pid, &record.started_at, &record.boot) {
                tokio::time::sleep(poll).await;
            }
            let exit = Exit {
                code: None,
                signal: None,
            };
            me.ended(&id, generation, exit, false);
            let _ = tx.send(Some(exit));
        })
        .abort_handle()
    }

    fn follow(&self, id: &str) -> AbortHandle {
        let logs = self.app_logs(id);
        let mut follow = Follow::from_end(&self.store().log_path(id));
        let poll = self.settings().poll;
        tokio::spawn(async move {
            loop {
                for line in follow.read() {
                    logs.push(Stream::Log, line);
                }
                tokio::time::sleep(poll).await;
            }
        })
        .abort_handle()
    }

    fn spawn(&self, app: &App, generation: u64) -> Result<Live> {
        let piped = self.settings().supervise && app.restart != Restart::Never;
        let mut file = self.store().mark_start(&app.id)?;
        let mut command = tokio::process::Command::new("/bin/sh");
        command
            .arg("-c")
            .arg(&app.command)
            .current_dir(&app.cwd)
            .envs(&app.env)
            .process_group(0)
            .stdin(Stdio::null())
            .kill_on_drop(false);
        if app.set_port {
            command.env("PORT", app.port.to_string());
        }
        if piped {
            command.stdout(Stdio::piped()).stderr(Stdio::piped());
        } else {
            command
                .stdout(Stdio::from(file.try_clone()?))
                .stderr(Stdio::from(file.try_clone()?));
        }
        let mut child = command.spawn().map_err(|error| {
            use std::io::Write;
            let _ = writeln!(file, "could not start: {error}");
            Problem::new(
                500,
                "spawn_failed",
                format!("Could not run \"{}\": {error}", app.command),
            )
        })?;
        let pid = child.id().unwrap_or_default();
        let started_at = process::start_time(pid).unwrap_or_default();
        let boot = process::boot_id();
        let since = time::stamp();
        let _ = self.store().set_running(
            &app.id,
            Some(Running {
                pid,
                started_at: started_at.clone(),
                boot: boot.clone(),
                since: since.clone(),
                piped,
            }),
        );
        let mut tasks = Vec::new();
        let mut pumps = Vec::new();
        if piped {
            let sink = Arc::new(Mutex::new(Sink::new(&self.store().log_path(&app.id), file)));
            let logs = self.app_logs(&app.id);
            if let Some(out) = child.stdout.take() {
                pumps.push(pump(out, Stream::Out, logs.clone(), sink.clone()));
            }
            if let Some(err) = child.stderr.take() {
                pumps.push(pump(err, Stream::Err, logs, sink));
            }
        } else {
            tasks.push(self.follow(&app.id));
        }
        let (tx, rx) = watch::channel(None);
        let me = self.clone();
        let id = app.id.clone();
        tokio::spawn(async move {
            let exit = match child.wait().await {
                Ok(status) => Exit {
                    code: status.code(),
                    signal: status.signal(),
                },
                Err(_) => Exit {
                    code: None,
                    signal: None,
                },
            };
            for done in pumps {
                let _ = tokio::time::timeout(Duration::from_secs(2), done).await;
            }
            let delay = me.ended(&id, generation, exit, piped);
            let _ = tx.send(Some(exit));
            if let Some(delay) = delay {
                me.respawn(id, generation, delay).await;
            }
        });
        Ok(Live {
            pid,
            since,
            started_at,
            boot,
            adopted: false,
            exited: rx,
            tasks,
        })
    }

    fn ended(&self, id: &str, generation: u64, exit: Exit, piped: bool) -> Option<Duration> {
        let delay = {
            let mut tracks = self.tracks();
            let track = tracks.get_mut(id)?;
            if track.generation != generation {
                return None;
            }
            if let Some(live) = track.live.take() {
                for task in live.tasks {
                    task.abort();
                }
            }
            track.last_exit = Some(exit);
            let stopping = std::mem::take(&mut track.stopping);
            let restart = match self.store().app(id).map(|app| app.restart) {
                Ok(Restart::Always) => true,
                Ok(Restart::OnFailure) => !exit.success(),
                _ => false,
            };
            if stopping || track.starting || !piped || !restart {
                None
            } else {
                self.backoff(track)
            }
        };
        let _ = self.store().set_running(id, None);
        self.emit(id, false);
        delay
    }

    fn backoff(&self, track: &mut Track) -> Option<Duration> {
        let now = Instant::now();
        let window = self.settings().restart_window;
        while track
            .recent
            .front()
            .is_some_and(|at| now.duration_since(*at) > window)
        {
            track.recent.pop_front();
        }
        if track.recent.len() >= self.settings().restart_limit {
            track.crashed = true;
            return None;
        }
        let delay = self
            .settings()
            .backoff
            .saturating_mul(1 << track.recent.len().min(16));
        track.recent.push_back(now);
        track.pending = true;
        Some(delay.min(self.settings().backoff_max))
    }

    async fn respawn(&self, id: String, generation: u64, delay: Duration) {
        tokio::time::sleep(delay).await;
        let next = {
            let mut tracks = self.tracks();
            let Some(track) = tracks.get_mut(&id) else {
                return;
            };
            if track.generation != generation || track.live.is_some() || !track.pending {
                return;
            }
            track.pending = false;
            track.generation += 1;
            track.restarts += 1;
            track.generation
        };
        let Ok(app) = self.store().app(&id) else {
            return;
        };
        match self.spawn(&app, next) {
            Ok(live) => {
                if let Some(track) = self.tracks().get_mut(&id) {
                    track.live = Some(live);
                }
                self.emit(&id, true);
            }
            Err(error) => {
                self.app_logs(&id).push(Stream::Err, error.message.clone());
                if let Some(track) = self.tracks().get_mut(&id) {
                    track.crashed = true;
                }
            }
        }
    }

    pub fn remove(&self, id: &str) -> Result<App> {
        let app = self.store().remove(id)?;
        self.store().set_running(id, None)?;
        if let Some(track) = self.tracks().remove(id) {
            if let Some(live) = track.live {
                live.tasks.iter().for_each(AbortHandle::abort);
            }
        }
        Ok(app)
    }

    pub async fn start(&self, id: &str) -> Result<Shown> {
        let app = self.store().app(id)?;
        if self.started_here(id) {
            return Err(Problem::new(
                409,
                "already_running",
                format!("{} is already running.", app.name),
            ));
        }
        if listening(app.port).await {
            return Err(Problem::new(
                409,
                "already_running",
                format!(
                    "Running (not started by ai2): something already listens on port {}.",
                    app.port
                ),
            ));
        }
        let generation = {
            let mut tracks = self.tracks();
            let track = tracks.entry(id.to_string()).or_default();
            track.generation += 1;
            track.starting = true;
            track.crashed = false;
            track.pending = false;
            track.recent.clear();
            track.generation
        };
        let live = match self.spawn(&app, generation) {
            Ok(live) => live,
            Err(error) => {
                if let Some(track) = self.tracks().get_mut(id) {
                    track.starting = false;
                }
                return Err(error);
            }
        };
        let mut exited = live.exited.clone();
        if let Some(track) = self.tracks().get_mut(id) {
            track.live = Some(live);
        }
        self.emit(id, false);
        let deadline = Instant::now() + self.settings().port_wait;
        let opened = loop {
            if listening(app.port).await {
                break true;
            }
            if exited.borrow_and_update().is_some() || Instant::now() >= deadline {
                break false;
            }
            let _ = tokio::time::timeout(self.settings().poll, exited.changed()).await;
        };
        if let Some(track) = self.tracks().get_mut(id) {
            track.starting = false;
        }
        if opened {
            self.emit(id, true);
            self.check(id).await?;
            return self.show(id).await;
        }
        let exit = *exited.borrow();
        if let Some(exit) = exit {
            let how = match (exit.code, exit.signal) {
                (Some(code), _) => format!("with code {code}"),
                (None, Some(signal)) => format!("on signal {signal}"),
                _ => String::new(),
            };
            return Err(Problem::new(
                500,
                "exited",
                format!(
                    "\"{}\" exited {how} before opening port {}; see {}",
                    app.command,
                    app.port,
                    self.store().log_path(id).display()
                ),
            )
            .with_lines(self.store().tail(id, 20)?));
        }
        Err(Problem::new(
            504,
            "port_closed",
            format!(
                "Port {} did not open within {} seconds; see {}",
                app.port,
                self.settings().port_wait.as_secs(),
                self.store().log_path(id).display()
            ),
        ))
    }

    pub async fn stop(&self, id: &str) -> Result<Shown> {
        let app = self.store().app(id)?;
        let target = {
            let mut tracks = self.tracks();
            let track = tracks.entry(id.to_string()).or_default();
            match &track.live {
                Some(live) => Some((
                    live.pid,
                    live.adopted,
                    live.started_at.clone(),
                    live.boot.clone(),
                    live.exited.clone(),
                )),
                None if track.pending || track.crashed => {
                    track.generation += 1;
                    track.pending = false;
                    None
                }
                None => {
                    return Err(Problem::new(
                        409,
                        "not_ours",
                        format!("{} was not started by ai2, so it is not stopped.", app.name),
                    ))
                }
            }
        };
        let Some((pid, adopted, started_at, boot, mut exited)) = target else {
            return self.show(id).await;
        };
        if adopted && !process::same(pid, &started_at, &boot) {
            return Err(Problem::new(
                409,
                "not_ours",
                format!("{} is no longer the process ai2 started.", app.name),
            ));
        }
        if let Some(track) = self.tracks().get_mut(id) {
            track.stopping = true;
            track.pending = false;
        }
        process::signal_group(pid, libc::SIGTERM);
        let grace = self.settings().grace;
        let ended = tokio::time::timeout(grace, exited.wait_for(Option::is_some))
            .await
            .is_ok();
        if !ended && (!adopted || process::same(pid, &started_at, &boot)) {
            process::signal_group(pid, libc::SIGKILL);
            let _ = tokio::time::timeout(grace, exited.wait_for(Option::is_some)).await;
        }
        let deadline = Instant::now() + self.settings().port_close;
        while Instant::now() < deadline && listening(app.port).await {
            tokio::time::sleep(self.settings().poll).await;
        }
        self.show(id).await
    }

    pub async fn restart(&self, id: &str) -> Result<Shown> {
        if self.started_here(id) {
            self.stop(id).await?;
        }
        self.start(id).await
    }

    pub fn status_of(&self, id: &str, running: bool) -> Status {
        let tracks = self.tracks();
        let Some(track) = tracks.get(id) else {
            return Status {
                running,
                ..Status::default()
            };
        };
        Status {
            running,
            started_here: track.live.is_some(),
            pid: track.live.as_ref().map(|l| l.pid),
            since: track.live.as_ref().map(|l| l.since.clone()),
            restarts: track.restarts,
            last_exit: track.last_exit,
            crashed: track.crashed,
        }
    }

    pub async fn status(&self, id: &str) -> Result<Status> {
        let app = self.store().app(id)?;
        Ok(self.status_of(id, listening(app.port).await))
    }

    pub async fn show(&self, id: &str) -> Result<Shown> {
        let app = self.store().app(id)?;
        Ok(self.shown(app, false).await)
    }

    async fn shown(&self, app: App, document: bool) -> Shown {
        let running = listening(app.port).await;
        let status = self.status_of(&app.id, running);
        let api = self.store().api(&app);
        let document = document.then(|| self.store().document(&app.id)).flatten();
        Shown {
            app,
            status,
            api,
            document,
        }
    }

    pub async fn list(&self, include_openapi: bool) -> Result<Vec<Shown>> {
        self.adopt().await?;
        let mut out = Vec::new();
        for app in self.store().apps()? {
            let mut shown = self.shown(app, include_openapi).await;
            if shown.status.running && self.stale(&shown.app.id, &shown.api) {
                shown.api = self.check(&shown.app.id).await?;
                if include_openapi {
                    shown.document = self.store().document(&shown.app.id);
                }
            }
            out.push(shown);
        }
        Ok(out)
    }

    fn stale(&self, id: &str, api: &Api) -> bool {
        api.state == ApiState::Unchecked
            || self
                .store()
                .api_age(id)
                .is_none_or(|age| age > self.settings().recheck)
    }

    pub async fn check(&self, id: &str) -> Result<Api> {
        let app = self.store().app(id)?;
        let store = self.store().clone();
        tokio::task::spawn_blocking(move || openapi::check(&store, &app))
            .await
            .map_err(|error| Problem::internal(error.to_string()))?
    }
}

pub async fn listening(port: u16) -> bool {
    tokio::task::spawn_blocking(move || port::listening(port))
        .await
        .unwrap_or(false)
}

fn pump(
    pipe: impl AsyncRead + Unpin + Send + 'static,
    stream: Stream,
    logs: Arc<Logs>,
    sink: Arc<Mutex<Sink>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut lines = BufReader::new(pipe).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            sink.lock().unwrap_or_else(|e| e.into_inner()).write(&line);
            logs.push(stream, line);
        }
    })
}
