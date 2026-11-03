use crate::reply::error;
use crate::state::Shared;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use mcp::clients::{now, Client, Code, Token};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

pub fn pieces(state: &Shared) -> mcp::Pieces {
    let bin = std::env::current_exe().unwrap_or_else(|_| "ai2".into());
    let mut pieces = mcp::Pieces::local("ai2", env!("CARGO_PKG_VERSION"), bin, state.home.clone());
    pieces.apps = Arc::new(mcp::apps::Local::in_process(state.apps.clone()));
    pieces.vault = Arc::new(mcp::vault::Socket(state.admin.clone()));
    pieces.agents = state.agents.clone();
    pieces.clients = state.clients.clone();
    pieces
}

fn endpoint(state: &Shared) -> String {
    format!("{}/mcp", state.origin)
}

pub fn mount(router: Router<Shared>, state: &Shared) -> Router<Shared> {
    let pieces = pieces(state);
    let document = mcp::Server::new(pieces.clone()).openapi(&endpoint(state));
    router
        .route(
            "/mcp/openapi.json",
            get(move || async move { Json(document) }),
        )
        .route(crate::mcp_auth::PAIR, post(pair))
        .nest_service("/mcp", mcp::http_service(pieces, state.port))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpClient {
    pub name: String,
    pub version: String,
    pub transport: String,
    pub token: Option<String>,
    pub first_seen: u64,
    pub last_seen: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpToken {
    pub id: String,
    pub name: String,
    pub created: u64,
    pub last_used: Option<u64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PairingCode {
    pub code: String,
    pub expires: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Connections {
    pub clients: Vec<McpClient>,
    pub tokens: Vec<McpToken>,
    pub pairing: Option<PairingCode>,
    pub url: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Revoked {
    pub revoked: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct Pair {
    pub code: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Paired {
    pub id: String,
    pub token: String,
    pub url: String,
    pub header: String,
}

impl From<Client> for McpClient {
    fn from(client: Client) -> Self {
        Self {
            name: client.name,
            version: client.version,
            transport: client.transport,
            token: client.token,
            first_seen: client.first_seen,
            last_seen: client.last_seen,
        }
    }
}

impl From<Token> for McpToken {
    fn from(token: Token) -> Self {
        Self {
            id: token.id,
            name: token.name,
            created: token.created,
            last_used: token.last_used,
        }
    }
}

impl From<Code> for PairingCode {
    fn from(code: Code) -> Self {
        Self {
            code: code.code,
            expires: code.expires,
        }
    }
}

pub fn routes() -> OpenApiRouter<Shared> {
    OpenApiRouter::new()
        .routes(routes!(connections))
        .routes(routes!(pairing))
        .routes(routes!(revoke))
}

fn failed(what: &str, failure: anyhow::Error) -> Response {
    error(500, "mcp_clients", &format!("{what}: {failure:#}"))
}

#[utoipa::path(
    get,
    path = "/mcp/clients",
    tag = "mcp",
    responses((status = 200, description = "The MCP clients seen so far, the HTTP client tokens and the waiting pairing code", body = Connections))
)]
async fn connections(State(state): State<Shared>) -> Response {
    let read = tokio::task::spawn_blocking({
        let clients = state.clients.clone();
        move || Ok::<_, anyhow::Error>((clients.clients()?, clients.tokens()?))
    })
    .await;
    match read {
        Ok(Ok((clients, tokens))) => Json(Connections {
            clients: clients.into_iter().map(McpClient::from).collect(),
            tokens: tokens.into_iter().map(McpToken::from).collect(),
            pairing: state.pairing.waiting(now()).map(PairingCode::from),
            url: endpoint(&state),
        })
        .into_response(),
        Ok(Err(failure)) => failed("reading the MCP clients", failure),
        Err(failure) => failed("reading the MCP clients", failure.into()),
    }
}

#[utoipa::path(
    post,
    path = "/mcp/pairing",
    tag = "mcp",
    responses((status = 200, description = "A new pairing code, good once for ten minutes; it replaces any code before it", body = PairingCode))
)]
async fn pairing(State(state): State<Shared>) -> Response {
    match state.pairing.issue(now()) {
        Ok(code) => Json(PairingCode::from(code)).into_response(),
        Err(failure) => failed("making a pairing code", failure),
    }
}

#[utoipa::path(
    delete,
    path = "/mcp/tokens/{id}",
    tag = "mcp",
    params(("id" = String, Path, description = "The token id, as the clients list names it")),
    responses(
        (status = 200, description = "The token no longer opens /mcp", body = Revoked),
        (status = 404, description = "There is no such token", body = crate::Reply)
    )
)]
async fn revoke(State(state): State<Shared>, Path(id): Path<String>) -> Response {
    let clients = state.clients.clone();
    let asked = id.clone();
    match tokio::task::spawn_blocking(move || clients.revoke(&asked)).await {
        Ok(Ok(true)) => Json(Revoked { revoked: id }).into_response(),
        Ok(Ok(false)) => error(404, "not_found", "There is no such token."),
        Ok(Err(failure)) => failed("revoking the token", failure),
        Err(failure) => failed("revoking the token", failure.into()),
    }
}

async fn pair(State(state): State<Shared>, Json(asked): Json<Pair>) -> Response {
    if let Err(refusal) = state.pairing.take(&asked.code, now()) {
        return error(403, "pairing_refused", refusal.message());
    }
    let clients = state.clients.clone();
    let name = asked.name.unwrap_or_default();
    match tokio::task::spawn_blocking(move || clients.mint(&name, now())).await {
        Ok(Ok(minted)) => Json(Paired {
            id: minted.token.id,
            header: format!("Authorization: Bearer {}", minted.secret.as_str()),
            token: minted.secret.to_string(),
            url: endpoint(&state),
        })
        .into_response(),
        Ok(Err(failure)) => failed("making the token", failure),
        Err(failure) => failed("making the token", failure.into()),
    }
}
