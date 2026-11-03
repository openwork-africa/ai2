use crate::state::Shared;
use apps::{Api, AppEvent, AppPatch, IconUpload, LogLine, NewApp, Problem, Shown, Supervisor};
use axum::body::Bytes;
use axum::extract::{FromRequest, Multipart, Path, Query, Request, State};
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use base64::Engine;
use futures_util::stream::{self, Stream};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;
use std::convert::Infallible;
use tokio::sync::broadcast;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

const TAIL: usize = 200;
const REPLAY: usize = 500;

pub fn routes() -> OpenApiRouter<Shared> {
    OpenApiRouter::new()
        .routes(routes!(list, register))
        .routes(routes!(events))
        .routes(routes!(show, update, remove))
        .routes(routes!(document))
        .routes(routes!(check))
        .routes(routes!(icon))
        .routes(routes!(start))
        .routes(routes!(stop))
        .routes(routes!(restart))
        .routes(routes!(logs))
        .routes(routes!(log_events))
}

#[derive(Serialize, ToSchema)]
pub struct AppsReply {
    pub apps: Vec<Shown>,
}

#[derive(Serialize, ToSchema)]
pub struct AppReply {
    pub app: Shown,
}

#[derive(Serialize, ToSchema)]
pub struct ApiReply {
    pub api: Api,
}

#[derive(Serialize, ToSchema)]
pub struct LinesReply {
    pub lines: Vec<LogLine>,
}

#[derive(Deserialize, IntoParams)]
pub struct ListQuery {
    #[param(value_type = Option<String>, example = "1")]
    pub openapi: Option<String>,
}

#[derive(Deserialize, IntoParams)]
pub struct TailQuery {
    pub tail: Option<usize>,
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
    work: impl FnOnce() -> apps::Result<T> + Send + 'static,
) -> apps::Result<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| Problem::internal(error.to_string()))?
}

fn yes(value: Option<&str>) -> bool {
    matches!(value, Some("1" | "true" | "yes"))
}

#[utoipa::path(
    get,
    path = "/apps",
    tag = "apps",
    params(ListQuery),
    responses((status = 200, description = "Every registered app, whether it runs, and its OpenAPI state; openapi=1 inlines each fetched document", body = AppsReply))
)]
async fn list(
    State(state): State<Shared>,
    Query(query): Query<ListQuery>,
) -> Answer<Json<AppsReply>> {
    let apps = state.apps.list(yes(query.openapi.as_deref())).await?;
    Ok(Json(AppsReply { apps }))
}

#[utoipa::path(
    post,
    path = "/apps",
    tag = "apps",
    request_body(content = NewApp, description = "JSON with the icon as { type, base64 }, or multipart/form-data with an app JSON part and an icon file part"),
    responses(
        (status = 201, description = "Registered", body = AppReply),
        (status = 200, description = "An app with this name was updated", body = AppReply),
        (status = 422, description = "A required field is missing, e.g. openapi_required", body = Problem),
        (status = 413, description = "The icon is over 512 KiB", body = Problem),
        (status = 400, description = "The icon is not an allowed image, or an svg with a script", body = Problem)
    )
)]
async fn register(State(state): State<Shared>, request: Request) -> Answer<Response> {
    let value = read_app(&state, request).await?;
    let new = NewApp::parse(value)?;
    let store = state.apps.store().clone();
    let (app, created) = blocking(move || store.register(new)).await?;
    let shown = state.apps.show(&app.id).await?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(AppReply { app: shown })).into_response())
}

async fn read_app(state: &Shared, request: Request) -> apps::Result<Value> {
    let multipart = request
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("multipart/form-data"));
    let mut value = if multipart {
        let form = Multipart::from_request(request, state)
            .await
            .map_err(|e| Problem::new(400, "invalid", e.body_text()))?;
        read_form(form).await?
    } else {
        let body = Bytes::from_request(request, state)
            .await
            .map_err(|e| Problem::new(413, "too_large", e.body_text()))?;
        parse_json(&body)?
    };
    refuse_icon_path(&mut value)?;
    Ok(value)
}

