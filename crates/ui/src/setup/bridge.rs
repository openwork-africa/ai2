use super::Engine;
use ::setup::host::Host;
use ::setup::State as StepState;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::VecDeque;
use std::sync::{mpsc, Arc, Mutex, MutexGuard};
use std::time::Instant;
use tokio::sync::broadcast;
use utoipa::ToSchema;
use zeroize::Zeroizing;

pub const RING: usize = 5000;
pub const STEP_LOG: usize = 2000;
pub const CANCELLED: &str = "setup was cancelled in the browser";

#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub id: u64,
    pub kind: &'static str,
    pub data: Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Idle,
    Checking,
    Running,
    Done,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct Line {
    pub t: u64,
    pub stream: &'static str,
    pub line: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct StepView {
    pub id: String,
    pub status: String,
    pub warnings: Vec<String>,
    #[schema(value_type = Object)]
    pub info: Map<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ms: Option<u64>,
    #[schema(value_type = Vec<Line>)]
    pub log: VecDeque<Line>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Field {
    pub label: String,
    pub secret: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Input {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
    pub kind: String,
    pub title: String,
    pub why: String,
    pub fields: Vec<Field>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct Warning {
    pub step: String,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct Snapshot {
    pub phase: Phase,
    pub check: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ok: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ms: Option<u64>,
    pub steps: Vec<StepView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<Input>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    pub report: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub warnings: Vec<Warning>,
}

pub enum Answer {
    Values(Vec<Zeroizing<String>>),
    Cancel,
}

struct Inner {
    snapshot: Snapshot,
    waiting: Option<mpsc::Sender<Answer>>,
    last: u64,
    ring: VecDeque<Event>,
    asked: u64,
    started: Instant,
    armed: Option<bool>,
    engine: Option<Arc<Engine>>,
    host: Option<Host>,
}

pub struct Bridge {
    inner: Mutex<Inner>,
    live: broadcast::Sender<Event>,
}

fn fresh(id: &str) -> StepView {
    StepView {
        id: id.into(),
        status: "pending".into(),
        warnings: Vec::new(),
        info: Map::new(),
        error: None,
        ms: None,
        log: VecDeque::new(),
    }
}

impl Bridge {
    pub fn new(ids: &[&str]) -> Self {
        let (live, _) = broadcast::channel(1024);
        Self {
            inner: Mutex::new(Inner {
                snapshot: Snapshot {
                    phase: Phase::Idle,
                    check: false,
                    ok: None,
                    ms: None,
                    steps: ids.iter().map(|id| fresh(id)).collect(),
                    input: None,
                    report: None,
                    error: None,
                    warnings: Vec::new(),
                },
                waiting: None,
                last: 0,
                ring: VecDeque::new(),
                asked: 0,
                started: Instant::now(),
                armed: None,
                engine: None,
                host: None,
            }),
            live,
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn emit(&self, inner: &mut Inner, kind: &'static str, data: Value) {
        inner.last += 1;
        let event = Event {
            id: inner.last,
            kind,
            data,
        };
        if inner.ring.len() == RING {
            inner.ring.pop_front();
        }
        inner.ring.push_back(event.clone());
        let _ = self.live.send(event);
    }

    pub fn snapshot(&self) -> Snapshot {
        self.lock().snapshot.clone()
    }

    pub fn phase(&self) -> Phase {
        self.lock().snapshot.phase
    }

    pub fn ok(&self) -> Option<bool> {
        self.lock().snapshot.ok
    }

    pub fn set_engine(&self, engine: Engine) {
        let mut inner = self.lock();
        inner.engine = Some(Arc::new(engine));
        inner.host = None;
    }

    pub fn engine(&self) -> Option<Arc<Engine>> {
        self.lock().engine.clone()
    }

    pub fn ensure_engine(&self, make: impl FnOnce() -> Engine) -> Arc<Engine> {
        let mut inner = self.lock();
        inner.engine.get_or_insert_with(|| Arc::new(make())).clone()
    }

    pub fn cached_host(&self) -> Option<Host> {
        self.lock().host.clone()
    }

    pub fn keep_host(&self, host: Host) {
        self.lock().host = Some(host);
    }

    pub fn arm(&self, check: bool) {
        self.lock().armed = Some(check);
    }

    pub fn take_armed(&self) -> Option<bool> {
        self.lock().armed.take()
    }

    pub fn subscribe(&self, after: Option<u64>) -> (Vec<Event>, broadcast::Receiver<Event>) {
        let inner = self.lock();
        let receiver = self.live.subscribe();
        let covered = after.filter(|seen| {
            *seen <= inner.last
                && inner
                    .ring
                    .front()
                    .map_or(*seen == inner.last, |first| first.id <= seen + 1)
        });
        let replay = match covered {
            Some(seen) => inner
                .ring
                .iter()
                .filter(|event| event.id > seen)
                .cloned()
                .collect(),
            None => vec![Event {
                id: inner.last,
                kind: "snapshot",
                data: json!(inner.snapshot),
            }],
        };
        (replay, receiver)
    }

    pub fn begin(&self, check: bool) -> bool {
        let mut inner = self.lock();
        if matches!(inner.snapshot.phase, Phase::Checking | Phase::Running) {
            return false;
        }
        inner.armed = None;
        inner.started = Instant::now();
        let ids: Vec<String> = inner.snapshot.steps.iter().map(|s| s.id.clone()).collect();
        inner.snapshot = Snapshot {
            phase: if check {
                Phase::Checking
            } else {
                Phase::Running
            },
            check,
            ok: None,
            ms: None,
            steps: ids.iter().map(|id| fresh(id)).collect(),
            input: None,
            report: None,
            error: None,
            warnings: Vec::new(),
        };
        let data = json!(inner.snapshot);
        self.emit(&mut inner, "snapshot", data);
        true
    }

    pub fn log(&self, step: Option<&str>, stream: &'static str, line: &str) {
        let mut inner = self.lock();
        let t = inner.started.elapsed().as_millis() as u64;
        if let Some(view) = step.and_then(|id| inner.snapshot.steps.iter_mut().find(|s| s.id == id))
        {
            if view.log.len() == STEP_LOG {
                view.log.pop_front();
            }
            view.log.push_back(Line {
                t,
                stream,
                line: line.into(),
            });
        }
        self.emit(
            &mut inner,
            "log",
            json!({ "step": step, "t": t, "stream": stream, "line": line }),
        );
    }

    pub fn started(&self, id: &str) {
        let mut inner = self.lock();
        if let Some(view) = inner.snapshot.steps.iter_mut().find(|s| s.id == id) {
            view.status = "running".into();
        }
        self.emit(&mut inner, "step", json!({ "id": id, "status": "running" }));
    }

    pub fn finished(&self, id: &str, state: &StepState, ms: Option<u64>) {
        let mut inner = self.lock();
        let status = json!(state.status)
            .as_str()
            .unwrap_or("unknown")
            .to_string();
        let error = state
            .info
            .get("error")
            .and_then(Value::as_str)
            .map(String::from);
        let mut info = state.info.clone();
        info.remove("error");
        for text in &state.warnings {
            inner.snapshot.warnings.push(Warning {
                step: id.into(),
                text: text.clone(),
            });
        }
        if let Some(view) = inner.snapshot.steps.iter_mut().find(|s| s.id == id) {
            view.status = status.clone();
            view.warnings = state.warnings.clone();
            view.info = info.clone();
            view.error = error.clone();
            view.ms = ms;
        }
        let mut data = json!({
            "id": id,
            "status": status,
            "warnings": state.warnings,
            "info": info,
        });
        if let Some(ms) = ms {
            data["ms"] = json!(ms);
        }
        if let Some(error) = error {
            data["error"] = json!(error);
        }
        self.emit(&mut inner, "step", data);
    }

    pub fn ask(
        &self,
        step: Option<&str>,
        kind: &str,
        title: &str,
        why: &str,
        fields: Vec<Field>,
        error: Option<&str>,
    ) -> Result<Vec<Zeroizing<String>>> {
        let (sender, receiver) = mpsc::channel();
        {
            let mut inner = self.lock();
            inner.asked += 1;
            let input = Input {
                id: format!("p{}", inner.asked),
                step: step.map(String::from),
                kind: kind.into(),
                title: title.into(),
                why: why.into(),
                fields,
                error: error.map(String::from),
            };
            inner.waiting = Some(sender);
            inner.snapshot.input = Some(input.clone());
            self.emit(&mut inner, "input", json!(input));
        }
        match receiver.recv() {
            Ok(Answer::Values(values)) => Ok(values),
            Ok(Answer::Cancel) | Err(_) => bail!(CANCELLED),
        }
    }

    pub fn answer(&self, id: &str, answer: Answer) -> bool {
        let mut inner = self.lock();
        if inner.snapshot.input.as_ref().map(|input| input.id.as_str()) != Some(id) {
            return false;
        }
        inner.snapshot.input = None;
        let cancelled = matches!(answer, Answer::Cancel);
        if let Some(waiting) = inner.waiting.take() {
            let _ = waiting.send(answer);
        }
        self.emit(
            &mut inner,
            "answered",
            json!({ "id": id, "cancelled": cancelled }),
        );
        true
    }

    pub fn finish(&self, result: Result<(bool, Value)>) {
        let mut inner = self.lock();
        let ms = inner.started.elapsed().as_millis() as u64;
        let (ok, report, error) = match result {
            Ok((ok, report)) => (ok, Some(report), None),
            Err(error) => (false, None, Some(format!("{error:#}"))),
        };
        inner.snapshot.phase = Phase::Done;
        inner.snapshot.ok = Some(ok);
        inner.snapshot.ms = Some(ms);
        inner.snapshot.report = report.clone();
        inner.snapshot.error = error.clone();
        inner.snapshot.input = None;
        inner.waiting = None;
        let mut data = json!({
            "ok": ok,
            "ms": ms,
            "check": inner.snapshot.check,
            "report": report,
            "warnings": inner.snapshot.warnings,
        });
        if let Some(error) = error {
            data["error"] = json!(error);
        }
        self.emit(&mut inner, "done", data);
    }
}
