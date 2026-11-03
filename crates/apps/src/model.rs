use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use utoipa::ToSchema;

#[derive(
    Serialize,
    Deserialize,
    JsonSchema,
    ToSchema,
    clap::ValueEnum,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Debug,
    Default,
)]
#[serde(rename_all = "kebab-case")]
pub enum Restart {
    #[default]
    Never,
    OnFailure,
    Always,
}

#[derive(Serialize, Deserialize, JsonSchema, ToSchema, Clone, PartialEq, Debug)]
pub struct Icon {
    #[serde(rename = "type")]
    pub content_type: String,
    pub file: String,
    pub bytes: u64,
}

#[derive(Serialize, Deserialize, JsonSchema, ToSchema, Clone, PartialEq, Debug)]
pub struct App {
    pub id: String,
    pub name: String,
    pub description: String,
    pub command: String,
    pub cwd: String,
    pub port: u16,
    pub url: String,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default = "yes")]
    pub set_port: bool,
    #[serde(default)]
    pub restart: Restart,
    pub icon: Option<Icon>,
    pub openapi: String,
    pub added: String,
    pub updated: String,
}

fn yes() -> bool {
    true
}

#[derive(Serialize, Deserialize, JsonSchema, ToSchema, Clone, PartialEq, Debug, Default)]
pub struct IconUpload {
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema, ToSchema, Clone, PartialEq, Debug, Default)]
pub struct NewApp {
    pub name: String,
    pub description: String,
    pub command: String,
    pub openapi: String,
    pub icon: Option<IconUpload>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set_port: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restart: Option<Restart>,
}

#[derive(Serialize, Deserialize, JsonSchema, ToSchema, Clone, PartialEq, Debug, Default)]
pub struct AppPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openapi: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<IconUpload>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set_port: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restart: Option<Restart>,
}

#[derive(
    Serialize, Deserialize, JsonSchema, ToSchema, Clone, Copy, PartialEq, Eq, Debug, Default,
)]
#[serde(rename_all = "lowercase")]
pub enum ApiState {
    #[default]
    Unchecked,
    Valid,
    Unreachable,
    Invalid,
}

#[derive(Serialize, Deserialize, JsonSchema, ToSchema, Clone, PartialEq, Debug, Default)]
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

#[derive(Serialize, Deserialize, JsonSchema, ToSchema, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Exit {
    pub code: Option<i32>,
    pub signal: Option<i32>,
}

impl Exit {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

#[derive(Serialize, Deserialize, JsonSchema, ToSchema, Clone, PartialEq, Debug, Default)]
pub struct Status {
    pub running: bool,
    pub started_here: bool,
    pub pid: Option<u32>,
    pub since: Option<String>,
    pub restarts: u32,
    pub last_exit: Option<Exit>,
    pub crashed: bool,
}

#[derive(Serialize, Deserialize, JsonSchema, ToSchema, Clone, PartialEq, Debug)]
pub struct Shown {
    #[serde(flatten)]
    pub app: App,
    #[serde(flatten)]
    pub status: Status,
    pub api: Api,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    pub document: Option<Value>,
}

#[derive(Serialize, Deserialize, JsonSchema, ToSchema, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Stream {
    Out,
    Err,
    Log,
}

#[derive(Serialize, Deserialize, JsonSchema, ToSchema, Clone, PartialEq, Debug)]
pub struct LogLine {
    pub t: Option<u64>,
    pub stream: Stream,
    pub line: String,
}

#[derive(Serialize, Deserialize, JsonSchema, ToSchema, Clone, PartialEq, Debug)]
pub struct AppEvent {
    pub id: String,
    pub running: bool,
    pub started_here: bool,
    pub pid: Option<u32>,
}
