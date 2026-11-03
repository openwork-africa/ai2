use crate::state::Shared;
use agents::{
    AgentFound, AgentList, Enhanced, Follow, NewAgent, Problem, Removed, RunAsk, RunEvent,
    RunFound, RunList,
};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures_util::stream::{self, Stream};
use std::collections::VecDeque;
use std::convert::Infallible;
use std::time::Duration;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

const POLL: Duration = Duration::from_millis(150);

pub fn routes() -> OpenApiRouter<Shared> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(show, remove))
        .routes(routes!(enhance))
        .routes(routes!(runs, start))
        .routes(routes!(run))
        .routes(routes!(run_events))
}

struct Failed(Problem);

impl From<Problem> for Failed {
    fn from(problem: Problem) -> Failed {
        Failed(problem)
    }
}

impl IntoResponse for Failed {
    fn into_response(self) -> Response {
        let status =
            StatusCode::from_u16(self.0.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, Json(self.0)).into_response()
    }
}

type Answer<T> = Result<T, Failed>;

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> agents::Result<T> + Send + 'static,
) -> Answer<T> {
    Ok(tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| Problem::internal(error.to_string()))??)
}

#[utoipa::path(
    get,
    path = "/agents",
    tag = "agents",
    responses((status = 200, description = "Every kept agent, by name", body = AgentList))
)]
async fn list(State(state): State<Shared>) -> Answer<Json<AgentList>> {
    let agents = state.agents.clone();
    let agents = blocking(move || agents.list()).await?;
    Ok(Json(AgentList { agents }))
}

#[utoipa::path(
    post,
    path = "/agents",
    tag = "agents",
    request_body = NewAgent,
    responses(
        (status = 201, description = "Kept; a short personality was expanded when a model is set up", body = AgentFound),
        (status = 200, description = "The agent with this name was replaced", body = AgentFound),
        (status = 400, description = "A field is wrong, e.g. unknown_harness or not_a_folder", body = Problem)
    )
)]
async fn create(State(state): State<Shared>, Json(new): Json<NewAgent>) -> Answer<Response> {
    let agents = state.agents.clone();
    let (agent, replaced) = blocking(move || {
        let replaced = agents.get(new.name.trim()).is_ok();
        Ok((agents.create(&new, None)?, replaced))
    })
    .await?;
    let status = if replaced {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok((status, Json(AgentFound { agent })).into_response())
}

#[utoipa::path(
    get,
    path = "/agents/{name}",
    tag = "agents",
    params(("name" = String, Path, description = "The agent's name")),
    responses(
        (status = 200, description = "The agent", body = AgentFound),
        (status = 404, description = "No agent with this name", body = Problem)
    )
)]
async fn show(State(state): State<Shared>, Path(name): Path<String>) -> Answer<Json<AgentFound>> {
    let agents = state.agents.clone();
    let agent = blocking(move || agents.get(&name)).await?;
    Ok(Json(AgentFound { agent }))
}

#[utoipa::path(
    delete,
    path = "/agents/{name}",
    tag = "agents",
    params(("name" = String, Path, description = "The agent's name")),
    responses(
        (status = 200, description = "Forgotten; its runs are kept", body = Removed),
        (status = 404, description = "No agent with this name", body = Problem)
    )
)]
async fn remove(State(state): State<Shared>, Path(name): Path<String>) -> Answer<Json<Removed>> {
    let agents = state.agents.clone();
    let removed = blocking(move || agents.remove(&name).map(|_| name)).await?;
    Ok(Json(Removed { removed }))
}

#[utoipa::path(
    post,
    path = "/agents/{name}/enhance",
    tag = "agents",
    params(("name" = String, Path, description = "The agent's name")),
    responses(
        (status = 200, description = "enhanced, skipped (nothing short to expand) or failed with the reason; the agent as it is now", body = Enhanced),
        (status = 404, description = "No agent with this name", body = Problem)
    )
)]
async fn enhance(State(state): State<Shared>, Path(name): Path<String>) -> Answer<Json<Enhanced>> {
    let agents = state.agents.clone();
    Ok(Json(blocking(move || agents.enhance(&name, None)).await?))
}

#[utoipa::path(
    get,
    path = "/runs",
    tag = "agents",
    responses((status = 200, description = "Every agent run, newest first", body = RunList))
)]
async fn runs(State(state): State<Shared>) -> Answer<Json<RunList>> {
    let agents = state.agents.clone();
    let runs = blocking(move || Ok(agents.runs().list())).await?;
    Ok(Json(RunList { runs }))
}

#[utoipa::path(
    post,
    path = "/runs",
    tag = "agents",
    request_body = RunAsk,
    responses(
        (status = 202, description = "The run started; follow it on its events", body = RunFound),
        (status = 400, description = "No prompt, no agent or harness, or not a folder", body = Problem),
        (status = 404, description = "No agent with this name", body = Problem)
    )
)]
async fn start(State(state): State<Shared>, Json(ask): Json<RunAsk>) -> Answer<Response> {
    let agents = state.agents.clone();
    let prepared = blocking(move || agents.prepare(&ask)).await?;
    let run = prepared.run().clone();
    let agents = state.agents.clone();
    std::thread::spawn(move || agents.execute(prepared, &mut |_| {}));
    Ok((StatusCode::ACCEPTED, Json(RunFound { run })).into_response())
}

#[utoipa::path(
    get,
    path = "/runs/{id}",
    tag = "agents",
    params(("id" = String, Path, description = "The run's id")),
    responses(
        (status = 200, description = "The run's record", body = RunFound),
        (status = 404, description = "No such run", body = Problem)
    )
)]
async fn run(State(state): State<Shared>, Path(id): Path<String>) -> Answer<Json<RunFound>> {
    let agents = state.agents.clone();
    let run = blocking(move || agents.runs().get(&id)).await?;
    Ok(Json(RunFound { run }))
}

struct Tail {
    follow: Follow,
    pending: VecDeque<(usize, RunEvent)>,
}

#[utoipa::path(
    get,
    path = "/runs/{id}/events",
    tag = "agents",
    params(("id" = String, Path, description = "The run's id")),
    responses(
        (status = 200, description = "Server-sent run events: started, its lines, exited, the review, then done; Last-Event-ID replays from there", content_type = "text/event-stream", body = RunEvent),
        (status = 404, description = "No such run", body = Problem)
    )
)]
async fn run_events(
    State(state): State<Shared>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Answer<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let from = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<usize>().ok())
        .map_or(0, |last| last + 1);
    let follow = state.agents.runs().follow(&id, from)?;
    let tail = Tail {
        follow,
        pending: VecDeque::new(),
    };
    let stream = stream::unfold(tail, |mut tail| async move {
        loop {
            if let Some((at, event)) = tail.pending.pop_front() {
                let sse = Event::default()
                    .event(event.name())
                    .id(at.to_string())
                    .json_data(&event)
                    .unwrap_or_default();
                return Some((Ok(sse), tail));
            }
            if tail.follow.done() {
                return None;
            }
            let fresh = tail.follow.read();
            if fresh.is_empty() {
                tokio::time::sleep(POLL).await;
            }
            tail.pending.extend(fresh);
        }
    });
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

#[cfg(test)]
mod tests;
