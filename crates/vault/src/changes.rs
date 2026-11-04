use crate::admin::{bad, checked_name, fail, internal, listed, missing, Failure};
use crate::api::{self, Changed, Deleted, KeyOut, NewToken, Removed, Revoked, SandboxOut};
use crate::crypto::{self, Sealed};
use crate::daemon::Vault;
use crate::inject::{self, Inject};
use crate::policy;
use crate::store::{Endpoint, KeyRecord, Mode, Sandbox, Store, TokenRecord};
use crate::time;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;
use utoipa::ToSchema;
use zeroize::Zeroizing;

pub const MAX_ITEMS: usize = 100;
const AGENT_TTL: &str = "8h";
const APP_TTL: &str = "30d";

#[derive(Deserialize, ToSchema)]
pub struct PutKey {
    pub secret: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub approved_urls: Vec<String>,
    pub inject: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct NamedKey {
    pub name: String,
    #[serde(flatten)]
    pub key: PutKey,
}

#[derive(Deserialize, ToSchema)]
pub struct Rotate {
    pub secret: String,
}

#[derive(Deserialize, ToSchema)]
pub struct NamedRotate {
    pub name: String,
    pub secret: String,
}

#[derive(Deserialize, ToSchema)]
pub struct Name {
    pub name: String,
}

#[derive(Deserialize, ToSchema)]
pub struct TokenId {
    pub id: String,
}

#[derive(Deserialize, ToSchema)]
#[serde(untagged)]
pub enum Methods {
    One(String),
    Many(Vec<String>),
}

#[derive(Deserialize, ToSchema)]
pub struct CreateToken {
    pub keys: Vec<String>,
    pub sandbox: String,
    pub mode: Mode,
    pub methods: Option<Methods>,
    #[serde(default)]
    pub url_prefixes: Vec<String>,
    pub intent: Option<String>,
    pub ttl: Option<String>,
    pub uses: Option<u64>,
}

#[derive(Deserialize, ToSchema)]
pub struct AddSandbox {
    pub name: String,
    #[schema(value_type = Option<String>)]
    pub socket: Option<PathBuf>,
    pub tcp: Option<String>,
    #[serde(default)]
    #[schema(value_type = Vec<String>)]
    #[allow(dead_code)]
    pub peers: Vec<IpAddr>,
}

#[derive(Deserialize, ToSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    PutKey(NamedKey),
    RotateKey(NamedRotate),
    DeleteKey(Name),
    CreateToken(CreateToken),
    RevokeToken(TokenId),
    AddSandbox(AddSandbox),
    RemoveSandbox(Name),
}

#[derive(Deserialize, ToSchema)]
pub struct Batch {
    pub actions: Vec<Action>,
}

struct Token {
    keys: Vec<String>,
    sandbox: String,
    mode: Mode,
    methods: Vec<String>,
    url_prefixes: Vec<String>,
    intent: Option<String>,
    ttl: u64,
    uses: Option<u64>,
}

enum Step {
    PutKey {
        name: String,
        description: String,
        approved_urls: Vec<String>,
        inject: String,
        secret: Zeroizing<String>,
    },
    RotateKey {
        name: String,
        secret: Zeroizing<String>,
    },
    DeleteKey {
        name: String,
    },
    CreateToken(Token),
    RevokeToken {
        id: String,
    },
    AddSandbox {
        name: String,
        endpoint: Endpoint,
    },
    RemoveSandbox {
        name: String,
    },
}

fn checked_secret(secret: &str) -> Result<(), Failure> {
    if secret.is_empty() || secret.contains(['\r', '\n', '\0']) {
        return Err(bad("the secret is empty or has a line break"));
    }
    Ok(())
}

