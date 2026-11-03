mod bridge;
mod reporting;
#[cfg(test)]
mod tests;

use crate::reply::error;
use crate::state::{Shared, State as UiState};
use crate::Server;
use ::setup::host::Host;
use ::setup::runner::{Runner, System};
use anyhow::{Context, Result};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures_util::stream::{self, Stream, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::{Arc, Mutex, OnceLock};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use zeroize::Zeroizing;

pub use bridge::{Answer, Bridge, Event, Field, Input, Line, Phase, Snapshot, StepView, Warning};
pub use reporting::Reporting;

pub type Drive = dyn Fn(&dyn Runner, bool) -> Result<(bool, Value)> + Send + Sync;

pub struct Engine {
    pub host: Arc<dyn Fn() -> Host + Send + Sync>,
    pub runner: Arc<dyn Fn() -> Box<dyn Runner> + Send + Sync>,
    pub live: bool,
    pub drive: Arc<Drive>,
}

impl Engine {
    pub fn system(options: ::setup::Options) -> Self {
        Self {
            host: Arc::new(|| Host::detect(&System)),
            runner: Arc::new(|| Box::new(System)),
            live: true,
            drive: Arc::new(move |runner, check| ::setup::setup_with(&options, check, runner)),
        }
    }
}

pub fn step_ids() -> Vec<&'static str> {
    ::setup::all().iter().map(|step| step.id()).collect()
}

type Bridges = Mutex<HashMap<(u16, String), Arc<Bridge>>>;

fn bridges() -> &'static Bridges {
    static BRIDGES: OnceLock<Bridges> = OnceLock::new();
    BRIDGES.get_or_init(Bridges::default)
}

pub fn bridge(state: &UiState) -> Arc<Bridge> {
    let mut all = bridges()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    all.entry((state.port, state.nonce.clone()))
        .or_insert_with(|| Arc::new(Bridge::new(&step_ids())))
        .clone()
}

fn options_for(state: &UiState) -> ::setup::Options {
    let bin = std::env::current_exe().unwrap_or_else(|_| "ai2".into());
    let mut options = ::setup::Options::new(bin, &state.home);
    options.admin = state.admin.clone();
    options
}

fn engine(state: &UiState) -> Arc<Engine> {
    bridge(state).ensure_engine(|| Engine::system(options_for(state)))
}

pub fn start(bridge: &Arc<Bridge>, engine: Arc<Engine>, check: bool) -> bool {
    if !bridge.begin(check) {
        return false;
    }
    let bridge = bridge.clone();
    std::thread::spawn(move || {
        let reporting = Reporting::new((engine.runner)(), bridge.clone(), engine.live);
        let result = (engine.drive)(&reporting, check);
        bridge.finish(result);
    });
    true
}

pub fn arm(state: &UiState, engine: Engine, check: bool) -> Arc<Bridge> {
    let bridge = bridge(state);
    bridge.set_engine(engine);
    bridge.arm(check);
    bridge
}

pub struct Launch {
    pub port: u16,
    pub open: bool,
}

pub fn run(ui: &crate::Options, setup: ::setup::Options, launch: Launch) -> Result<bool> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("starting the ui runtime")?;
    runtime.block_on(async move {
        let server = Server::bind(launch.port, ui).await?;
        serve(server, Engine::system(setup), false, launch.open).await
    })
}

pub async fn serve(server: Server, engine: Engine, check: bool, open: bool) -> Result<bool> {
    let bridge = arm(&server.state(), engine, check);
    let url = server.url(Some(first_page(check)));
    println!("{}", json!({ "url": url, "setup": true }));
    if open {
        if let Err(error) = crate::open(&url) {
            eprintln!("ai2: could not open the browser, open the url yourself: {error:#}");
        }
    }
    server.serve().await?;
    Ok(bridge.ok().unwrap_or(true))
}

pub fn first_page(check: bool) -> &'static str {
    if check {
        "setup"
    } else {
        "welcome"
    }
}

pub fn routes() -> OpenApiRouter<Shared> {
    OpenApiRouter::new()
        .routes(routes!(snapshot))
        .routes(routes!(host))
        .routes(routes!(steps_info))
        .routes(routes!(begin))
        .routes(routes!(events))
        .routes(routes!(answer))
}

fn host_of(state: &UiState) -> Host {
    let bridge = bridge(state);
    if let Some(host) = bridge.cached_host() {
        return host;
    }
    let host = (engine(state).host)();
    bridge.keep_host(host.clone());
    host
}

