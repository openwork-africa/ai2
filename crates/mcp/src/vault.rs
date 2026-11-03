use crate::progress;
use crate::{Failure, Server};
use anyhow::Result;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router, Json};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use zeroize::Zeroize;

pub const PRESENCE: &str = "Approve this in ai2 ui or the CLI: the vault asks the person at the machine for every change, and an agent cannot answer for them.";

pub trait Admin: Send + Sync {
    fn call(&self, method: &str, path: &str, body: Option<&Value>) -> Result<(u16, Value)>;
}

pub struct Socket(pub PathBuf);

impl Admin for Socket {
    fn call(&self, method: &str, path: &str, body: Option<&Value>) -> Result<(u16, Value)> {
        ::vault::client::call(&self.0, method, path, body, &[])
    }
}

fn failed(status: u16, reply: &Value) -> Failure {
    let failure = Failure::from_reply(status, reply);
    if failure.error == "presence_required" {
        return Failure::new(
            "presence_required",
            format!("{} {PRESENCE}", failure.message),
        );
    }
    failure
}

const SECRETS: [&str; 3] = ["secret", "token", "revoke"];

fn forget(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        for name in SECRETS {
            if let Some(Value::String(secret)) = object.get_mut(name) {
                secret.zeroize();
            }
        }
    }
}

fn read<T: DeserializeOwned>(
    status: u16,
    reply: &Value,
    field: Option<&str>,
) -> Result<T, Failure> {
    if status != 200 {
        return Err(failed(status, reply));
    }
    let value = match field {
        Some(field) => reply.get(field).unwrap_or(&Value::Null),
        None => reply,
    };
    T::deserialize(value)
        .map_err(|error| Failure::new("bad_reply", format!("the vault answered {error}")))
}