fn check(vault: &Vault, action: Action) -> Result<Step, Failure> {
    let http = vault.config.http_prefixes;
    match action {
        Action::PutKey(NamedKey { name, key }) => {
            checked_name(&name)?;
            let secret = Zeroizing::new(key.secret);
            checked_secret(&secret)?;
            let inject = key.inject.unwrap_or_else(|| inject::DEFAULT.into());
            Inject::parse(&inject).map_err(|e| bad(e.to_string()))?;
            if key.approved_urls.is_empty() {
                return Err(bad("a key needs at least one approved URL"));
            }
            for url in &key.approved_urls {
                policy::parse(url, http).map_err(|e| bad(e.to_string()))?;
            }
            Ok(Step::PutKey {
                name,
                description: key.description,
                approved_urls: key.approved_urls,
                inject,
                secret,
            })
        }
        Action::RotateKey(NamedRotate { name, secret }) => {
            let secret = Zeroizing::new(secret);
            checked_secret(&secret)?;
            Ok(Step::RotateKey { name, secret })
        }
        Action::DeleteKey(Name { name }) => Ok(Step::DeleteKey { name }),
        Action::CreateToken(asked) => checked_token(asked, http).map(Step::CreateToken),
        Action::RevokeToken(TokenId { id }) => Ok(Step::RevokeToken { id }),
        Action::AddSandbox(asked) => checked_sandbox(vault, asked),
        Action::RemoveSandbox(Name { name }) => Ok(Step::RemoveSandbox { name }),
    }
}

fn checked_token(asked: CreateToken, http: bool) -> Result<Token, Failure> {
    if asked.keys.is_empty() {
        return Err(bad("a token needs at least one key"));
    }
    let spec = match &asked.methods {
        Some(Methods::One(spec)) => spec.clone(),
        Some(Methods::Many(list)) => list.join(","),
        None => "read".into(),
    };
    let methods = policy::methods(&spec).map_err(|e| bad(e.to_string()))?;
    let intent = asked.intent.filter(|i| !i.trim().is_empty());
    if asked.mode == Mode::Agent && intent.is_none() {
        return Err(bad("an agent token needs --intent"));
    }
    if asked.uses == Some(0) {
        return Err(bad("--uses must be at least 1"));
    }
    for prefix in &asked.url_prefixes {
        policy::parse(prefix, http).map_err(|e| bad(e.to_string()))?;
    }
    let default_ttl = if asked.mode == Mode::Agent {
        AGENT_TTL
    } else {
        APP_TTL
    };
    let ttl =
        time::ttl(asked.ttl.as_deref().unwrap_or(default_ttl)).map_err(|e| bad(e.to_string()))?;
    Ok(Token {
        keys: asked.keys,
        sandbox: asked.sandbox,
        mode: asked.mode,
        methods,
        url_prefixes: asked.url_prefixes,
        intent,
        ttl,
        uses: asked.uses,
    })
}

fn checked_sandbox(vault: &Vault, asked: AddSandbox) -> Result<Step, Failure> {
    checked_name(&asked.name)?;
    let endpoint = match asked.tcp {
        Some(_) => {
            return Err(bad(
                "tcp sandboxes are not supported: a source address is not an identity; use a unix socket",
            ));
        }
        None => Endpoint::Unix {
            path: asked
                .socket
                .unwrap_or_else(|| vault.config.sockets.join(&asked.name).join("gateway.sock")),
        },
    };
    Ok(Step::AddSandbox {
        name: asked.name,
        endpoint,
    })
}

#[derive(Default)]
struct Seen {
    keys: HashMap<String, Vec<String>>,
    sandboxes: HashSet<String>,
    tokens: HashMap<String, (Vec<String>, String)>,
    touched: HashSet<(&'static str, String)>,
}

impl Seen {
    fn of(store: &Store) -> Seen {
        Seen {
            keys: store
                .keys
                .iter()
                .map(|key| (key.name.clone(), key.approved_urls.clone()))
                .collect(),
            sandboxes: store.sandboxes.iter().map(|s| s.name.clone()).collect(),
            tokens: store
                .tokens
                .iter()
                .map(|t| (t.id.clone(), (t.keys.clone(), t.sandbox.clone())))
                .collect(),
            touched: HashSet::new(),
        }
    }

    fn touch(&mut self, kind: &'static str, name: &str) -> Result<(), Failure> {
        if self.touched.insert((kind, name.to_string())) {
            return Ok(());
        }
        Err(bad(format!("the {kind} {name} is changed twice in one go")))
    }