fn parse_json(body: &[u8]) -> apps::Result<Value> {
    serde_json::from_slice(body)
        .map_err(|e| Problem::new(400, "invalid", format!("The body is not JSON: {e}")))
}

async fn read_form(mut form: Multipart) -> apps::Result<Value> {
    let mut app = None;
    let mut icon = None;
    while let Some(field) = form
        .next_field()
        .await
        .map_err(|e| Problem::new(400, "invalid", e.body_text()))?
    {
        match field.name() {
            Some("app") => {
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| Problem::new(400, "invalid", e.body_text()))?;
                app = Some(parse_json(&bytes)?);
            }
            Some("icon") => {
                let kind = field.content_type().map(str::to_string);
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| Problem::new(413, "icon_too_large", e.body_text()))?;
                if !bytes.is_empty() {
                    icon = Some(IconUpload {
                        content_type: kind.filter(|k| k.starts_with("image/")),
                        base64: Some(base64::engine::general_purpose::STANDARD.encode(&bytes)),
                        path: None,
                    });
                }
            }
            _ => {}
        }
    }
    let mut app =
        app.ok_or_else(|| Problem::new(400, "invalid", "Send the app as a JSON part named app."))?;
    if let (Some(icon), Some(object)) = (icon, app.as_object_mut()) {
        object.insert("icon".to_string(), serde_json::to_value(icon)?);
    }
    Ok(app)
}

fn refuse_icon_path(value: &mut Value) -> apps::Result<()> {
    if value["icon"].get("path").is_some_and(|p| !p.is_null()) {
        return Err(Problem::new(
            400,
            "icon_path",
            "Send the icon as base64 or as a file part; ai2 ui does not read files from disk for a request.",
        ));
    }
    Ok(())
}

#[utoipa::path(
    get,
    path = "/apps/events",
    tag = "apps",
    responses((status = 200, description = "Server-sent app events, one AppEvent JSON per data line, whenever an app starts or stops", body = AppEvent, content_type = "text/event-stream"))
)]
async fn events(State(state): State<Shared>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let receiver = state.apps.events();
    let stream = stream::unfold(receiver, |mut receiver| async move {
        loop {
            match receiver.recv().await {
                Ok(event) => {
                    let sent = Event::default().event("app").json_data(&event).ok()?;
                    return Some((Ok(sent), receiver));
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[utoipa::path(
    get,
    path = "/apps/{id}",
    tag = "apps",
    params(("id" = String, Path, description = "The app's id")),
    responses(
        (status = 200, description = "The app, its status and its OpenAPI state", body = AppReply),
        (status = 404, description = "No such app", body = Problem)
    )
)]
async fn show(State(state): State<Shared>, Path(id): Path<String>) -> Answer<Json<AppReply>> {
    Ok(Json(AppReply {
        app: state.apps.show(&id).await?,
    }))
}

#[utoipa::path(
    patch,
    path = "/apps/{id}",
    tag = "apps",
    params(("id" = String, Path, description = "The app's id")),
    request_body(content = AppPatch, description = "Any fields of the app; the icon as { type, base64 }"),
    responses(
        (status = 200, description = "The app as updated", body = AppReply),
        (status = 404, description = "No such app", body = Problem)
    )
)]
async fn update(
    State(state): State<Shared>,
    Path(id): Path<String>,
    body: Bytes,
) -> Answer<Json<AppReply>> {
    let mut value = parse_json(&body)?;
    refuse_icon_path(&mut value)?;
    let patch: AppPatch =
        serde_json::from_value(value).map_err(|e| Problem::new(400, "invalid", e.to_string()))?;
    let store = state.apps.store().clone();
    let target = id.clone();
    blocking(move || store.update(&target, patch)).await?;
    Ok(Json(AppReply {
        app: state.apps.show(&id).await?,
    }))
}

#[utoipa::path(
    delete,
    path = "/apps/{id}",
    tag = "apps",
    params(("id" = String, Path, description = "The app's id")),
    responses(
        (status = 204, description = "Removed; a running app keeps running"),
        (status = 404, description = "No such app", body = Problem)
    )
)]
async fn remove(State(state): State<Shared>, Path(id): Path<String>) -> Answer<StatusCode> {
    let supervisor: Supervisor = state.apps.clone();
    blocking(move || supervisor.remove(&id)).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/apps/{id}/openapi.json",
    tag = "apps",
    params(("id" = String, Path, description = "The app's id")),
    responses(
        (status = 200, description = "The app's own OpenAPI document as last fetched", body = Object),
        (status = 404, description = "openapi_unchecked until the first successful check; the message names where the app serves it", body = Problem)
    )
)]
async fn document(State(state): State<Shared>, Path(id): Path<String>) -> Answer<Json<Value>> {
    let store = state.apps.store();
    let app = store.app(&id)?;
    match store.document(&id) {
        Some(document) => Ok(Json(document)),
        None => Err(Problem::new(
            404,
            "openapi_unchecked",
            format!(
                "{}'s OpenAPI document has not been checked yet; it is at {}.",
                app.name,
                apps::openapi::location(&app)
            ),
        )
        .into()),
    }
}

#[utoipa::path(
    post,
    path = "/apps/{id}/check",
    tag = "apps",
    params(("id" = String, Path, description = "The app's id")),
    responses(
        (status = 200, description = "Fetched and validated the app's OpenAPI document now", body = ApiReply),
        (status = 404, description = "No such app", body = Problem)
    )
)]
async fn check(State(state): State<Shared>, Path(id): Path<String>) -> Answer<Json<ApiReply>> {
    Ok(Json(ApiReply {
        api: state.apps.check(&id).await?,
    }))
}

