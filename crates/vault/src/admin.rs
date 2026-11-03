use crate::api::{
    self, Approvals, Audit, Changed, Decided, Deleted, Error, KeyOut, KeyView, Keys, NewToken,
    NewTokens, Proposed, Removed, Results, Revoked, RevokedMany, SandboxOut, Sandboxes, Status,
    Tokens,
};
use crate::changes::{
    self, Action, AddSandbox, Batch, CreateToken, Name, NamedKey, NamedRotate, PutKey, Rotate,
    TokenId,
};
use crate::crypto;
use crate::daemon::{Absent, Vault};
use crate::http::Request;
use crate::openapi;
use crate::policy;
use crate::store::{Endpoint, KeyRecord, Mode, Sandbox, TokenRecord};
use crate::vet::KeyInfo;
use keys::Caller;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use utoipa::{OpenApi, ToSchema};

pub struct Failure {
    status: u16,
    code: &'static str,
    message: String,
    seconds: Option<u64>,
}

type Reply = Result<Value, Failure>;

pub fn fail(status: u16, code: &'static str, message: impl Into<String>) -> Failure {
    Failure {
        status,
        code,
        message: message.into(),
        seconds: None,
    }
}

pub fn missing(what: &str, name: &str) -> Failure {
    fail(404, "not_found", format!("there is no {what} {name}"))
}

pub fn bad(message: impl Into<String>) -> Failure {
    fail(400, "bad_request", message)
}

pub fn internal(error: anyhow::Error) -> Failure {
    fail(500, "internal", format!("{error:#}"))
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "ai2 vault admin",
        description = "The vault daemon's admin socket. Changes need a human present: Touch ID on macOS, polkit or X-Vault-Passphrase on Linux."
    ),
    paths(
        status,
        list_keys,
        put_keys,
        get_key,
        put_key,
        delete_key,
        rotate_key,
        propose,
        list_tokens,
        create_tokens,
        revoke_tokens,
        revoke_token,
        list_sandboxes,
        add_sandboxes,
        remove_sandbox,
        batch,
        list_approvals,
        decide,
        audit,
        admin_document,
        gateway_document
    ),
    components(schemas(
        Status,
        Keys,
        KeyOut,
        KeyView,
        NewToken,
        NewTokens,
        Tokens,
        Sandboxes,
        SandboxOut,
        Sandbox,
        Endpoint,
        Mode,
        Deleted,
        Revoked,
        RevokedMany,
        Removed,
        Changed,
        Results,
        Proposed,
        Approvals,
        Decided,
        Audit,
        Error,
        PutKey,
        NamedKey,
        Rotate,
        CreateToken,
        AddSandbox,
        Action,
        Batch,
        Propose,
        Decide,
        TokenBody,
        SandboxBody
    ))
)]
struct Document;

pub fn document() -> utoipa::openapi::OpenApi {
    Document::openapi()
}

struct Asked<'a> {
    vault: &'a Arc<Vault>,
    request: &'a Request,
    caller: &'a Caller,
}

impl Asked<'_> {
    fn present(&self, action: &str) -> Result<(), Failure> {
        present(self.vault, self.request, self.caller, action)
    }

    fn change(&self, actions: Vec<Action>) -> Result<Vec<Value>, Failure> {
        changes::run(self.vault, actions, |action| self.present(action))
    }

    fn one(&self, action: Action) -> Reply {
        Ok(self.change(vec![action])?.swap_remove(0))
    }

    fn many(&self, actions: Vec<Action>, field: &str, part: Option<&str>) -> Reply {
        let outs = self.change(actions)?;
        let outs: Vec<Value> = match part {
            Some(part) => outs.into_iter().map(|mut out| out[part].take()).collect(),
            None => outs,
        };
        Ok(json!({ field: outs }))
    }
}

