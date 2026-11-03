use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct Reply {
    pub error: String,
    pub message: String,
}

pub fn error(status: u16, code: &str, message: &str) -> Response {
    let status = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (
        status,
        Json(Reply {
            error: code.to_string(),
            message: message.to_string(),
        }),
    )
        .into_response()
}
