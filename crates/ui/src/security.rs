use crate::nonce::same;
use crate::reply::error;
use crate::state::Shared;
use axum::extract::{Request, State};
use axum::http::header::{HeaderName, HeaderValue, HOST, ORIGIN};
use axum::http::{HeaderMap, Method};
use axum::middleware::Next;
use axum::response::Response;

pub const NONCE_HEADER: &str = "X-Ai2-UI-Nonce";
pub const BODY_LIMIT: usize = 4 * 1024 * 1024;
pub const POLICY: &str = "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data: blob:; font-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

const HEADERS: [(&str, &str); 6] = [
    ("content-security-policy", POLICY),
    ("cache-control", "no-store"),
    ("x-content-type-options", "nosniff"),
    ("referrer-policy", "no-referrer"),
    ("x-frame-options", "DENY"),
    ("cross-origin-resource-policy", "same-origin"),
];

pub async fn guard(State(state): State<Shared>, request: Request, next: Next) -> Response {
    let response = match check(&state, &request) {
        Some(refused) => refused,
        None => next.run(request).await,
    };
    secure(response)
}

fn check(state: &Shared, request: &Request) -> Option<Response> {
    let headers = request.headers();
    if single(headers, HOST.as_str()) != Some(state.host.as_str()) {
        return Some(forbidden("The Host header does not match this server."));
    }
    let origin = headers.get_all(ORIGIN).iter().collect::<Vec<_>>();
    if origin.len() > 1
        || origin
            .iter()
            .any(|o| o.as_bytes() != state.origin.as_bytes())
    {
        return Some(forbidden("The Origin does not match this server."));
    }
    let writes = !matches!(*request.method(), Method::GET | Method::HEAD);
    if writes && origin.is_empty() {
        return Some(forbidden("A change must come from this page."));
    }
    if guarded(request.uri().path()) {
        let given = single(headers, NONCE_HEADER).unwrap_or_default();
        if !same(given, &state.nonce) {
            return Some(forbidden(
                "The request does not carry this page's nonce. Open the address ai2 ui printed.",
            ));
        }
    }
    None
}

pub fn guarded(path: &str) -> bool {
    path == "/api"
        || path.starts_with("/api/")
        || path == "/mcp"
        || path.starts_with("/mcp/")
        || path == "/openapi.json"
}

fn single<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut values = headers.get_all(name).iter();
    let first = values.next()?;
    if values.next().is_some() {
        return None;
    }
    first.to_str().ok()
}

fn secure(mut response: Response) -> Response {
    let headers = response.headers_mut();
    for (name, value) in HEADERS {
        headers.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    response
}

pub fn forbidden(message: &str) -> Response {
    error(403, "forbidden", message)
}