pub fn route(vault: &Arc<Vault>, request: &Request, caller: &Caller) -> (u16, Value) {
    let asked = Asked {
        vault,
        request,
        caller,
    };
    let query = request.query().unwrap_or_default();
    let parts: Vec<&str> = request.path().trim_matches('/').split('/').collect();
    let method = request.method.as_str();
    let reply = match (method, parts.as_slice()) {
        ("GET", ["status"]) => Ok(status(vault)),
        ("GET", ["keys"]) => Ok(list_keys(vault)),
        ("PUT", ["keys"]) => put_keys(&asked),
        ("GET", ["keys", name]) => get_key(vault, name),
        ("PUT", ["keys", name]) => put_key(&asked, name),
        ("DELETE", ["keys", name]) => delete_key(&asked, name),
        ("POST", ["keys", name, "rotate"]) => rotate_key(&asked, name),
        ("POST", ["keys", name, "propose"]) => propose(vault, name, request),
        ("GET", ["tokens"]) => Ok(list_tokens(vault)),
        ("POST", ["tokens"]) => create_tokens(&asked),
        ("DELETE", ["tokens"]) => revoke_tokens(&asked),
        ("DELETE", ["tokens", id]) => revoke_token(&asked, id),
        ("GET", ["sandboxes"]) => Ok(list_sandboxes(vault)),
        ("POST", ["sandboxes"]) => add_sandboxes(&asked),
        ("DELETE", ["sandboxes", name]) => remove_sandbox(&asked, name),
        ("POST", ["batch"]) => batch(&asked),
        ("GET", ["approvals"]) => Ok(list_approvals(vault)),
        ("POST", ["approvals", id]) => decide(&asked, id),
        ("GET", ["audit"]) => audit(vault, query),
        ("GET", ["openapi.json"]) => Ok(admin_document()),
        ("GET", ["openapi", "gateway.json"]) => Ok(gateway_document()),
        _ => Err(fail(
            404,
            "not_found",
            format!("no route {method} {}", request.path()),
        )),
    };
    match reply {
        Ok(value) => (200, value),
        Err(failure) => (
            failure.status,
            api::value(Error {
                error: failure.code.to_string(),
                message: failure.message,
                seconds: failure.seconds,
            }),
        ),
    }
}

fn present(vault: &Vault, request: &Request, caller: &Caller, action: &str) -> Result<(), Failure> {
    vault
        .present(request, caller, action)
        .map_err(|absent| match absent {
            Absent::No => fail(
                401,
                "presence_required",
                format!("{action}: needs you present"),
            ),
            Absent::Busy => fail(409, "busy", "another vault prompt is open; answer it first"),
            Absent::Throttled(left) => {
                let seconds = left.as_millis().div_ceil(1000) as u64;
                Failure {
                    seconds: Some(seconds),
                    ..fail(
                        429,
                        "throttled",
                        format!("too many refused prompts: try again in {seconds}s"),
                    )
                }
            }
        })
}

pub fn listed(items: &[String]) -> String {
    items.join(", ")
}

fn parse<T: DeserializeOwned>(value: Value) -> Result<T, Failure> {
    serde_json::from_value(value).map_err(|e| bad(e.to_string()))
}

fn body<T: DeserializeOwned>(request: &Request) -> Result<T, Failure> {
    parse(request.json().map_err(|e| bad(e.to_string()))?)
}

enum Shape {
    One(Value),
    Many(Vec<Value>),
}

fn shape(request: &Request) -> Result<Shape, Failure> {
    match request.json().map_err(|e| bad(e.to_string()))? {
        Value::Array(items) => Ok(Shape::Many(items)),
        one => Ok(Shape::One(one)),
    }
}

fn list<T: DeserializeOwned>(request: &Request, what: &str) -> Result<Vec<T>, Failure> {
    match shape(request)? {
        Shape::Many(items) => items.into_iter().map(parse).collect(),
        Shape::One(_) => Err(bad(format!("the body is a list of {what}"))),
    }
}

#[derive(ToSchema)]
#[allow(dead_code)]
enum TokenBody {
    One(CreateToken),
    Many(Vec<CreateToken>),
}

#[derive(ToSchema)]
#[allow(dead_code)]
enum SandboxBody {
    One(AddSandbox),
    Many(Vec<AddSandbox>),
}