#[utoipa::path(
    get,
    path = "/setup",
    tag = "setup",
    responses((status = 200, description = "Where setup is: the phase, every step with its log, a waiting input and the final report", body = Snapshot))
)]
async fn snapshot(State(state): State<Shared>) -> Json<Snapshot> {
    Json(bridge(&state).snapshot())
}

#[utoipa::path(
    get,
    path = "/setup/host",
    tag = "setup",
    responses((status = 200, description = "This machine as setup sees it: os, version, chip, systemd, user and whether containers are supported", body = Object))
)]
async fn host(State(state): State<Shared>) -> Response {
    match tokio::task::spawn_blocking(move || host_of(&state).to_json()).await {
        Ok(host) => Json(host).into_response(),
        Err(_) => error(500, "host_failed", "Looking at this machine failed."),
    }
}

#[utoipa::path(
    get,
    path = "/setup/steps/info",
    tag = "setup",
    responses((status = 200, description = "Each step's title, why and fun fact for this machine, and the inputs setup may ask for", body = Object))
)]
async fn steps_info(State(state): State<Shared>) -> Response {
    match tokio::task::spawn_blocking(move || host_of(&state)).await {
        Ok(host) => Json(json!({
            "steps": ::setup::steps_info(&host.os),
            "inputs": ::setup::inputs_info(),
        }))
        .into_response(),
        Err(_) => error(500, "host_failed", "Looking at this machine failed."),
    }
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct Start {
    #[serde(default)]
    pub check: bool,
    #[serde(default)]
    pub from: Option<String>,
}

#[utoipa::path(
    post,
    path = "/setup/start",
    tag = "setup",
    request_body = Start,
    responses(
        (status = 202, description = "Setup started; follow it on /api/setup/events", body = Object),
        (status = 409, description = "Setup is already running", body = crate::Reply)
    )
)]
async fn begin(State(state): State<Shared>, Json(body): Json<Start>) -> Response {
    let bridge = bridge(&state);
    if !start(&bridge, engine(&state), body.check) {
        return error(409, "already_running", "Setup is already running.");
    }
    (
        StatusCode::ACCEPTED,
        Json(json!({ "phase": bridge.phase(), "from": body.from })),
    )
        .into_response()
}

fn sse(event: Event) -> Result<SseEvent, Infallible> {
    Ok(SseEvent::default()
        .event(event.kind)
        .id(event.id.to_string())
        .data(event.data.to_string()))
}

#[utoipa::path(
    get,
    path = "/setup/events",
    tag = "setup",
    params(("Last-Event-ID" = Option<u64>, Header, description = "The last event seen; the server replays what came after it")),
    responses((status = 200, description = "Server-sent events: snapshot, step, log, input, answered, done", content_type = "text/event-stream"))
)]
async fn events(
    State(state): State<Shared>,
    headers: HeaderMap,
) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let bridge = bridge(&state);
    if let Some(check) = bridge.take_armed() {
        start(&bridge, engine(&state), check);
    }
    let after = headers
        .get("last-event-id")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok());
    let (replay, receiver) = bridge.subscribe(after);
    let seen = replay
        .last()
        .map(|event| event.id)
        .or(after)
        .unwrap_or_default();
    let live = stream::unfold((receiver, seen), |(mut receiver, seen)| async move {
        loop {
            match receiver.recv().await {
                Ok(event) if event.id > seen => {
                    let id = event.id;
                    return Some((event, (receiver, id)));
                }
                Ok(_) => continue,
                Err(_) => return None,
            }
        }
    });
    Sse::new(stream::iter(replay).chain(live).map(sse)).keep_alive(KeepAlive::default())
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AnswerBody {
    pub id: String,
    #[serde(default)]
    pub values: Vec<String>,
    #[serde(default)]
    pub cancel: bool,
}

#[utoipa::path(
    post,
    path = "/setup/answer",
    tag = "setup",
    request_body = AnswerBody,
    responses(
        (status = 204, description = "The waiting step has its answer"),
        (status = 409, description = "No input with that id is waiting", body = crate::Reply)
    )
)]
async fn answer(State(state): State<Shared>, Json(body): Json<AnswerBody>) -> Response {
    let answer = if body.cancel {
        Answer::Cancel
    } else {
        Answer::Values(body.values.into_iter().map(Zeroizing::new).collect())
    };
    if bridge(&state).answer(&body.id, answer) {
        StatusCode::NO_CONTENT.into_response()
    } else {
        error(409, "no_such_input", "No question with that id is waiting.")
    }
}
