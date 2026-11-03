use crate::reply::error;
use crate::state::Shared;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::convert::Infallible;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Notify;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

pub type Launch = Arc<dyn Fn(&NewReview) -> anyhow::Result<Launched> + Send + Sync>;

pub struct Launched {
    pub command: std::process::Command,
    pub hash: String,
}

static LAUNCH: OnceLock<Launch> = OnceLock::new();
static JOBS: OnceLock<Mutex<HashMap<String, Arc<Job>>>> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);

pub fn inject(launch: Launch) -> bool {
    LAUNCH.set(launch).is_ok()
}

fn launcher() -> Option<&'static Launch> {
    LAUNCH.get()
}

fn jobs() -> &'static Mutex<HashMap<String, Arc<Job>>> {
    JOBS.get_or_init(Default::default)
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Reviews {
    pub reviews: Vec<Summary>,
    pub can_start: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Summary {
    pub hash: String,
    pub folder: String,
    pub objective: String,
    pub generated_at: String,
    pub harness: Option<String>,
    pub ratings: Vec<Score>,
    pub findings: usize,
    pub open_findings: usize,
    pub exploits: usize,
    pub report: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Score {
    pub key: String,
    pub name: String,
    pub short: String,
    pub riskier: bool,
    pub score: Option<f64>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, ToSchema)]
pub struct NewReview {
    pub folder: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objective: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub providers: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Started {
    pub job: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum JobEvent {
    Log {
        line: String,
    },
    Done {
        ok: bool,
        code: Option<i32>,
        hash: String,
        message: Option<String>,
    },
}

struct Job {
    hash: String,
    events: Mutex<Vec<JobEvent>>,
    finished: std::sync::atomic::AtomicBool,
    notify: Notify,
}

impl Job {
    fn push(&self, event: JobEvent) {
        let done = matches!(event, JobEvent::Done { .. });
        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
        if done {
            self.finished.store(true, Ordering::SeqCst);
        }
        self.notify.notify_waiters();
    }

    fn at(&self, index: usize) -> Option<JobEvent> {
        self.events.lock().ok()?.get(index).cloned()
    }

    async fn next(&self, index: usize) -> Option<JobEvent> {
        loop {
            let notified = self.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if let Some(event) = self.at(index) {
                return Some(event);
            }
            if self.finished.load(Ordering::SeqCst) {
                return None;
            }
            notified.await;
        }
    }
}

pub fn routes() -> OpenApiRouter<Shared> {
    OpenApiRouter::new()
        .routes(routes!(list, start))
        .routes(routes!(one))
        .routes(routes!(job_events))
}

fn folder(state: &Shared) -> PathBuf {
    state.home.join("reviews")
}

pub fn valid_hash(hash: &str) -> bool {
    hash.len() == 16
        && hash
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

fn read(dir: &std::path::Path) -> Option<Value> {
    let bytes = std::fs::read(dir.join("review.json")).ok()?;
    serde_json::from_slice::<Value>(&bytes)
        .ok()
        .filter(Value::is_object)
}

fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_string()
}

fn summary(hash: String, dir: &std::path::Path, review: &Value) -> Summary {
    let empty = Vec::new();
    let ratings = review["ratings"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .map(|card| Score {
            key: text(card, "key"),
            name: text(card, "name"),
            short: text(card, "short"),
            riskier: card["riskier"].as_bool().unwrap_or(false),
            score: card["rating"]["score"].as_f64(),
        })
        .collect();
    let findings = review["findings"].as_array().unwrap_or(&empty);
    Summary {
        hash,
        folder: text(review, "folder"),
        objective: text(review, "objective"),
        generated_at: text(review, "generated_at"),
        harness: review["harness"].as_str().map(String::from),
        ratings,
        findings: findings.len(),
        open_findings: findings
            .iter()
            .filter(|f| {
                !f["status"]
                    .as_str()
                    .unwrap_or_default()
                    .eq_ignore_ascii_case("fixed")
            })
            .count(),
        exploits: review["exploits"].as_array().map_or(0, Vec::len),
        report: dir.join("report.html").is_file(),
    }
}

pub fn scan(root: &std::path::Path) -> Vec<Summary> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut found: Vec<Summary> = entries
        .flatten()
        .filter_map(|entry| {
            let hash = entry.file_name().into_string().ok()?;
            if !valid_hash(&hash) {
                return None;
            }
            let dir = entry.path();
            let review = read(&dir)?;
            Some(summary(hash, &dir, &review))
        })
        .collect();
    found.sort_by(|a, b| {
        b.generated_at
            .cmp(&a.generated_at)
            .then_with(|| a.folder.cmp(&b.folder))
    });
    found
}

#[utoipa::path(
    get,
    path = "/reviews",
    tag = "reviews",
    responses((status = 200, description = "Every review in the ai2 home, newest first", body = Reviews))
)]
async fn list(State(state): State<Shared>) -> Json<Reviews> {
    let root = folder(&state);
    let reviews = tokio::task::spawn_blocking(move || scan(&root))
        .await
        .unwrap_or_default();
    Json(Reviews {
        reviews,
        can_start: launcher().is_some(),
    })
}

#[utoipa::path(
    get,
    path = "/reviews/{hash}",
    tag = "reviews",
    params(("hash" = String, Path, description = "The folder hash: sixteen lowercase hex digits")),
    responses(
        (status = 200, description = "The review.json as it is", body = Object),
        (status = 400, description = "Not a folder hash", body = crate::Reply),
        (status = 404, description = "No review with this hash", body = crate::Reply)
    )
)]
async fn one(State(state): State<Shared>, Path(hash): Path<String>) -> Response {
    if !valid_hash(&hash) {
        return error(
            400,
            "bad_request",
            "A review hash is sixteen lowercase hex digits.",
        );
    }
    let dir = folder(&state).join(&hash);
    match tokio::task::spawn_blocking(move || read(&dir))
        .await
        .ok()
        .flatten()
    {
        Some(review) => Json(review).into_response(),
        None => error(404, "not_found", "There is no review with this hash."),
    }
}

