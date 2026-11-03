use crate::crypto;
use crate::daemon::{Absent, Vault};
use crate::http::{self, Request};
use crate::inject::{self, Inject};
use crate::policy;
use crate::store::{Endpoint, KeyRecord, Mode, Sandbox, TokenRecord};
use crate::time;
use crate::vet::KeyInfo;
use keys::Caller;
use serde::Deserialize;
use serde_json::{json, Value};
use std::net::IpAddr;
use std::sync::Arc;

const AGENT_TTL: &str = "8h";
const APP_TTL: &str = "30d";

pub struct Failure {
    status: u16,
    code: &'static str,
    message: String,
    seconds: Option<u64>,
}

type Reply = Result<Value, Failure>;

fn fail(status: u16, code: &'static str, message: impl Into<String>) -> Failure {
    Failure {
        status,
        code,
        message: message.into(),
        seconds: None,
    }
}

fn missing(what: &str, name: &str) -> Failure {
    fail(404, "not_found", format!("there is no {what} {name}"))
}

fn bad(message: impl Into<String>) -> Failure {
    fail(400, "bad_request", message)
}

fn internal(error: anyhow::Error) -> Failure {
    fail(500, "internal", format!("{error:#}"))
}

pub fn route(vault: &Arc<Vault>, request: &Request, caller: &Caller) -> (u16, Value) {
    let query = request.query().unwrap_or_default();
    let parts: Vec<&str> = request.path().trim_matches('/').split('/').collect();
    let method = request.method.as_str();
    let reply = match (method, parts.as_slice()) {
        ("GET", ["status"]) => Ok(status(vault)),
        ("GET", ["keys"]) => Ok(list_keys(vault)),
        ("GET", ["keys", name]) => get_key(vault, name),
        ("PUT", ["keys", name]) => put_key(vault, name, request, caller),
        ("DELETE", ["keys", name]) => delete_key(vault, name, request, caller),
        ("POST", ["keys", name, "rotate"]) => rotate_key(vault, name, request, caller),
        ("POST", ["keys", name, "propose"]) => propose(vault, name, request),
        ("GET", ["tokens"]) => Ok(list_tokens(vault)),
        ("POST", ["tokens"]) => create_token(vault, request, caller),
        ("DELETE", ["tokens", id]) => revoke_token(vault, id, request, caller),
        ("GET", ["sandboxes"]) => Ok(list_sandboxes(vault)),
        ("POST", ["sandboxes"]) => add_sandbox(vault, request, caller),
        ("DELETE", ["sandboxes", name]) => remove_sandbox(vault, name, request, caller),
        ("GET", ["approvals"]) => Ok(json!({ "approvals": vault.approvals.list() })),
        ("POST", ["approvals", id]) => decide(vault, id, request, caller),
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

fn listed(items: &[String]) -> String {
    items.join(", ")
}

fn body<T: for<'de> Deserialize<'de>>(request: &Request) -> Result<T, Failure> {
    let value = request.json().map_err(|e| bad(e.to_string()))?;
    serde_json::from_value(value).map_err(|e| bad(e.to_string()))
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

fn checked_name(name: &str) -> Result<(), Failure> {
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
    known_key(vault, name).map(|key| json!({ "key": key.public() }))
}

#[derive(Deserialize)]
struct PutKey {
    secret: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    approved_urls: Vec<String>,
    inject: Option<String>,
}

fn checked_secret(secret: &str) -> Result<(), Failure> {
    if secret.is_empty() || secret.contains(['\r', '\n', '\0']) {
        return Err(bad("the secret is empty or has a line break"));
    }
    Ok(())
}

fn put_key(vault: &Vault, name: &str, request: &Request, caller: &Caller) -> Reply {
    checked_name(name)?;
    let put: PutKey = body(request)?;
    let put_secret = zeroize::Zeroizing::new(put.secret);
    checked_secret(&put_secret)?;
    let inject = put.inject.unwrap_or_else(|| inject::DEFAULT.into());
    Inject::parse(&inject).map_err(|e| bad(e.to_string()))?;
    if put.approved_urls.is_empty() {
        return Err(bad("a key needs at least one approved URL"));
    }
    for url in &put.approved_urls {
        policy::parse(url, vault.config.allow_http).map_err(|e| bad(e.to_string()))?;
    }
    let verb = if vault.store().key(name).is_some() {
        "Replace"
    } else {
        "Store"
    };
    present(
        vault,
        request,
        caller,
        &format!(
            "{verb} the vault key {name}, to be sent only to {} as {inject}",
            listed(&put.approved_urls)
        ),
    )?;
    let sealed = crypto::seal(
        &*vault.master().map_err(internal)?,
        name,
        put_secret.as_bytes(),
    )
    .map_err(internal)?;
    let mut store = vault.store();
    let created = store
        .key(name)
        .map(|key| key.created.clone())
        .unwrap_or_else(time::stamp);
    let rotated = store.key(name).map(|_| time::stamp());
    let record = KeyRecord {
        name: name.into(),
        description: put.description,
        approved_urls: put.approved_urls,
        inject,
        created,
        rotated,
        nonce: sealed.nonce,
        secret: sealed.secret,
    };
    let public = record.public();
    store.forget_key(name);
    store.keys.push(record);
    store.save("keys", &store.keys).map_err(internal)?;
    store.save("vetted", &store.vetted).map_err(internal)?;
    Ok(json!({ "key": public }))
}

fn known_key(vault: &Vault, name: &str) -> Result<KeyRecord, Failure> {
    vault
        .store()
        .key(name)
        .cloned()
        .ok_or_else(|| missing("vault key", name))
}

fn delete_key(vault: &Vault, name: &str, request: &Request, caller: &Caller) -> Reply {
    let key = known_key(vault, name)?;
    present(
        vault,
        request,
        caller,
        &format!(
            "Delete the vault key {name}, which is sent to {}",
            listed(&key.approved_urls)
        ),
    )?;
    let mut store = vault.store();
    store.forget_key(name);
    store.save("keys", &store.keys).map_err(internal)?;
    store.save("vetted", &store.vetted).map_err(internal)?;
    Ok(json!({ "deleted": name }))
}

#[derive(Deserialize)]
struct Rotate {
    secret: String,
}

fn rotate_key(vault: &Vault, name: &str, request: &Request, caller: &Caller) -> Reply {
    let rotate: Rotate = body(request)?;
    let secret = zeroize::Zeroizing::new(rotate.secret);
    checked_secret(&secret)?;
    let known = known_key(vault, name)?;
    present(
        vault,
        request,
        caller,
        &format!(
            "Replace the secret of the vault key {name}, which is sent to {}",
            listed(&known.approved_urls)
        ),
    )?;
    let sealed = crypto::seal(&*vault.master().map_err(internal)?, name, secret.as_bytes())
        .map_err(internal)?;
    let mut store = vault.store();
    let Some(key) = store.keys.iter_mut().find(|key| key.name == name) else {
        return Err(missing("vault key", name));
    };
    key.nonce = sealed.nonce;
    key.secret = sealed.secret;
    key.rotated = Some(time::stamp());
    let public = key.public();
    store.save("keys", &store.keys).map_err(internal)?;
    Ok(json!({ "key": public }))
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

#[derive(Deserialize)]
#[serde(untagged)]
enum Methods {
    One(String),
    Many(Vec<String>),
}

#[derive(Deserialize)]
struct CreateToken {
    keys: Vec<String>,
    sandbox: String,
    mode: Mode,
    methods: Option<Methods>,
    #[serde(default)]
    url_prefixes: Vec<String>,
    intent: Option<String>,
    ttl: Option<String>,
    uses: Option<u64>,
}

fn create_token(vault: &Vault, request: &Request, caller: &Caller) -> Reply {
    let asked: CreateToken = body(request)?;
    if asked.keys.is_empty() {
        return Err(bad("a token needs at least one key"));
    }
    let spec = match &asked.methods {
        Some(Methods::One(spec)) => spec.clone(),
        Some(Methods::Many(list)) => list.join(","),
        None => "read".into(),
    };
    let methods = policy::methods(&spec).map_err(|e| bad(e.to_string()))?;
    let intent = asked.intent.clone().filter(|i| !i.trim().is_empty());
    if asked.mode == Mode::Agent && intent.is_none() {
        return Err(bad("an agent token needs --intent"));
    }
    if asked.uses == Some(0) {
        return Err(bad("--uses must be at least 1"));
    }
    for prefix in &asked.url_prefixes {
        policy::parse(prefix, vault.config.allow_http).map_err(|e| bad(e.to_string()))?;
    }
    let default_ttl = if asked.mode == Mode::Agent {
        AGENT_TTL
    } else {
        APP_TTL
    };
    let ttl =
        time::ttl(asked.ttl.as_deref().unwrap_or(default_ttl)).map_err(|e| bad(e.to_string()))?;
    {
        let store = vault.store();
        if store.sandbox(&asked.sandbox).is_none() {
            return Err(missing("sandbox", &asked.sandbox));
        }
        if let Some(absent) = asked.keys.iter().find(|k| store.key(k).is_none()) {
            return Err(missing("vault key", absent));
        }
    }
    let action = token_action(&asked, &methods, intent.as_deref(), ttl);
    present(vault, request, caller, &action)?;
    let token = crypto::token().map_err(internal)?;
    let revoke = crypto::token().map_err(internal)?;
    let mut store = vault.store();
    let now = time::now();
    let record = TokenRecord {
        id: crypto::id("tok_").map_err(internal)?,
        hash: crypto::hash(&token),
        keys: asked.keys,
        sandbox: asked.sandbox,
        mode: asked.mode,
        methods,
        url_prefixes: asked.url_prefixes,
        intent,
        created: now,
        expires: now.saturating_add(ttl),
        uses: asked.uses,
        revoke: Some(crypto::hash(&revoke)),
    };
    let mut out = record.public();
    out["token"] = json!(token.as_str());
    out["revoke"] = json!(revoke.as_str());
    if record.sandbox == "host" && cfg!(target_os = "macos") {
        out["warnings"] = json!([
            "Not safe: a host token works for any process on this Mac, outside a container."
        ]);
    }
    store.tokens.push(record);
    store.save("tokens", &store.tokens).map_err(internal)?;
    Ok(out)
}

fn token_action(asked: &CreateToken, methods: &[String], intent: Option<&str>, ttl: u64) -> String {
    let mode = match asked.mode {
        Mode::App => "app",
        Mode::Agent => "agent",
    };
    let urls = if asked.url_prefixes.is_empty() {
        "the keys' approved URLs".to_string()
    } else {
        listed(&asked.url_prefixes)
    };
    let mut action = format!(
        "Make an {mode} token for the keys {} in the sandbox {}: methods {}, URLs {urls}, for {}",
        listed(&asked.keys),
        asked.sandbox,
        listed(methods),
        time::span(ttl)
    );
    if let Some(uses) = asked.uses {
        action.push_str(&format!(", {uses} uses"));
    }
    if let Some(intent) = intent {
        action.push_str(&format!(", intent: \"{intent}\""));
    }
    action
}

fn revoke_token(vault: &Vault, id: &str, request: &Request, caller: &Caller) -> Reply {
    let token = vault
        .store()
        .token(id)
        .cloned()
        .ok_or_else(|| missing("token", id))?;
    let own = request
        .header("x-vault-revoke")
        .map(crypto::hash)
        .is_some_and(|hash| token.revoke.as_deref() == Some(hash.as_str()));
    if !own {
        present(
            vault,
            request,
            caller,
            &format!(
                "Revoke the vault token {id} for the keys {} in the sandbox {}",
                listed(&token.keys),
                token.sandbox
            ),
        )?;
    }
    let mut store = vault.store();
    let before = store.tokens.len();
    store.tokens.retain(|t| t.id != id);
    if store.tokens.len() == before {
        return Err(missing("token", id));
    }
    store.save("tokens", &store.tokens).map_err(internal)?;
    Ok(json!({ "revoked": id }))
}

fn list_sandboxes(vault: &Vault) -> Value {
    json!({ "sandboxes": vault.store().sandboxes })
}

#[derive(Deserialize)]
struct AddSandbox {
    name: String,
    socket: Option<std::path::PathBuf>,
    tcp: Option<String>,
    #[serde(default)]
    peers: Vec<IpAddr>,
}

fn add_sandbox(vault: &Arc<Vault>, request: &Request, caller: &Caller) -> Reply {
    let asked: AddSandbox = body(request)?;
    checked_name(&asked.name)?;
    if vault.store().sandbox(&asked.name).is_some() {
        return Err(fail(
            409,
            "exists",
            format!("there is a sandbox {} already", asked.name),
        ));
    }
    let endpoint = match asked.tcp {
        Some(listen) => {
            if asked.peers.is_empty() {
                return Err(bad("a tcp sandbox needs at least one --peer address"));
            }
            Endpoint::Tcp {
                listen,
                peers: asked.peers,
            }
        }
        None => Endpoint::Unix {
            path: asked
                .socket
                .unwrap_or_else(|| vault.config.sockets.join(&asked.name).join("gateway.sock")),
        },
    };
    let at = match &endpoint {
        Endpoint::Unix { path } => format!("the socket {}", path.display()),
        Endpoint::Tcp { listen, peers } => format!(
            "TCP {listen}, taking calls from {}",
            peers
                .iter()
                .map(IpAddr::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    present(
        vault,
        request,
        caller,
        &format!("Add the vault sandbox {} at {at}", asked.name),
    )?;
    let endpoint = vault
        .open_endpoint(&asked.name, &endpoint)
        .map_err(internal)?;
    let sandbox = Sandbox {
        name: asked.name,
        endpoint,
    };
    let mut store = vault.store();
    store.sandboxes.push(sandbox.clone());
    store
        .save("sandboxes", &store.sandboxes)
        .map_err(internal)?;
    Ok(json!({ "sandbox": sandbox }))
}

fn remove_sandbox(vault: &Vault, name: &str, request: &Request, caller: &Caller) -> Reply {
    if vault.store().sandbox(name).is_some() {
        present(
            vault,
            request,
            caller,
            &format!("Remove the vault sandbox {name}; its tokens stop working"),
        )?;
    }
    {
        let mut store = vault.store();
        if store.sandbox(name).is_none() {
            return Err(missing("sandbox", name));
        }
        store.sandboxes.retain(|s| s.name != name);
        store
            .save("sandboxes", &store.sandboxes)
            .map_err(internal)?;
    }
    vault.close_endpoint(name);
    Ok(json!({ "removed": name }))
}

#[derive(Deserialize)]
struct Decide {
    approve: bool,
}

fn decide(vault: &Vault, id: &str, request: &Request, caller: &Caller) -> Reply {
    let asked: Decide = body(request)?;
    let Some(line) = vault.approvals.line(id) else {
        return Err(missing("waiting call", id));
    };
    let verb = if asked.approve { "Allow" } else { "Refuse" };
    present(vault, request, caller, &format!("{verb} this call: {line}"))?;
    if !vault.approvals.decide(id, asked.approve) {
        return Err(missing("waiting call", id));
    }
    Ok(json!({ "id": id, "approved": asked.approve }))
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