    fn key_urls(&self, name: &str) -> Result<String, Failure> {
        self.keys
            .get(name)
            .map(|urls| listed(urls))
            .ok_or_else(|| missing("vault key", name))
    }
}

fn line(seen: &mut Seen, step: &Step) -> Result<String, Failure> {
    match step {
        Step::PutKey {
            name,
            approved_urls,
            inject,
            ..
        } => {
            seen.touch("vault key", name)?;
            let verb = if seen.keys.contains_key(name) {
                "Replace"
            } else {
                "Store"
            };
            seen.keys.insert(name.clone(), approved_urls.clone());
            Ok(format!(
                "{verb} the vault key {name}, to be sent only to {} as {inject}",
                listed(approved_urls)
            ))
        }
        Step::RotateKey { name, .. } => {
            let urls = seen.key_urls(name)?;
            seen.touch("vault key", name)?;
            Ok(format!(
                "Replace the secret of the vault key {name}, which is sent to {urls}"
            ))
        }
        Step::DeleteKey { name } => {
            let urls = seen.key_urls(name)?;
            seen.touch("vault key", name)?;
            seen.keys.remove(name);
            Ok(format!(
                "Delete the vault key {name}, which is sent to {urls}"
            ))
        }
        Step::CreateToken(token) => {
            if !seen.sandboxes.contains(&token.sandbox) {
                return Err(missing("sandbox", &token.sandbox));
            }
            if let Some(absent) = token.keys.iter().find(|k| !seen.keys.contains_key(*k)) {
                return Err(missing("vault key", absent));
            }
            Ok(token_line(token))
        }
        Step::RevokeToken { id } => {
            let Some((keys, sandbox)) = seen.tokens.remove(id) else {
                return Err(missing("token", id));
            };
            seen.touch("token", id)?;
            Ok(format!(
                "Revoke the vault token {id} for the keys {} in the sandbox {sandbox}",
                listed(&keys)
            ))
        }
        Step::AddSandbox { name, endpoint } => {
            if seen.sandboxes.contains(name) {
                return Err(fail(
                    409,
                    "exists",
                    format!("there is a sandbox {name} already"),
                ));
            }
            seen.touch("sandbox", name)?;
            seen.sandboxes.insert(name.clone());
            Ok(format!("Add the vault sandbox {name} at {}", at(endpoint)))
        }
        Step::RemoveSandbox { name } => {
            if !seen.sandboxes.remove(name) {
                return Err(missing("sandbox", name));
            }
            seen.touch("sandbox", name)?;
            Ok(format!(
                "Remove the vault sandbox {name}; its tokens stop working"
            ))
        }
    }
}

fn at(endpoint: &Endpoint) -> String {
    match endpoint {
        Endpoint::Unix { path } => format!("the socket {}", path.display()),
        Endpoint::Tcp { listen, peers } => format!(
            "TCP {listen}, taking calls from {}",
            peers
                .iter()
                .map(IpAddr::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn token_line(token: &Token) -> String {
    let mode = match token.mode {
        Mode::App => "app",
        Mode::Agent => "agent",
    };
    let urls = if token.url_prefixes.is_empty() {
        "the keys' approved URLs".to_string()
    } else {
        listed(&token.url_prefixes)
    };
    let mut action = format!(
        "Make an {mode} token for the keys {} in the sandbox {}: methods {}, URLs {urls}, for {}",
        listed(&token.keys),
        token.sandbox,
        listed(&token.methods),
        time::span(token.ttl)
    );
    if let Some(uses) = token.uses {
        action.push_str(&format!(", {uses} uses"));
    }
    if let Some(intent) = &token.intent {
        action.push_str(&format!(", intent: \"{intent}\""));
    }
    action
}

fn lines(store: &Store, steps: &[Step]) -> Result<Vec<String>, Failure> {
    let mut seen = Seen::of(store);
    steps.iter().map(|step| line(&mut seen, step)).collect()
}

pub fn prompt(lines: &[String]) -> String {
    match lines {
        [one] => one.clone(),
        _ => format!(
            "Make these {} vault changes, all or none: {}",
            lines.len(),
            lines
                .iter()
                .enumerate()
                .map(|(i, line)| format!("({}) {line}", i + 1))
                .collect::<Vec<_>>()
                .join(" ")
        ),
    }
}

pub fn run(
    vault: &Arc<Vault>,
    actions: Vec<Action>,
    confirm: impl FnOnce(&str) -> Result<(), Failure>,
) -> Result<Vec<Value>, Failure> {
    if actions.is_empty() {
        return Err(bad("there is nothing to change"));
    }
    if actions.len() > MAX_ITEMS {
        return Err(bad(format!("at most {MAX_ITEMS} changes in one go")));
    }
    let steps = actions
        .into_iter()
        .map(|action| check(vault, action))
        .collect::<Result<Vec<_>, _>>()?;
    let asked = lines(&vault.store(), &steps)?;
    confirm(&prompt(&asked))?;
    apply(vault, steps)
}

enum Ready {
    Key(KeyRecord),
    Rotated(String, Sealed),
    Deleted(String),
    Token(Box<TokenRecord>, Zeroizing<String>, Zeroizing<String>),
    Revoked(String),
    Sandbox(String, Endpoint, bool),
    Removed(String),
}

fn ready(vault: &Vault, steps: Vec<Step>) -> Result<Vec<Ready>, Failure> {
    let sealing = steps
        .iter()
        .any(|step| matches!(step, Step::PutKey { .. } | Step::RotateKey { .. }));
    let master = if sealing {
        Some(vault.master().map_err(internal)?)
    } else {
        None
    };
    let seal = |name: &str, secret: &str| {
        let master = master.as_ref().ok_or_else(|| bad("no master key"))?;
        crypto::seal(master, name, secret.as_bytes()).map_err(internal)
    };
    let now = time::now();
    steps
        .into_iter()
        .map(|step| {
            Ok(match step {
                Step::PutKey {
                    name,
                    description,
                    approved_urls,
                    inject,
                    secret,
                } => {
                    let sealed = seal(&name, &secret)?;
                    Ready::Key(KeyRecord {
                        name,
                        description,
                        approved_urls,
                        inject,
                        created: time::stamp(),
                        rotated: None,
                        nonce: sealed.nonce,
                        secret: sealed.secret,
                    })
                }
                Step::RotateKey { name, secret } => {
                    let sealed = seal(&name, &secret)?;
                    Ready::Rotated(name, sealed)
                }
                Step::DeleteKey { name } => Ready::Deleted(name),
                Step::CreateToken(token) => {
                    let secret = crypto::token().map_err(internal)?;
                    let revoke = crypto::token().map_err(internal)?;
                    let record = TokenRecord {
                        id: crypto::id("tok_").map_err(internal)?,
                        hash: crypto::hash(&secret),
                        keys: token.keys,
                        sandbox: token.sandbox,
                        mode: token.mode,
                        methods: token.methods,
                        url_prefixes: token.url_prefixes,
                        intent: token.intent,
                        created: now,
                        expires: now.saturating_add(token.ttl),
                        uses: token.uses,
                        revoke: Some(crypto::hash(&revoke)),
                    };
                    Ready::Token(Box::new(record), secret, revoke)
                }
                Step::RevokeToken { id } => Ready::Revoked(id),
                Step::AddSandbox { name, endpoint } => Ready::Sandbox(name, endpoint, false),
                Step::RemoveSandbox { name } => Ready::Removed(name),
            })
        })
        .collect()
}

#[derive(Default)]
struct Dirty {
    keys: bool,
    vetted: bool,
    tokens: bool,
    sandboxes: bool,
}

impl Dirty {
    fn save(&self, store: &Store) -> anyhow::Result<()> {
        if self.keys {
            store.save("keys", &store.keys)?;
        }
        if self.vetted {
            store.save("vetted", &store.vetted)?;
        }
        if self.tokens {
            store.save("tokens", &store.tokens)?;
        }
        if self.sandboxes {
            store.save("sandboxes", &store.sandboxes)?;
        }
        Ok(())
    }
}

fn token_out(record: &TokenRecord, token: &str, revoke: &str) -> NewToken {
    let mut warnings = Vec::new();
    if record.sandbox == "host" && cfg!(target_os = "macos") {
        warnings.push(
            "Not safe: a host token works for any process on this Mac, outside a container."
                .to_string(),
        );
    }
    NewToken {
        view: record.view(),
        token: token.to_string(),
        revoke: revoke.to_string(),
        warnings,
    }
}

fn change(
    vault: &Arc<Vault>,
    store: &mut Store,
    item: Ready,
    dirty: &mut Dirty,
    opened: &mut Vec<String>,
) -> Result<Changed, Failure> {
    Ok(match item {
        Ready::Key(mut record) => {
            if let Some(old) = store.key(&record.name) {
                record.created = old.created.clone();
                record.rotated = Some(time::stamp());
            }
            let key = record.view();
            store.forget_key(&record.name);
            store.keys.push(record);
            dirty.keys = true;
            dirty.vetted = true;
            Changed::Key(KeyOut { key })
        }
        Ready::Rotated(name, sealed) => {
            let Some(key) = store.keys.iter_mut().find(|key| key.name == name) else {
                return Err(missing("vault key", &name));
            };
            key.nonce = sealed.nonce;
            key.secret = sealed.secret;
            key.rotated = Some(time::stamp());
            dirty.keys = true;
            Changed::Key(KeyOut { key: key.view() })
        }
        Ready::Deleted(name) => {
            store.forget_key(&name);
            dirty.keys = true;
            dirty.vetted = true;
            dirty.tokens = true;
            Changed::Deleted(Deleted { deleted: name })
        }
        Ready::Token(record, token, revoke) => {
            let out = token_out(&record, &token, &revoke);
            store.tokens.push(*record);
            dirty.tokens = true;
            Changed::Token(out)
        }
        Ready::Revoked(id) => {
            store.tokens.retain(|t| t.id != id);
            dirty.tokens = true;
            Changed::Revoked(Revoked { revoked: id })
        }
        Ready::Sandbox(name, endpoint, throwaway) => {
            let endpoint = vault.open_endpoint(&name, &endpoint).map_err(internal)?;
            opened.push(name.clone());
            let sandbox = Sandbox {
                name,
                endpoint,
                throwaway,
            };
            store.sandboxes.push(sandbox.clone());
            dirty.sandboxes = true;
            Changed::Sandbox(SandboxOut { sandbox })
        }
        Ready::Removed(name) => {
            store.forget_sandbox(&name);
            dirty.sandboxes = true;
            dirty.tokens = true;
            Changed::Removed(Removed { removed: name })
        }
    })
}

fn apply(vault: &Arc<Vault>, steps: Vec<Step>) -> Result<Vec<Value>, Failure> {
    let mut ready = ready(vault, steps)?;
    let tokened: HashSet<String> = ready
        .iter()
        .filter_map(|item| match item {
            Ready::Token(record, ..) => Some(record.sandbox.clone()),
            _ => None,
        })
        .collect();
    for item in &mut ready {
        if let Ready::Sandbox(name, _, throwaway) = item {
            *throwaway = tokened.contains(name.as_str());
        }
    }
    let removed: Vec<String> = ready
        .iter()
        .filter_map(|item| match item {
            Ready::Removed(name) => Some(name.clone()),
            _ => None,
        })
        .collect();
    let mut opened = Vec::new();
    let applied = {
        let mut store = vault.store();
        let before = (
            store.keys.clone(),
            store.tokens.clone(),
            store.sandboxes.clone(),
            store.vetted.clone(),
        );
        let mut dirty = Dirty::default();
        let applied = ready
            .into_iter()
            .map(|item| change(vault, &mut store, item, &mut dirty, &mut opened).map(api::value))
            .collect::<Result<Vec<_>, _>>()
            .and_then(|outs| dirty.save(&store).map(|_| outs).map_err(internal));
        if applied.is_err() {
            (store.keys, store.tokens, store.sandboxes, store.vetted) = before;
            let _ = dirty.save(&store);
        }
        applied
    };
    let closing = match &applied {
        Ok(_) => removed,
        Err(_) => opened,
    };
    for name in closing {
        vault.close_endpoint(&name);
    }
    applied
}
