use crate::state::Shared;
use crate::{bundle, harnesses, reviews, security, status, vault};
use axum::extract::DefaultBodyLimit;
use axum::routing::get;
use axum::{middleware, Json, Router};
use tower_http::limit::RequestBodyLimitLayer;
use utoipa::openapi::OpenApi;
use utoipa::OpenApi as _;
use utoipa_axum::router::OpenApiRouter;

#[derive(utoipa::OpenApi)]
#[openapi(info(
    title = "ai2 ui",
    description = "The local ai2 server on 127.0.0.1. Every request carries the X-Ai2-UI-Nonce header."
))]
struct Doc;

pub fn api() -> OpenApiRouter<Shared> {
    OpenApiRouter::new()
        .merge(status::routes())
        .merge(vault::routes())
        .merge(crate::setup::routes())
        .merge(harnesses::routes())
        .merge(reviews::routes())
}

pub fn router(state: Shared) -> (Router, OpenApi) {
    let (api, mut document) = OpenApiRouter::with_openapi(Doc::openapi())
        .nest("/api", api())
        .split_for_parts();
    document.info.version = env!("CARGO_PKG_VERSION").to_string();
    let served = document.clone();
    let router = Router::new()
        .merge(api)
        .route("/openapi.json", get(move || async move { Json(served) }))
        .fallback(bundle::serve)
        .layer(DefaultBodyLimit::max(security::BODY_LIMIT))
        .layer(RequestBodyLimitLayer::new(security::BODY_LIMIT))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            security::guard,
        ))
        .with_state(state);
    (router, document)
}
