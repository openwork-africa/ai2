use crate::state::Shared;
use axum::extract::Request;
use axum::http::HeaderMap;

pub const PAIR: &str = "/mcp/pair";

pub fn open(path: &str) -> bool {
    cfg!(feature = "mcp") && path == PAIR
}

#[cfg(feature = "mcp")]
pub fn caller(state: &Shared, path: &str, headers: &HeaderMap) -> Option<String> {
    if path != "/mcp" && !path.starts_with("/mcp/") {
        return None;
    }
    let given = crate::security::single(headers, "authorization")?;
    let (scheme, token) = given.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    state.clients.check(token.trim(), mcp::clients::now())
}

#[cfg(not(feature = "mcp"))]
pub fn caller(_state: &Shared, _path: &str, _headers: &HeaderMap) -> Option<String> {
    None
}

#[cfg(feature = "mcp")]
pub fn mark(request: &mut Request, caller: Option<String>) {
    if let Some(token) = caller {
        request
            .extensions_mut()
            .insert(mcp::clients::Caller { token });
    }
}

#[cfg(not(feature = "mcp"))]
pub fn mark(_request: &mut Request, _caller: Option<String>) {}