#[utoipa::path(
    get,
    path = "/apps/{id}/icon",
    tag = "apps",
    params(("id" = String, Path, description = "The app's id")),
    responses(
        (status = 200, description = "The app's favicon with its stored type, or an SVG letter tile when it has none", content_type = "image/*"),
        (status = 404, description = "No such app", body = Problem)
    )
)]
async fn icon(State(state): State<Shared>, Path(id): Path<String>) -> Answer<Response> {
    let store = state.apps.store().clone();
    let target = id.clone();
    let found = blocking(move || {
        let app = store.app(&target)?;
        Ok((app.name, store.icon(&target).ok().flatten()))
    })
    .await?;
    Ok(match found {
        (_, Some((kind, bytes))) => ([(CONTENT_TYPE, kind)], bytes).into_response(),
        (name, None) => (
            [(CONTENT_TYPE, "image/svg+xml".to_string())],
            letter_tile(&name),
        )
            .into_response(),
    })
}

fn letter_tile(name: &str) -> String {
    let letter = name
        .chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().collect::<String>())
        .unwrap_or_else(|| "?".to_string());
    let hue = name
        .bytes()
        .fold(0u32, |sum, b| sum.wrapping_mul(31).wrapping_add(b as u32))
        % 360;
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect width="64" height="64" rx="14" fill="hsl({hue} 45% 92%)"/><text x="32" y="42" font-family="system-ui, sans-serif" font-size="28" font-weight="600" text-anchor="middle" fill="hsl({hue} 40% 30%)">{letter}</text></svg>"##,
        letter = escape(&letter)
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[utoipa::path(
    post,
    path = "/apps/{id}/start",
    tag = "apps",
    params(("id" = String, Path, description = "The app's id")),
    responses(
        (status = 200, description = "Started; the port is open and the OpenAPI document was checked", body = AppReply),
        (status = 409, description = "already_running", body = Problem),
        (status = 500, description = "exited, with the last log lines", body = Problem),
        (status = 504, description = "port_closed", body = Problem)
    )
)]
async fn start(State(state): State<Shared>, Path(id): Path<String>) -> Answer<Json<AppReply>> {
    Ok(Json(AppReply {
        app: state.apps.start(&id).await?,
    }))
}