#[utoipa::path(
    get,
    path = "/status",
    description = "Whether the master key is loaded, the platform, the daemon's version and how it checks presence (touch_id, polkit, passphrase)",
    responses((status = 200, body = Status))
)]
fn status(vault: &Vault) -> Value {
    api::value(Status {
        master_key: vault.has_master(),
        platform: std::env::consts::OS.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        presence: vault.presence_kind().to_string(),
    })
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
        && !name.starts_with('.')
}

pub fn checked_name(name: &str) -> Result<(), Failure> {
    if valid_name(name) {
        Ok(())
    } else {
        Err(bad(format!(
            "{name} is not a name: letters, digits, - _ . and at most 64"
        )))
    }
}

#[utoipa::path(
    get,
    path = "/keys",
    description = "The keys' names and metadata, never their secrets",
    responses((status = 200, body = Keys))
)]
fn list_keys(vault: &Vault) -> Value {
    let keys = vault.store().keys.iter().map(KeyRecord::view).collect();
    api::value(Keys { keys })
}

#[utoipa::path(
    get,
    path = "/keys/{name}",
    params(("name" = String, Path)),
    responses((status = 200, body = KeyOut), (status = 404, body = Error))
)]
fn get_key(vault: &Vault, name: &str) -> Reply {
    vault
        .store()
        .key(name)
        .map(|key| api::value(KeyOut { key: key.view() }))
        .ok_or_else(|| missing("vault key", name))
}

#[utoipa::path(
    put,
    path = "/keys/{name}",
    description = "Stores or replaces a key; needs presence",
    params(("name" = String, Path), ("X-Vault-Passphrase" = Option<String>, Header)),
    request_body = PutKey,
    responses(
        (status = 200, body = KeyOut),
        (status = 400, body = Error),
        (status = 401, body = Error),
        (status = 409, body = Error),
        (status = 429, body = Error)
    )
)]
fn put_key(asked: &Asked, name: &str) -> Reply {
    checked_name(name)?;
    let key: PutKey = body(asked.request)?;
    asked.one(Action::PutKey(NamedKey {
        name: name.to_string(),
        key,
    }))
}

#[utoipa::path(
    put,
    path = "/keys",
    description = "Stores or replaces several keys with one prompt, all or none",
    params(("X-Vault-Passphrase" = Option<String>, Header)),
    request_body = Vec<NamedKey>,
    responses(
        (status = 200, body = Keys),
        (status = 400, body = Error),
        (status = 401, body = Error),
        (status = 409, body = Error),
        (status = 429, body = Error)
    )
)]
fn put_keys(asked: &Asked) -> Reply {
    let keys: Vec<NamedKey> = list(asked.request, "keys, each with its name")?;
    asked.many(
        keys.into_iter().map(Action::PutKey).collect(),
        "keys",
        Some("key"),
    )
}

#[utoipa::path(
    delete,
    path = "/keys/{name}",
    description = "Deletes a key; needs presence",
    params(("name" = String, Path), ("X-Vault-Passphrase" = Option<String>, Header)),
    responses(
        (status = 200, body = Deleted),
        (status = 401, body = Error),
        (status = 404, body = Error),
        (status = 409, body = Error),
        (status = 429, body = Error)
    )
)]
fn delete_key(asked: &Asked, name: &str) -> Reply {
    asked.one(Action::DeleteKey(Name {
        name: name.to_string(),
    }))
}

#[utoipa::path(
    post,
    path = "/keys/{name}/rotate",
    description = "Replaces a key's secret; needs presence",
    params(("name" = String, Path), ("X-Vault-Passphrase" = Option<String>, Header)),
    request_body = Rotate,
    responses(
        (status = 200, body = KeyOut),
        (status = 400, body = Error),
        (status = 401, body = Error),
        (status = 404, body = Error),
        (status = 409, body = Error),
        (status = 429, body = Error)
    )
)]
fn rotate_key(asked: &Asked, name: &str) -> Reply {
    let rotate: Rotate = body(asked.request)?;
    asked.one(Action::RotateKey(NamedRotate {
        name: name.to_string(),
        secret: rotate.secret,
    }))
}

