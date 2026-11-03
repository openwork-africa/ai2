use crate::admin;
use crate::state::Shared;
use axum::extract::State;
use axum::Json;
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

pub fn features() -> Vec<&'static str> {
    vec!["apps", "vault"]
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Status {
    pub version: String,
    pub features: Vec<String>,
    pub vault: VaultStatus,
    pub setup: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct VaultStatus {
    pub reachable: bool,
    pub version: Option<String>,
    pub master_key: Option<bool>,
}

pub fn routes() -> OpenApiRouter<Shared> {
    OpenApiRouter::new().routes(routes!(status))
}

#[utoipa::path(
    get,
    path = "/status",
    tag = "ui",
    responses((status = 200, description = "What this ai2 ui serves and whether the vault answers", body = Status))
)]
async fn status(State(state): State<Shared>) -> Json<Status> {
    let socket = state.admin.clone();
    let vault = tokio::task::spawn_blocking(move || vault_status(&socket))
        .await
        .unwrap_or(VaultStatus {
            reachable: false,
            version: None,
            master_key: None,
        });
    Json(Status {
        version: env!("CARGO_PKG_VERSION").to_string(),
        features: features().into_iter().map(String::from).collect(),
        vault,
        setup: "unknown".to_string(),
    })
}

fn vault_status(socket: &std::path::Path) -> VaultStatus {
    let reply = admin::send(socket, "GET", "/status", &[], None)
        .ok()
        .filter(|reply| reply.status == 200)
        .and_then(|reply| serde_json::from_slice::<serde_json::Value>(&reply.body).ok());
    match reply {
        Some(value) => VaultStatus {
            reachable: true,
            version: value["version"].as_str().map(String::from),
            master_key: value["master_key"].as_bool(),
        },
        None => VaultStatus {
            reachable: false,
            version: None,
            master_key: None,
        },
    }
}
