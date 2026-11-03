use crate::approvals::Pending;
use crate::store::{Mode, Sandbox};
use serde::Serialize;
use serde_json::Value;
use utoipa::ToSchema;

pub fn value(item: impl Serialize) -> Value {
    serde_json::to_value(item).unwrap_or(Value::Null)
}

#[derive(Serialize, ToSchema)]
pub struct KeyView {
    pub name: String,
    pub description: String,
    pub approved_urls: Vec<String>,
    pub inject: String,
    pub created: String,
    pub rotated: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct TokenView {
    pub id: String,
    pub keys: Vec<String>,
    pub sandbox: String,
    pub mode: Mode,
    pub methods: Vec<String>,
    pub url_prefixes: Vec<String>,
    pub intent: Option<String>,
    pub created: String,
    pub expires: String,
    pub uses_left: Option<u64>,
}

#[derive(Serialize, ToSchema)]
pub struct NewToken {
    #[serde(flatten)]
    pub view: TokenView,
    pub token: String,
    pub revoke: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub struct KeyOut {
    pub key: KeyView,
}

#[derive(Serialize, ToSchema)]
pub struct Keys {
    pub keys: Vec<KeyView>,
}

#[derive(Serialize, ToSchema)]
pub struct Tokens {
    pub tokens: Vec<TokenView>,
}

#[derive(Serialize, ToSchema)]
pub struct NewTokens {
    pub tokens: Vec<NewToken>,
}

#[derive(Serialize, ToSchema)]
pub struct SandboxOut {
    pub sandbox: Sandbox,
}

#[derive(Serialize, ToSchema)]
pub struct Sandboxes {
    pub sandboxes: Vec<Sandbox>,
}

#[derive(Serialize, ToSchema)]
pub struct Deleted {
    pub deleted: String,
}

#[derive(Serialize, ToSchema)]
pub struct Revoked {
    pub revoked: String,
}

#[derive(Serialize, ToSchema)]
pub struct RevokedMany {
    pub revoked: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub struct Removed {
    pub removed: String,
}

#[derive(Serialize, ToSchema)]
#[serde(untagged)]
pub enum Changed {
    Key(KeyOut),
    Token(NewToken),
    Sandbox(SandboxOut),
    Deleted(Deleted),
    Revoked(Revoked),
    Removed(Removed),
}

#[derive(Serialize, ToSchema)]
pub struct Results {
    pub results: Vec<Changed>,
}

#[derive(Serialize, ToSchema)]
pub struct Status {
    pub master_key: bool,
    pub platform: String,
    pub version: String,
    #[schema(example = "touch_id")]
    pub presence: String,
}

#[derive(Serialize, ToSchema)]
pub struct Proposed {
    pub name: String,
    pub approved_urls: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub struct Approvals {
    pub approvals: Vec<Pending>,
}

#[derive(Serialize, ToSchema)]
pub struct Decided {
    pub id: String,
    pub approved: bool,
}

#[derive(Serialize, ToSchema)]
pub struct Audit {
    pub entries: Vec<Value>,
}

#[derive(Serialize, ToSchema)]
pub struct Error {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seconds: Option<u64>,
}

#[derive(Serialize, ToSchema)]
pub struct Denied {
    #[schema(example = "denied")]
    pub error: String,
    #[schema(example = "url_not_allowed")]
    pub reason: String,
    pub intent: Option<String>,
}
