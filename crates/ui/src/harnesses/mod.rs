mod jobs;
#[cfg(test)]
mod tests;

pub use jobs::{Action, Harnesses, Refused};

use crate::reply::error;
use crate::state::Shared;
use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event as Sent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

#[derive(Debug, Serialize, ToSchema)]
pub struct Harness {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub command: String,
    pub logo: Option<String>,
    pub can_sign_in: bool,
    pub sign_in_hint: Option<String>,
    pub asks: Option<String>,
    pub models: Vec<Model>,
    pub installed: bool,
    pub signed_in: bool,
    pub connected: bool,
    pub job: Option<Job>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Model {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct Job {
    pub id: String,
    pub harness: String,
    pub action: String,
    pub running: bool,
    pub ok: Option<bool>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct List {
    pub harnesses: Vec<Harness>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Started {
    pub job: Job,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct Answer {
    pub text: String,
}

pub fn routes() -> OpenApiRouter<Shared> {
    with(Arc::new(Harnesses::public()))
}

pub fn with(harnesses: Arc<Harnesses>) -> OpenApiRouter<Shared> {
    OpenApiRouter::new()
        .routes(routes!(list))
        .routes(routes!(install))
        .routes(routes!(sign_in))
        .routes(routes!(events))
        .routes(routes!(answer))
        .layer(Extension(harnesses))
}

#[utoipa::path(
    get,
    path = "/harnesses",
    tag = "harness",
    responses((status = 200, description = "Every harness: installed, signed in, its models, its logo and its latest job", body = List))
)]
async fn list(Extension(harnesses): Extension<Arc<Harnesses>>) -> Json<List> {
    Json(List {
        harnesses: harnesses.list(),
    })
}

#[utoipa::path(
    post,
    path = "/harnesses/{id}/install",
    tag = "harness",
    params(("id" = String, Path, description = "The harness id, as the list names it")),
    responses(
        (status = 202, description = "The install runs; its output is on the job's events", body = Started),
        (status = 404, description = "There is no such harness", body = crate::Reply),
        (status = 409, description = "A job for this harness is already running", body = crate::Reply)
    )
)]
async fn install(
    Extension(harnesses): Extension<Arc<Harnesses>>,
    Path(id): Path<String>,
) -> Response {
    started(harnesses.start(&id, Action::Install))
}

#[utoipa::path(
    post,
    path = "/harnesses/{id}/sign-in",
    tag = "harness",
    params(("id" = String, Path, description = "The harness id, as the list names it")),
    responses(
        (status = 202, description = "The sign-in runs; its output, link and code are on the job's events", body = Started),
        (status = 404, description = "There is no such harness", body = crate::Reply),
        (status = 409, description = "A job for this harness is already running, or it signs in the first time it runs", body = crate::Reply)
    )
)]
async fn sign_in(
    Extension(harnesses): Extension<Arc<Harnesses>>,
    Path(id): Path<String>,
) -> Response {
    started(harnesses.start(&id, Action::SignIn))
}

fn started(result: Result<Job, Refused>) -> Response {
    match result {
        Ok(job) => (StatusCode::ACCEPTED, Json(Started { job })).into_response(),
        Err(refused) => error(refused.status, refused.code, &refused.message),
    }
}

#[utoipa::path(
    get,
    path = "/harnesses/jobs/{job}/events",
    tag = "harness",
    params(
        ("job" = String, Path, description = "The job id from install or sign-in"),
        ("Last-Event-ID" = Option<String>, Header, description = "The last event seen; the stream carries on after it")
    ),
    responses(
        (status = 200, description = "Server-sent events: line, link, code, error and a last done, each with a counter id", content_type = "text/event-stream"),
        (status = 404, description = "There is no such job", body = crate::Reply)
    )
)]
async fn events(
    Extension(harnesses): Extension<Arc<Harnesses>>,
    Path(job): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(run) = harnesses.job(&job) else {
        return error(404, "not_found", "There is no such job.");
    };
    let after = headers
        .get("last-event-id")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<usize>().ok())
        .map_or(0, |seen| seen + 1);
    let stream = run.stream(after);
    let stream = futures_util::StreamExt::map(stream, |(at, event)| {
        let kind = match &event {
            harness::job::Event::Line { .. } => "line",
            harness::job::Event::Link { .. } => "link",
            harness::job::Event::Code { .. } => "code",
            harness::job::Event::Error { .. } => "error",
            harness::job::Event::Done { .. } => "done",
        };
        Sent::default()
            .event(kind)
            .id(at.to_string())
            .json_data(&event)
    });
    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

#[utoipa::path(
    post,
    path = "/harnesses/jobs/{job}/answer",
    tag = "harness",
    params(("job" = String, Path, description = "The running sign-in's job id")),
    request_body = Answer,
    responses(
        (status = 204, description = "Typed into the job; it is never shown back"),
        (status = 404, description = "There is no such job", body = crate::Reply),
        (status = 409, description = "The job has ended", body = crate::Reply)
    )
)]
async fn answer(
    Extension(harnesses): Extension<Arc<Harnesses>>,
    Path(job): Path<String>,
    Json(answer): Json<Answer>,
) -> Response {
    let Some(run) = harnesses.job(&job) else {
        return error(404, "not_found", "There is no such job.");
    };
    if run.answer(answer.text) {
        StatusCode::NO_CONTENT.into_response()
    } else {
        error(409, "ended", "That job has ended; start it again.")
    }
}
