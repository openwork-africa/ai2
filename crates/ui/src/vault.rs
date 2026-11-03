use crate::admin;
use crate::reply::error;
use crate::state::Shared;
use axum::body::Bytes;
use axum::extract::{OriginalUri, State};
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use utoipa_axum::router::OpenApiRouter;

const PREFIX: &str = "/api/vault/";
pub const PASSPHRASE_HEADER: &str = "x-vault-passphrase";

pub fn routes() -> OpenApiRouter<Shared> {
    OpenApiRouter::new().route("/vault/{*rest}", any(proxy))
}

async fn proxy(
    State(state): State<Shared>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let method = method.as_str().to_string();
    let rest = uri
        .path()
        .strip_prefix(PREFIX)
        .unwrap_or_default()
        .to_string();
    if !allowed(&method, &rest) {
        return error(404, "not_found", "The vault has no such action.");
    }
    let query = uri.query().filter(|q| !q.is_empty());
    if query.is_some_and(|q| !q.bytes().all(safe_byte)) {
        return error(400, "bad_request", "The query has characters it may not.");
    }
    if !body.is_empty() && serde_json::from_slice::<serde_json::Value>(&body).is_err() {
        return error(400, "bad_request", "The body is not JSON.");
    }
    let passphrase = match headers.get(PASSPHRASE_HEADER).map(|v| v.to_str()) {
        None => None,
        Some(Ok(p)) if !p.chars().any(char::is_control) => Some(p.to_string()),
        Some(_) => {
            return error(
                400,
                "bad_request",
                "The passphrase has characters it may not.",
            )
        }
    };
    let target = match query {
        Some(query) => format!("/{rest}?{query}"),
        None => format!("/{rest}"),
    };
    let socket = state.admin.clone();
    let sent = tokio::task::spawn_blocking(move || {
        admin::send(&socket, &method, &target, &body, passphrase.as_deref())
    })
    .await;
    match sent {
        Ok(Ok(reply)) => {
            let kind = reply
                .header("content-type")
                .unwrap_or("application/json")
                .to_string();
            let status = StatusCode::from_u16(reply.status).unwrap_or(StatusCode::BAD_GATEWAY);
            (status, [(CONTENT_TYPE, kind)], reply.body).into_response()
        }
        Ok(Err(failure)) => error(
            502,
            "daemon_unavailable",
            &format!(
                "The vault daemon did not answer at {}: {failure:#}. Start it with ai2 setup.",
                state.admin.display()
            ),
        ),
        Err(failure) => error(500, "internal", &failure.to_string()),
    }
}

pub fn allowed(method: &str, path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.iter().any(|part| !segment(part)) {
        return false;
    }
    matches!(
        (method, parts.as_slice()),
        ("GET", ["status"])
            | ("GET", ["keys"])
            | ("GET" | "PUT" | "DELETE", ["keys", _])
            | ("POST", ["keys", _, "rotate" | "propose"])
            | ("GET" | "POST", ["tokens"])
            | ("DELETE", ["tokens", _])
            | ("GET" | "POST", ["sandboxes"])
            | ("DELETE", ["sandboxes", _])
            | ("GET", ["approvals"])
            | ("POST", ["approvals", _])
            | ("GET", ["audit"])
    )
}

fn segment(part: &str) -> bool {
    !part.is_empty() && part != "." && part != ".." && part.bytes().all(safe_byte)
}

fn safe_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"-._~%=&".contains(&byte)
}