#[utoipa::path(
    post,
    path = "/reviews",
    tag = "reviews",
    request_body = NewReview,
    responses(
        (status = 202, description = "The review runs; follow it on the job's events", body = Started),
        (status = 400, description = "No folder given", body = crate::Reply),
        (status = 500, description = "The review could not start", body = crate::Reply),
        (status = 501, description = "This build cannot start reviews", body = crate::Reply)
    )
)]
async fn start(Json(request): Json<NewReview>) -> Response {
    let Some(launch) = launcher() else {
        return error(
            501,
            "not_available",
            "This ai2 ui cannot start reviews. Run ai2 in the folder instead.",
        );
    };
    if request.folder.trim().is_empty() {
        return error(400, "bad_request", "Say which folder to review.");
    }
    match spawn(launch, &request) {
        Ok(started) => (StatusCode::ACCEPTED, Json(started)).into_response(),
        Err(failure) => error(500, "not_started", &format!("{failure:#}")),
    }
}

fn spawn(launch: &Launch, request: &NewReview) -> anyhow::Result<Started> {
    let Launched { command, hash } = launch(request)?;
    let mut command = tokio::process::Command::from(command);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn()?;
    let id = format!("{:x}", NEXT.fetch_add(1, Ordering::SeqCst));
    let job = Arc::new(Job {
        hash: hash.clone(),
        events: Mutex::new(Vec::new()),
        finished: Default::default(),
        notify: Notify::new(),
    });
    if let Ok(mut jobs) = jobs().lock() {
        jobs.insert(id.clone(), job.clone());
    }
    let stderr = child.stderr.take();
    tokio::spawn(async move {
        if let Some(stderr) = stderr {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                job.push(JobEvent::Log { line });
            }
        }
        let (ok, code, message) = match child.wait().await {
            Ok(exit) => (exit.success(), exit.code(), None),
            Err(failure) => (false, None, Some(failure.to_string())),
        };
        let hash = job.hash.clone();
        job.push(JobEvent::Done {
            ok,
            code,
            hash,
            message,
        });
    });
    Ok(Started { job: id, hash })
}

#[utoipa::path(
    get,
    path = "/reviews/jobs/{id}/events",
    tag = "reviews",
    params(("id" = String, Path, description = "The job from POST /api/reviews")),
    responses(
        (status = 200, description = "Server-sent events: log lines, then done; Last-Event-ID replays from there", content_type = "text/event-stream", body = JobEvent),
        (status = 404, description = "No such job", body = crate::Reply)
    )
)]
async fn job_events(Path(id): Path<String>, headers: HeaderMap) -> Response {
    let job = jobs().lock().ok().and_then(|jobs| jobs.get(&id).cloned());
    let Some(job) = job else {
        return error(404, "not_found", "There is no such review job.");
    };
    let from = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok())
        .map_or(0, |last| last + 1);
    let stream = futures_util::stream::unfold((job, from), |(job, index)| async move {
        let event = job.next(index).await?;
        let name = match event {
            JobEvent::Log { .. } => "log",
            JobEvent::Done { .. } => "done",
        };
        let sse = Event::default()
            .event(name)
            .id(index.to_string())
            .json_data(&event)
            .unwrap_or_default();
        Some((Ok::<_, Infallible>(sse), (job, index + 1)))
    });
    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

#[cfg(test)]
mod tests;