#[derive(Deserialize, ToSchema)]
struct Propose {
    #[serde(default)]
    description: Option<String>,
}

#[utoipa::path(
    post,
    path = "/keys/{name}/propose",
    description = "Asks the vetting model for approved URLs from the key's description",
    params(("name" = String, Path)),
    request_body = Propose,
    responses(
        (status = 200, body = Proposed),
        (status = 400, body = Error),
        (status = 503, body = Error)
    )
)]
fn propose(vault: &Vault, name: &str, request: &Request) -> Reply {
    checked_name(name)?;
    let asked: Propose = body(request)?;
    let mut info = vault.store().key(name).map_or_else(
        || KeyInfo {
            name: name.into(),
            description: String::new(),
            approved_urls: Vec::new(),
        },
        KeyRecord::info,
    );
    if let Some(description) = asked.description {
        info.description = description;
    }
    if info.description.trim().is_empty() {
        return Err(bad(
            "the model needs a description to propose approved URLs",
        ));
    }
    let proposed = vault
        .vetter()
        .propose(&info)
        .map_err(|e| fail(503, "vetting_unavailable", format!("{e:#}")))?;
    let approved_urls = proposed
        .into_iter()
        .filter(|p| policy::parse(p, vault.config.http_prefixes).is_ok())
        .collect();
    Ok(api::value(Proposed {
        name: name.to_string(),
        approved_urls,
    }))
}

#[utoipa::path(
    get,
    path = "/tokens",
    description = "The tokens, never the token values",
    responses((status = 200, body = Tokens))
)]
fn list_tokens(vault: &Vault) -> Value {
    let tokens = vault.store().tokens.iter().map(TokenRecord::view).collect();
    api::value(Tokens { tokens })
}

#[utoipa::path(
    post,
    path = "/tokens",
    description = "Makes a token, or several with one prompt (a list body, answered as {\"tokens\": [...]}); needs presence. The token and its revoke value are shown once.",
    params(("X-Vault-Passphrase" = Option<String>, Header)),
    request_body = TokenBody,
    responses(
        (status = 200, body = NewToken),
        (status = 400, body = Error),
        (status = 401, body = Error),
        (status = 404, body = Error),
        (status = 409, body = Error),
        (status = 429, body = Error)
    )
)]
fn create_tokens(asked: &Asked) -> Reply {
    match shape(asked.request)? {
        Shape::One(one) => asked.one(Action::CreateToken(parse::<CreateToken>(one)?)),
        Shape::Many(items) => {
            let tokens = items
                .into_iter()
                .map(|item| parse(item).map(Action::CreateToken))
                .collect::<Result<Vec<_>, _>>()?;
            asked.many(tokens, "tokens", None)
        }
    }
}

#[utoipa::path(
    delete,
    path = "/tokens/{id}",
    description = "Revokes a token; needs presence unless X-Vault-Revoke carries the token's revoke value",
    params(
        ("id" = String, Path),
        ("X-Vault-Revoke" = Option<String>, Header),
        ("X-Vault-Passphrase" = Option<String>, Header)
    ),
    responses(
        (status = 200, body = Revoked),
        (status = 401, body = Error),
        (status = 404, body = Error),
        (status = 409, body = Error),
        (status = 429, body = Error)
    )
)]
fn revoke_token(asked: &Asked, id: &str) -> Reply {
    let own = asked
        .request
        .header("x-vault-revoke")
        .map(crypto::hash)
        .is_some_and(|hash| {
            asked
                .vault
                .store()
                .token(id)
                .is_some_and(|token| token.revoke.as_deref() == Some(hash.as_str()))
        });
    let action = Action::RevokeToken(TokenId { id: id.to_string() });
    if !own {
        return asked.one(action);
    }
    Ok(changes::run(asked.vault, vec![action], |_| Ok(()))?.swap_remove(0))
}