fn segment(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

async fn ask<T: DeserializeOwned + Send + 'static>(
    admin: Arc<dyn Admin>,
    method: &'static str,
    path: String,
    body: Option<Value>,
    field: Option<&'static str>,
) -> Result<T, Failure> {
    progress::quiet(move || {
        let mut body = body;
        let reply = admin.call(method, &path, body.as_ref());
        if let Some(body) = body.as_mut() {
            forget(body);
        }
        let (status, mut reply) = reply?;
        let read = read(status, &reply, field);
        forget(&mut reply);
        read
    })
    .await?
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct VaultStatus {
    pub reachable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub master_key: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(default)]
pub struct KeyInfo {
    pub name: String,
    pub description: String,
    pub approved_urls: Vec<String>,
    pub inject: String,
    pub created: String,
    pub rotated: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct Keys {
    pub keys: Vec<KeyInfo>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct KeyFound {
    pub key: KeyInfo,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct Has {
    pub name: String,
    pub present: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    App,
    Agent,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct TokenInfo {
    pub id: String,
    pub keys: Vec<String>,
    pub sandbox: String,
    pub mode: Mode,
    #[serde(default)]
    pub methods: Vec<String>,
    #[serde(default)]
    pub url_prefixes: Vec<String>,
    #[serde(default)]
    pub intent: Option<String>,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub expires: String,
    #[serde(default)]
    pub uses_left: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct Tokens {
    pub tokens: Vec<TokenInfo>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct TokenMade {
    #[serde(flatten)]
    pub made: TokenInfo,
    pub token: String,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub withheld: String,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(default)]
pub struct SandboxInfo {
    pub name: String,
    pub transport: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listen: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub peers: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct Sandboxes {
    pub sandboxes: Vec<SandboxInfo>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct SandboxAdded {
    pub sandbox: SandboxInfo,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(default)]
pub struct Approval {
    pub id: String,
    #[serde(alias = "token")]
    pub token_id: String,
    pub sandbox: String,
    pub key: String,
    pub method: String,
    pub url: String,
    pub intent: String,
    pub line: String,
    pub created: String,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct Approvals {
    pub approvals: Vec<Approval>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct Decided {
    pub id: String,
    pub approved: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(default)]
pub struct AuditEntry {
    pub time: String,
    #[serde(alias = "token")]
    pub token_id: Option<String>,
    pub sandbox: String,
    pub key: Option<String>,
    pub method: String,
    pub url: Option<String>,
    pub decision: String,
    pub status: u16,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct Audit {
    pub entries: Vec<AuditEntry>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct Gone {
    pub gone: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct Name {
    pub name: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct PutKey {
    pub name: String,
    pub secret: String,
    #[serde(default)]
    pub description: String,
    pub approved_urls: Vec<String>,
    #[serde(default)]
    pub inject: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct RotateKey {
    pub name: String,
    pub secret: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct CreateToken {
    pub keys: Vec<String>,
    pub sandbox: String,
    pub mode: Mode,
    #[serde(default)]
    pub methods: Vec<String>,
    #[serde(default)]
    pub url_prefixes: Vec<String>,
    #[serde(default)]
    pub intent: Option<String>,
    #[serde(default)]
    pub ttl: Option<String>,
    #[serde(default)]
    pub uses: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct TokenId {
    pub id: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AddSandbox {
    pub name: String,
    #[serde(default)]
    pub socket: Option<String>,
    #[serde(default)]
    pub tcp: Option<String>,
    #[serde(default)]
    pub peers: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct Decide {
    pub id: String,
    pub approve: bool,
}

#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct AuditAsk {
    #[serde(default)]
    pub tail: Option<usize>,
}

pub const WITHHELD: &str = "The token works only from its sandbox, for its keys, methods and URL prefixes. Its one-time revoke value is never returned to an MCP client; ai2 ui and ai2 vault token create show it once. Revoke this token with vault_revoke_token when it is no longer needed.";

impl Server {
    fn admin(&self) -> Arc<dyn Admin> {
        self.pieces.vault.clone()
    }
}

#[tool_router(router = vault_tools, vis = "pub(crate)")]
impl Server {
    #[tool(
        name = "vault_status",
        description = "Whether the vault daemon is reachable, has its master key, and its platform and version"
    )]
    async fn vault_status(&self) -> Result<Json<VaultStatus>, Failure> {
        let admin = self.admin();
        let status = progress::quiet(move || match admin.call("GET", "/status", None) {
            Ok((200, reply)) => VaultStatus {
                reachable: true,
                ..serde_json::from_value(reply).unwrap_or_default()
            },
            Ok((status, reply)) => VaultStatus {
                message: Some(failed(status, &reply).message),
                ..VaultStatus::default()
            },
            Err(error) => VaultStatus {
                message: Some(format!("{error:#}")),
                ..VaultStatus::default()
            },
        })
        .await?;
        Ok(Json(status))
    }

    #[tool(
        name = "vault_list_keys",
        description = "Every vault key's name, description, approved URLs and how it is injected; never the secret"
    )]
    async fn vault_list_keys(&self) -> Result<Json<Keys>, Failure> {
        ask(self.admin(), "GET", "/keys".into(), None, None)
            .await
            .map(Json)
    }

    #[tool(
        name = "vault_has_key",
        description = "Whether the vault holds a key by this name"
    )]
    async fn vault_has_key(
        &self,
        Parameters(asked): Parameters<Name>,
    ) -> Result<Json<Has>, Failure> {
        let found: Result<KeyFound, Failure> = ask(
            self.admin(),
            "GET",
            format!("/keys/{}", segment(&asked.name)),
            None,
            None,
        )
        .await;
        match found {
            Ok(_) => Ok(Json(Has {
                name: asked.name,
                present: true,
            })),
            Err(failure) if failure.error == "not_found" => Ok(Json(Has {
                name: asked.name,
                present: false,
            })),
            Err(failure) => Err(failure),
        }
    }

    #[tool(
        name = "vault_put_key",
        description = "Stores or replaces a vault key, sent only to its approved URLs. Prefer ai2 ui, which asks for the secret without an agent seeing it. The person at the machine approves it; on Linux it answers presence_required, to be approved in ai2 ui or the CLI"
    )]
    async fn vault_put_key(
        &self,
        Parameters(mut asked): Parameters<PutKey>,
    ) -> Result<Json<KeyFound>, Failure> {
        let mut body = json!({
            "secret": asked.secret.as_str(),
            "description": asked.description,
            "approved_urls": asked.approved_urls,
        });
        asked.secret.zeroize();
        if let Some(inject) = asked.inject.take() {
            body["inject"] = json!(inject);
        }
        ask(
            self.admin(),
            "PUT",
            format!("/keys/{}", segment(&asked.name)),
            Some(body),
            None,
        )
        .await
        .map(Json)
    }

    #[tool(
        name = "vault_rotate_key",
        description = "Replaces the secret of a vault key, keeping its approved URLs. Prefer ai2 ui. The person at the machine approves it; on Linux it answers presence_required"
    )]
    async fn vault_rotate_key(
        &self,
        Parameters(mut asked): Parameters<RotateKey>,
    ) -> Result<Json<KeyFound>, Failure> {
        let body = json!({ "secret": asked.secret.as_str() });
        asked.secret.zeroize();
        ask(
            self.admin(),
            "POST",
            format!("/keys/{}/rotate", segment(&asked.name)),
            Some(body),
            None,
        )
        .await
        .map(Json)
    }

    #[tool(
        name = "vault_delete_key",
        description = "Deletes a vault key. The person at the machine approves it; on Linux it answers presence_required"
    )]
    async fn vault_delete_key(
        &self,
        Parameters(asked): Parameters<Name>,
    ) -> Result<Json<Gone>, Failure> {
        ask(
            self.admin(),
            "DELETE",
            format!("/keys/{}", segment(&asked.name)),
            None,
            Some("deleted"),
        )
        .await
        .map(|gone| Json(Gone { gone }))
    }

    #[tool(
        name = "vault_list_tokens",
        description = "Every vault token: its keys, sandbox, mode, methods, URL prefixes, intent and expiry; never the token itself"
    )]
    async fn vault_list_tokens(&self) -> Result<Json<Tokens>, Failure> {
        ask(self.admin(), "GET", "/tokens".into(), None, None)
            .await
            .map(Json)
    }

    #[tool(
        name = "vault_create_token",
        description = "Makes a vault token (vt_...) for keys in a sandbox and returns it, once the person at the machine approves it; on Linux it answers presence_required. The token works only from that sandbox, for those keys; its one-time revoke value is withheld"
    )]
    async fn vault_create_token(
        &self,
        Parameters(asked): Parameters<CreateToken>,
    ) -> Result<Json<TokenMade>, Failure> {
        let mut body = serde_json::to_value(&asked)
            .map_err(|error| Failure::new("bad_request", error.to_string()))?;
        if let Some(body) = body.as_object_mut().filter(|_| asked.methods.is_empty()) {
            body.remove("methods");
        }
        let mut made: TokenMade =
            ask(self.admin(), "POST", "/tokens".into(), Some(body), None).await?;
        made.withheld = WITHHELD.into();
        Ok(Json(made))
    }

    #[tool(
        name = "vault_revoke_token",
        description = "Revokes a vault token by its id. The person at the machine approves it; on Linux it answers presence_required"
    )]
    async fn vault_revoke_token(
        &self,
        Parameters(asked): Parameters<TokenId>,
    ) -> Result<Json<Gone>, Failure> {
        ask(
            self.admin(),
            "DELETE",
            format!("/tokens/{}", segment(&asked.id)),
            None,
            Some("revoked"),
        )
        .await
        .map(|gone| Json(Gone { gone }))
    }

    #[tool(
        name = "vault_list_sandboxes",
        description = "Every vault sandbox and the socket or TCP address its gateway listens on"
    )]
    async fn vault_list_sandboxes(&self) -> Result<Json<Sandboxes>, Failure> {
        ask(self.admin(), "GET", "/sandboxes".into(), None, None)
            .await
            .map(Json)
    }

    #[tool(
        name = "vault_add_sandbox",
        description = "Adds a vault sandbox on a Unix socket, or on TCP with the peer addresses it takes calls from. The person at the machine approves it; on Linux it answers presence_required"
    )]
    async fn vault_add_sandbox(
        &self,
        Parameters(asked): Parameters<AddSandbox>,
    ) -> Result<Json<SandboxAdded>, Failure> {
        let body = serde_json::to_value(&asked)
            .map_err(|error| Failure::new("bad_request", error.to_string()))?;
        ask(self.admin(), "POST", "/sandboxes".into(), Some(body), None)
            .await
            .map(Json)
    }

    #[tool(
        name = "vault_remove_sandbox",
        description = "Removes a vault sandbox; its tokens stop working. The person at the machine approves it; on Linux it answers presence_required"
    )]
    async fn vault_remove_sandbox(
        &self,
        Parameters(asked): Parameters<Name>,
    ) -> Result<Json<Gone>, Failure> {
        ask(
            self.admin(),
            "DELETE",
            format!("/sandboxes/{}", segment(&asked.name)),
            None,
            Some("removed"),
        )
        .await
        .map(|gone| Json(Gone { gone }))
    }

    #[tool(
        name = "vault_list_approvals",
        description = "The gateway calls waiting for the person at the machine to allow or refuse them"
    )]
    async fn vault_list_approvals(&self) -> Result<Json<Approvals>, Failure> {
        ask(self.admin(), "GET", "/approvals".into(), None, None)
            .await
            .map(Json)
    }

    #[tool(
        name = "vault_decide_approval",
        description = "Allows or refuses a waiting gateway call. The person at the machine approves it; on Linux it answers presence_required"
    )]
    async fn vault_decide_approval(
        &self,
        Parameters(asked): Parameters<Decide>,
    ) -> Result<Json<Decided>, Failure> {
        ask(
            self.admin(),
            "POST",
            format!("/approvals/{}", segment(&asked.id)),
            Some(json!({ "approve": asked.approve })),
            None,
        )
        .await
        .map(Json)
    }

    #[tool(
        name = "vault_audit",
        description = "The last gateway calls (50 unless tail says): time, token id, sandbox, key, method, URL and whether each was allowed"
    )]
    async fn vault_audit(
        &self,
        Parameters(asked): Parameters<AuditAsk>,
    ) -> Result<Json<Audit>, Failure> {
        let path = match asked.tail {
            Some(tail) => format!("/audit?tail={tail}"),
            None => "/audit".into(),
        };
        ask(self.admin(), "GET", path, None, None).await.map(Json)
    }
}
