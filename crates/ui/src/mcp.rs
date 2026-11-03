use crate::state::Shared;
use axum::routing::get;
use axum::{Json, Router};
use std::sync::Arc;

pub fn pieces(state: &Shared) -> mcp::Pieces {
    let bin = std::env::current_exe().unwrap_or_else(|_| "ai2".into());
    let mut pieces = mcp::Pieces::local("ai2", env!("CARGO_PKG_VERSION"), bin, state.home.clone());
    pieces.apps = Arc::new(mcp::apps::Local::in_process(state.apps.clone()));
    pieces.vault = Arc::new(mcp::vault::Socket(state.admin.clone()));
    pieces.agents = state.agents.clone();
    pieces
}

pub fn mount(router: Router<Shared>, state: &Shared) -> Router<Shared> {
    let pieces = pieces(state);
    let document =
        mcp::Server::new(pieces.clone()).openapi(&format!("http://127.0.0.1:{}/mcp", state.port));
    router
        .route(
            "/mcp/openapi.json",
            get(move || async move { Json(document) }),
        )
        .nest_service("/mcp", mcp::http_service(pieces, state.port))
}
