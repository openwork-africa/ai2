use crate::{Failure, Server};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router, Json};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;

pub type Answer<'a, T> = Pin<Box<dyn Future<Output = Result<T, Failure>> + Send + 'a>>;

pub trait Apps: Send + Sync {
    fn register(&self, app: NewApp) -> Answer<'_, Registered>;
    fn list(&self, include_openapi: bool) -> Answer<'_, Vec<Shown>>;
    fn start<'a>(&'a self, id: &'a str) -> Answer<'a, Shown>;
    fn stop<'a>(&'a self, id: &'a str) -> Answer<'a, Shown>;
    fn status<'a>(&'a self, id: &'a str) -> Answer<'a, Shown>;
    fn logs<'a>(&'a self, id: &'a str, tail: usize) -> Answer<'a, Vec<LogLine>>;
}

pub struct Unavailable;

fn unavailable<'a, T: Send + 'a>() -> Answer<'a, T> {
    Box::pin(async {
        Err(Failure::new(
            "apps_unavailable",
            "this build has no apps store yet",
        ))
    })
}

impl Apps for Unavailable {
    fn register(&self, _app: NewApp) -> Answer<'_, Registered> {
        unavailable()
    }
    fn list(&self, _include_openapi: bool) -> Answer<'_, Vec<Shown>> {
        unavailable()
    }
    fn start<'a>(&'a self, _id: &'a str) -> Answer<'a, Shown> {
        unavailable()
    }
    fn stop<'a>(&'a self, _id: &'a str) -> Answer<'a, Shown> {
        unavailable()
    }
    fn status<'a>(&'a self, _id: &'a str) -> Answer<'a, Shown> {
        unavailable()
    }
    fn logs<'a>(&'a self, _id: &'a str, _tail: usize) -> Answer<'a, Vec<LogLine>> {
        unavailable()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Restart {
    #[default]
    Never,
    OnFailure,
    Always,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct Icon {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct NewApp {
    pub name: String,
    pub command: String,
    pub openapi: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub set_port: Option<bool>,
    #[serde(default)]
    pub restart: Option<Restart>,
    #[serde(default)]
    pub icon: Option<Icon>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct App {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub command: String,
    pub cwd: String,
    pub port: u16,
    pub url: String,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    pub set_port: bool,
    pub restart: Restart,
    pub icon: Option<Icon>,
    pub openapi: String,
    pub added: String,
    pub updated: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ApiState {
    Unchecked,
    Valid,
    Unreachable,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct Api {
    pub openapi: String,
    pub href: String,
    pub state: ApiState,
    pub checked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operations: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct Shown {
    #[serde(flatten)]
    pub app: App,
    pub running: bool,
    pub started_here: bool,
    pub pid: Option<u32>,
    pub since: Option<String>,
    #[serde(default)]
    pub restarts: u32,
    pub api: Api,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct Registered {
    pub app: Shown,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct LogLine {
    pub t: String,
    pub stream: String,
    pub line: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct AppList {
    pub apps: Vec<Shown>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct AppFound {
    pub app: Shown,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Logs {
    pub lines: Vec<LogLine>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AppId {
    pub id: String,
}

#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct ListAsk {
    #[serde(default)]
    pub include_openapi: bool,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct LogsAsk {
    pub id: String,
    #[serde(default)]
    pub tail: Option<usize>,
}

#[tool_router(router = apps_tools, vis = "pub(crate)")]
impl Server {
    #[tool(
        name = "apps_register",
        description = "Registers a local app, or updates the one with the same name: its command, folder, port and the path of its OpenAPI document, which is required"
    )]
    async fn apps_register(
        &self,
        Parameters(asked): Parameters<NewApp>,
    ) -> Result<Json<Registered>, Failure> {
        if asked.openapi.trim().is_empty() {
            return Err(Failure::new(
                "openapi_required",
                "Send the path of the app's OpenAPI document, e.g. /openapi.json, or a file.",
            ));
        }
        self.pieces.apps.register(asked).await.map(Json)
    }

    #[tool(
        name = "apps_list",
        description = "Every local app: whether it runs, whether ai2 started it, and its API block; with include_openapi, each app's OpenAPI document too, so one call says what runs locally and how to call it"
    )]
    async fn apps_list(
        &self,
        Parameters(asked): Parameters<ListAsk>,
    ) -> Result<Json<AppList>, Failure> {
        let apps = self.pieces.apps.list(asked.include_openapi).await?;
        Ok(Json(AppList { apps }))
    }

    #[tool(
        name = "apps_start",
        description = "Starts a registered app and waits for its port to open; needs ai2 ui running"
    )]
    async fn apps_start(
        &self,
        Parameters(asked): Parameters<AppId>,
    ) -> Result<Json<AppFound>, Failure> {
        let app = self.pieces.apps.start(&asked.id).await?;
        Ok(Json(AppFound { app }))
    }

    #[tool(
        name = "apps_stop",
        description = "Stops an app ai2 started; one it did not start is refused with not_ours"
    )]
    async fn apps_stop(
        &self,
        Parameters(asked): Parameters<AppId>,
    ) -> Result<Json<AppFound>, Failure> {
        let app = self.pieces.apps.stop(&asked.id).await?;
        Ok(Json(AppFound { app }))
    }

    #[tool(
        name = "apps_status",
        description = "Whether an app runs, its pid, since when and its port"
    )]
    async fn apps_status(
        &self,
        Parameters(asked): Parameters<AppId>,
    ) -> Result<Json<AppFound>, Failure> {
        let app = self.pieces.apps.status(&asked.id).await?;
        Ok(Json(AppFound { app }))
    }

    #[tool(
        name = "apps_logs",
        description = "The last lines an app wrote (200 unless tail says)"
    )]
    async fn apps_logs(
        &self,
        Parameters(asked): Parameters<LogsAsk>,
    ) -> Result<Json<Logs>, Failure> {
        let lines = self
            .pieces
            .apps
            .logs(&asked.id, asked.tail.unwrap_or(200))
            .await?;
        Ok(Json(Logs { lines }))
    }
}
