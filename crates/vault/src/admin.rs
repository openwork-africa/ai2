use crate::changes::{
    self, Action, AddSandbox, Batch, CreateToken, Name, NamedKey, NamedRotate, PutKey, Rotate,
    TokenId,
};
use crate::crypto;
use crate::daemon::{Absent, Vault};
use crate::http::{self, Request};
use crate::policy;
use crate::store::{KeyRecord, TokenRecord};
use crate::vet::KeyInfo;
use keys::Caller;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

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
        ("GET", ["approvals"]) => Ok(json!({ "approvals": vault.approvals.list() })),
        ("POST", ["approvals", id]) => decide(&asked, id),
        ("GET", ["audit"]) => audit(vault, query),
        _ => Err(fail(
            404,
            "not_found",
            format!("no route {method} {}", request.path()),
        )),
    };
    match reply {
        Ok(value) => (200, value),
        Err(failure) => {
            let mut body = http::error(failure.code, &failure.message);
            if let Some(seconds) = failure.seconds {
                body["seconds"] = json!(seconds);
            }
            (failure.status, body)
        }
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

fn status(vault: &Vault) -> Value {
    json!({
        "master_key": vault.has_master(),
        "platform": std::env::consts::OS,
        "version": env!("CARGO_PKG_VERSION"),
        "presence": vault.presence_kind(),
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

fn list_keys(vault: &Vault) -> Value {
    let keys: Vec<Value> = vault.store().keys.iter().map(KeyRecord::public).collect();
    json!({ "keys": keys })
}

fn get_key(vault: &Vault, name: &str) -> Reply {
    vault
        .store()
        .key(name)
        .map(|key| json!({ "key": key.public() }))
        .ok_or_else(|| missing("vault key", name))
}

fn put_key(asked: &Asked, name: &str) -> Reply {
    checked_name(name)?;
    let key: PutKey = body(asked.request)?;
    asked.one(Action::PutKey(NamedKey {
        name: name.to_string(),
        key,
    }))
}

fn put_keys(asked: &Asked) -> Reply {
    let keys: Vec<NamedKey> = list(asked.request, "keys, each with its name")?;
    asked.many(
        keys.into_iter().map(Action::PutKey).collect(),
        "keys",
        Some("key"),
    )
}

fn delete_key(asked: &Asked, name: &str) -> Reply {
    asked.one(Action::DeleteKey(Name {
        name: name.to_string(),
    }))
}

fn rotate_key(asked: &Asked, name: &str) -> Reply {
    let rotate: Rotate = body(asked.request)?;
    asked.one(Action::RotateKey(NamedRotate {
        name: name.to_string(),
        secret: rotate.secret,
    }))
}

#[derive(Deserialize)]
struct Propose {
    #[serde(default)]
    description: Option<String>,
}

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
    let prefixes: Vec<String> = proposed
        .into_iter()
        .filter(|p| policy::parse(p, vault.config.allow_http).is_ok())
        .collect();
    Ok(json!({ "name": name, "approved_urls": prefixes }))
}

fn list_tokens(vault: &Vault) -> Value {
    let tokens: Vec<Value> = vault
        .store()
        .tokens
        .iter()
        .map(TokenRecord::public)
        .collect();
    json!({ "tokens": tokens })
}

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

fn list_sandboxes(vault: &Vault) -> Value {
    json!({ "sandboxes": vault.store().sandboxes })
}

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

fn remove_sandbox(asked: &Asked, name: &str) -> Reply {
    asked.one(Action::RemoveSandbox(Name {
        name: name.to_string(),
    }))
}

fn batch(asked: &Asked) -> Reply {
    let batch: Batch = body(asked.request)?;
    asked.many(batch.actions, "results", None)
}

#[derive(Deserialize)]
struct Decide {
    approve: bool,
}

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
    Ok(json!({ "id": id, "approved": decision.approve }))
}

fn audit(vault: &Vault, query: &str) -> Reply {
    let tail = query
        .split('&')
        .find_map(|pair| pair.strip_prefix("tail="))
        .map(|n| n.parse::<usize>().map_err(|_| bad("tail must be a number")))
        .transpose()?
        .unwrap_or(50);
    let lines = vault.store().audit_tail(tail).map_err(internal)?;
    Ok(json!({ "entries": lines }))
}
