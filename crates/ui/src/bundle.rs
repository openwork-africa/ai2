use crate::nonce::same;
use crate::reply::error;
use crate::security::{forbidden, guarded};
use crate::state::Shared;
use axum::extract::State;
use axum::http::header::CONTENT_TYPE;
use axum::http::{Method, Uri};
use axum::response::{IntoResponse, Response};
use include_dir::{include_dir, Dir};

static DIST: Dir = include_dir!("$CARGO_MANIFEST_DIR/dist");

pub const INDEX: &str = "index.html";

pub async fn serve(State(state): State<Shared>, method: Method, uri: Uri) -> Response {
    if guarded(uri.path()) {
        return error(404, "not_found", "There is nothing here.");
    }
    if !matches!(method, Method::GET | Method::HEAD) {
        return error(405, "method_not_allowed", "The page only answers GET.");
    }
    let path = uri.path().trim_start_matches('/');
    if path != INDEX {
        if let Some(file) = DIST.get_file(path) {
            return file_response(path, file.contents());
        }
    }
    if path
        .rsplit('/')
        .next()
        .is_some_and(|last| last.contains('.'))
    {
        return error(404, "not_found", "There is no such file.");
    }
    if let Some(given) = query_nonce(uri.query()) {
        if !same(given, &state.nonce) {
            return forbidden("Open the address ai2 ui printed.");
        }
    }
    match DIST.get_file(INDEX) {
        Some(index) => file_response(INDEX, index.contents()),
        None => error(500, "bundle_missing", "The web bundle was not built."),
    }
}

fn query_nonce(query: Option<&str>) -> Option<&str> {
    query?
        .split('&')
        .find_map(|pair| pair.strip_prefix("nonce="))
}

fn file_response(path: &str, contents: &'static [u8]) -> Response {
    let kind = mime_guess::from_path(path).first_or_octet_stream();
    let textual = kind.type_() == mime_guess::mime::TEXT
        || matches!(kind.subtype().as_str(), "javascript" | "json");
    let kind = if textual && kind.get_param("charset").is_none() {
        format!("{kind}; charset=utf-8")
    } else {
        kind.to_string()
    };
    ([(CONTENT_TYPE, kind)], contents).into_response()
}