#[utoipa::path(
    post,
    path = "/apps/{id}/stop",
    tag = "apps",
    params(("id" = String, Path, description = "The app's id")),
    responses(
        (status = 200, description = "Stopped", body = AppReply),
        (status = 409, description = "not_ours: ai2 did not start it", body = Problem)
    )
)]
async fn stop(State(state): State<Shared>, Path(id): Path<String>) -> Answer<Json<AppReply>> {
    Ok(Json(AppReply {
        app: state.apps.stop(&id).await?,
    }))
}

#[utoipa::path(
    post,
    path = "/apps/{id}/restart",
    tag = "apps",
    params(("id" = String, Path, description = "The app's id")),
    responses(
        (status = 200, description = "Stopped when ai2 started it, then started", body = AppReply),
        (status = 409, description = "already_running when something else holds the port", body = Problem)
    )
)]
async fn restart(State(state): State<Shared>, Path(id): Path<String>) -> Answer<Json<AppReply>> {
    Ok(Json(AppReply {
        app: state.apps.restart(&id).await?,
    }))
}

#[utoipa::path(
    get,
    path = "/apps/{id}/logs",
    tag = "apps",
    params(("id" = String, Path, description = "The app's id"), TailQuery),
    responses(
        (status = 200, description = "The last lines of the app's log", body = LinesReply),
        (status = 404, description = "No such app", body = Problem)
    )
)]
async fn logs(
    State(state): State<Shared>,
    Path(id): Path<String>,
    Query(query): Query<TailQuery>,
) -> Answer<Json<LinesReply>> {
    let supervisor = state.apps.clone();
    let tail = query.tail.unwrap_or(TAIL).min(apps::RING);
    let lines = blocking(move || supervisor.logs(&id, tail)).await?;
    Ok(Json(LinesReply { lines }))
}

struct Tail {
    pending: VecDeque<(Option<u64>, LogLine)>,
    receiver: broadcast::Receiver<(u64, LogLine)>,
    last: Option<u64>,
}

#[utoipa::path(
    get,
    path = "/apps/{id}/logs/events",
    tag = "apps",
    params(("id" = String, Path, description = "The app's id")),
    responses(
        (status = 200, description = "Server-sent log events, one LogLine JSON per data line; Last-Event-ID replays what was missed", body = LogLine, content_type = "text/event-stream"),
        (status = 404, description = "No such app", body = Problem)
    )
)]
async fn log_events(
    State(state): State<Shared>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Answer<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let supervisor = state.apps.clone();
    supervisor.store().app(&id)?;
    let after = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok());
    let receiver = supervisor.subscribe(&id);
    let ring = supervisor.replay(&id, after);
    let mut pending: VecDeque<(Option<u64>, LogLine)> = VecDeque::new();
    if ring.is_empty() && after.is_none() {
        let target = id.clone();
        let file = blocking(move || supervisor.logs(&target, TAIL))
            .await
            .unwrap_or_default();
        pending.extend(file.into_iter().map(|line| (None, line)));
    } else {
        let skip = ring.len().saturating_sub(REPLAY);
        pending.extend(ring.into_iter().skip(skip).map(|(n, line)| (Some(n), line)));
    }
    let last = pending.iter().filter_map(|(n, _)| *n).max().or(after);
    let tail = Tail {
        pending,
        receiver,
        last,
    };
    let stream = stream::unfold(tail, |mut tail| async move {
        loop {
            if let Some((n, line)) = tail.pending.pop_front() {
                return Some((Ok(log_event(n, &line)), tail));
            }
            match tail.receiver.recv().await {
                Ok((n, line)) => {
                    if tail.last.is_some_and(|last| n <= last) {
                        continue;
                    }
                    tail.last = Some(n);
                    return Some((Ok(log_event(Some(n), &line)), tail));
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    });
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

fn log_event(n: Option<u64>, line: &LogLine) -> Event {
    let event = Event::default()
        .event("log")
        .data(serde_json::to_string(line).unwrap_or_default());
    match n {
        Some(n) => event.id(n.to_string()),
        None => event,
    }
}

#[cfg(test)]
mod tests;