#[utoipa::path(
    delete,
    path = "/tokens",
    description = "Revokes several tokens with one prompt, all or none",
    params(("X-Vault-Passphrase" = Option<String>, Header)),
    request_body = Vec<String>,
    responses(
        (status = 200, body = RevokedMany),
        (status = 400, body = Error),
        (status = 401, body = Error),
        (status = 404, body = Error),
        (status = 409, body = Error),
        (status = 429, body = Error)
    )
)]
fn revoke_tokens(asked: &Asked) -> Reply {
    let ids: Vec<String> = list(asked.request, "token ids")?;
    asked.many(
        ids.into_iter()
            .map(|id| Action::RevokeToken(TokenId { id }))
            .collect(),
        "revoked",
        Some("revoked"),
    )
}

#[utoipa::path(
    get,
    path = "/sandboxes",
    responses((status = 200, body = Sandboxes))
)]
fn list_sandboxes(vault: &Vault) -> Value {
    api::value(Sandboxes {
        sandboxes: vault.store().sandboxes.clone(),
    })
}

#[utoipa::path(
    post,
    path = "/sandboxes",
    description = "Adds a sandbox endpoint, or several with one prompt (a list body, answered as {\"sandboxes\": [...]}); needs presence",
    params(("X-Vault-Passphrase" = Option<String>, Header)),
    request_body = SandboxBody,
    responses(
        (status = 200, body = SandboxOut),
        (status = 400, body = Error),
        (status = 401, body = Error),
        (status = 409, body = Error),
        (status = 429, body = Error),
        (status = 500, body = Error)
    )
)]
fn add_sandboxes(asked: &Asked) -> Reply {
    match shape(asked.request)? {
        Shape::One(one) => asked.one(Action::AddSandbox(parse::<AddSandbox>(one)?)),
        Shape::Many(items) => {
            let sandboxes = items
                .into_iter()
                .map(|item| parse(item).map(Action::AddSandbox))
                .collect::<Result<Vec<_>, _>>()?;
            asked.many(sandboxes, "sandboxes", Some("sandbox"))
        }
    }
}

#[utoipa::path(
    delete,
    path = "/sandboxes/{name}",
    description = "Removes a sandbox; its tokens stop working; needs presence",
    params(("name" = String, Path), ("X-Vault-Passphrase" = Option<String>, Header)),
    responses(
        (status = 200, body = Removed),
        (status = 401, body = Error),
        (status = 404, body = Error),
        (status = 409, body = Error),
        (status = 429, body = Error)
    )
)]
fn remove_sandbox(asked: &Asked, name: &str) -> Reply {
    asked.one(Action::RemoveSandbox(Name {
        name: name.to_string(),
    }))
}

#[utoipa::path(
    post,
    path = "/batch",
    description = "Changes of different kinds, checked together, one prompt, applied together or not at all; a token may use a key or sandbox made earlier in the same batch",
    params(("X-Vault-Passphrase" = Option<String>, Header)),
    request_body = Batch,
    responses(
        (status = 200, body = Results),
        (status = 400, body = Error),
        (status = 401, body = Error),
        (status = 404, body = Error),
        (status = 409, body = Error),
        (status = 429, body = Error),
        (status = 500, body = Error)
    )
)]
fn batch(asked: &Asked) -> Reply {
    let batch: Batch = body(asked.request)?;
    asked.many(batch.actions, "results", None)
}

#[utoipa::path(
    get,
    path = "/approvals",
    description = "Gateway calls waiting for a human",
    responses((status = 200, body = Approvals))
)]
fn list_approvals(vault: &Vault) -> Value {
    api::value(Approvals {
        approvals: vault.approvals.list(),
    })
}

#[derive(Deserialize, ToSchema)]
struct Decide {
    approve: bool,
}

#[utoipa::path(
    post,
    path = "/approvals/{id}",
    description = "Allows or refuses a waiting call; needs presence",
    params(("id" = String, Path), ("X-Vault-Passphrase" = Option<String>, Header)),
    request_body = Decide,
    responses(
        (status = 200, body = Decided),
        (status = 401, body = Error),
        (status = 404, body = Error),
        (status = 409, body = Error),
        (status = 429, body = Error)
    )
)]
fn decide(asked: &Asked, id: &str) -> Reply {
    let decision: Decide = body(asked.request)?;
    let approvals = &asked.vault.approvals;
    let Some(line) = approvals.line(id) else {
        return Err(missing("waiting call", id));
    };
    let verb = if decision.approve { "Allow" } else { "Refuse" };
    asked.present(&format!("{verb} this call: {line}"))?;
    if !approvals.decide(id, decision.approve) {
        return Err(missing("waiting call", id));
    }
    Ok(api::value(Decided {
        id: id.to_string(),
        approved: decision.approve,
    }))
}

#[utoipa::path(
    get,
    path = "/audit",
    params(("tail" = Option<usize>, Query, description = "How many of the last entries [default: 50]")),
    responses((status = 200, body = Audit), (status = 400, body = Error))
)]
fn audit(vault: &Vault, query: &str) -> Reply {
    let tail = query
        .split('&')
        .find_map(|pair| pair.strip_prefix("tail="))
        .map(|n| n.parse::<usize>().map_err(|_| bad("tail must be a number")))
        .transpose()?
        .unwrap_or(50);
    let entries = vault.store().audit_tail(tail).map_err(internal)?;
    Ok(api::value(Audit { entries }))
}

#[utoipa::path(
    get,
    path = "/openapi.json",
    description = "This document",
    responses((status = 200, body = Object))
)]
fn admin_document() -> Value {
    api::value(document())
}

#[utoipa::path(
    get,
    path = "/openapi/gateway.json",
    description = "The gateway's document: the one proxied path a sandbox endpoint answers",
    responses((status = 200, body = Object))
)]
fn gateway_document() -> Value {
    api::value(openapi::gateway())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn routed() -> BTreeSet<(String, String)> {
        let source = include_str!("admin.rs");
        let start = source.find("let reply = match (method").unwrap();
        let end = start + source[start..].find("_ => Err(").unwrap();
        source[start..end]
            .lines()
            .filter_map(|line| {
                let arm = line.trim().strip_prefix("(\"")?;
                let (method, rest) = arm.split_once("\", [")?;
                let (parts, _) = rest.split_once("])")?;
                let path: String = parts
                    .split(", ")
                    .map(|part| match part.strip_prefix('"') {
                        Some(literal) => format!("/{}", literal.trim_end_matches('"')),
                        None => format!("/{{{part}}}"),
                    })
                    .collect();
                Some((method.to_ascii_lowercase(), path))
            })
            .collect()
    }

    fn documented(doc: &Value) -> BTreeSet<(String, String)> {
        doc["paths"]
            .as_object()
            .unwrap()
            .iter()
            .flat_map(|(path, item)| {
                item.as_object()
                    .unwrap()
                    .keys()
                    .filter(|key| ["get", "put", "post", "delete"].contains(&key.as_str()))
                    .map(|method| (method.clone(), path.clone()))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    #[test]
    fn the_document_lists_every_admin_route_and_no_other() {
        let routes = routed();
        assert_eq!(routes.len(), 21, "{routes:?}");
        assert!(routes.contains(&("post".into(), "/keys/{name}/rotate".into())));
        let doc = admin_document();
        assert!(doc["openapi"].as_str().unwrap().starts_with("3."));
        assert_eq!(documented(&doc), routes);
        let text = serde_json::to_string(&doc).unwrap();
        let parsed: utoipa::openapi::OpenApi = serde_json::from_str(&text).unwrap();
        assert_eq!(
            parsed.paths.paths.len(),
            doc["paths"].as_object().unwrap().len()
        );
        for schema in ["Action", "Batch", "NewToken", "Error", "Endpoint"] {
            assert!(doc["components"]["schemas"][schema].is_object(), "{schema}");
        }
        let action = doc["components"]["schemas"]["Action"].to_string();
        for kind in [
            "put_key",
            "rotate_key",
            "delete_key",
            "create_token",
            "revoke_token",
            "add_sandbox",
            "remove_sandbox",
        ] {
            assert!(action.contains(kind), "{kind}");
        }
    }
}
