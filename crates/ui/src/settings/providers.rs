use super::Tested;
use crate::reply::error;
use crate::state::Shared;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::path::{Path as FilePath, PathBuf};
use std::time::Duration;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use vault::app::{self, AppToken, Call};

const HOST: &str = "host";
const APIS: [&str; 2] = ["openai-completions", "anthropic-messages"];
const WAIT: Duration = Duration::from_secs(60);
const PASSPHRASE: &str = "x-vault-passphrase";

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub struct Provider {
    pub name: String,
    pub api: String,
    pub base_url: String,
    pub key: String,
    pub stored: bool,
    pub pass: bool,
    pub models: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Providers {
    pub providers: Vec<Provider>,
}

#[derive(Deserialize, ToSchema)]
pub struct NewProvider {
    pub api: String,
    pub base_url: String,
    #[serde(default)]
    pub key: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Models {
    pub models: Vec<String>,
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct Asked {
    #[serde(default)]
    pub model: Option<String>,
}

pub struct Listed {
    pub name: String,
    pub api: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub models: Vec<String>,
}

pub struct Failure {
    status: u16,
    code: String,
    message: String,
}

fn fail(status: u16, code: &str, message: impl Into<String>) -> Failure {
    Failure {
        status,
        code: code.into(),
        message: message.into(),
    }
}

impl From<anyhow::Error> for Failure {
    fn from(failure: anyhow::Error) -> Self {
        fail(500, "internal", format!("{failure:#}"))
    }
}

impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        error(self.status, &self.code, &self.message)
    }
}

pub fn routes() -> OpenApiRouter<Shared> {
    OpenApiRouter::new()
        .routes(routes!(list))
        .routes(routes!(put_provider, delete_provider))
        .routes(routes!(models))
        .routes(routes!(test_provider))
}

pub fn providers_path(home: &FilePath) -> PathBuf {
    std::env::var_os("AI2_PROVIDERS")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join("providers.json"))
}

fn file(home: &FilePath) -> anyhow::Result<Value> {
    let path = providers_path(home);
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            let value: Value = serde_json::from_str(&text)
                .map_err(|e| anyhow::anyhow!("{} is not JSON: {e}", path.display()))?;
            if !value.is_object() {
                anyhow::bail!("{} is not a providers file", path.display());
            }
            Ok(value)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({ "providers": {} })),
        Err(e) => Err(anyhow::anyhow!("reading {}: {e}", path.display())),
    }
}

fn text(value: &Value, field: &str) -> String {
    value[field].as_str().unwrap_or_default().to_string()
}

pub fn read(home: &FilePath) -> anyhow::Result<Vec<Listed>> {
    let all = file(home)?;
    let Some(providers) = all["providers"].as_object() else {
        return Ok(Vec::new());
    };
    Ok(providers
        .iter()
        .map(|(name, entry)| Listed {
            name: name.clone(),
            api: text(entry, "api"),
            base_url: text(entry, "baseUrl"),
            api_key: entry["apiKey"].as_str().map(str::to_string),
            models: entry["models"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|model| model["id"].as_str().map(str::to_string))
                .collect(),
        })
        .collect())
}

fn write(home: &FilePath, change: impl FnOnce(&mut Map<String, Value>)) -> anyhow::Result<()> {
    let mut all = file(home)?;
    if !all["providers"].is_object() {
        all["providers"] = json!({});
    }
    if let Some(providers) = all["providers"].as_object_mut() {
        change(providers);
    }
    harness::settings::write_private(
        &providers_path(home),
        &(serde_json::to_string_pretty(&all)? + "\n"),
    )
}

pub fn write_entry(home: &FilePath, name: &str, api: &str, base_url: &str) -> anyhow::Result<()> {
    write(home, |providers| {
        let entry = providers
            .entry(name.to_string())
            .or_insert_with(|| json!({}));
        if !entry.is_object() {
            *entry = json!({});
        }
        entry["api"] = json!(api);
        entry["baseUrl"] = json!(base_url);
        entry["apiKey"] = json!(format!("vault:{name}"));
        if !entry["models"].is_array() {
            entry["models"] = json!([]);
        }
    })
}

pub fn remember_model(home: &FilePath, name: &str, model: &str) -> anyhow::Result<()> {
    write(home, |providers| {
        let Some(models) = providers
            .get_mut(name)
            .and_then(|entry| entry.get_mut("models"))
            .and_then(Value::as_array_mut)
        else {
            return;
        };
        if !models.iter().any(|known| known["id"] == model) {
            models.push(json!({ "id": model }));
        }
    })
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}

struct Admin<'a> {
    socket: &'a FilePath,
    passphrase: Option<&'a str>,
}

impl Admin<'_> {
    fn call(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> Result<(u16, Value), Failure> {
        let headers: Vec<(&str, &str)> = self
            .passphrase
            .map(|p| ("X-Vault-Passphrase", p))
            .into_iter()
            .collect();
        vault::client::call(self.socket, method, path, body, &headers)
            .map_err(|failure| fail(502, "daemon_unavailable", format!("{failure:#}")))
    }

    fn ok(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, Failure> {
        let (status, value) = self.call(method, path, body)?;
        if status == 200 {
            return Ok(value);
        }
        Err(fail(
            status,
            value["error"].as_str().unwrap_or("vault_refused"),
            value["message"].as_str().unwrap_or("The vault said no."),
        ))
    }

    fn has_key(&self, name: &str) -> bool {
        self.call("GET", &format!("/keys/{name}"), None)
            .is_ok_and(|(status, _)| status == 200)
    }

    fn host(&self) -> Result<Option<Value>, Failure> {
        let listed = self.ok("GET", "/sandboxes", None)?;
        Ok(listed["sandboxes"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|sandbox| sandbox["name"] == HOST)
            .cloned())
    }
}

fn inject(api: &str) -> &'static str {
    if api == "anthropic-messages" {
        "header:x-api-key"
    } else {
        "bearer"
    }
}

fn token_action(name: &str, prefix: &str) -> Value {
    json!({
        "action": "create_token",
        "keys": [name],
        "sandbox": HOST,
        "mode": "app",
        "methods": "GET,POST",
        "url_prefixes": [prefix],
    })
}

fn keep_pass(home: &FilePath, admin: &Admin, name: &str, made: &Value) -> Result<(), Failure> {
    let host = admin
        .host()?
        .ok_or_else(|| fail(502, "no_sandbox", "The vault has no host sandbox."))?;
    let socket = host["path"]
        .as_str()
        .ok_or_else(|| fail(502, "no_socket", "The host sandbox has no socket."))?;
    let pass = AppToken {
        id: text(made, "id"),
        token: text(made, "token"),
        revoke: text(made, "revoke"),
        socket: PathBuf::from(socket),
    };
    if pass.token.is_empty() {
        return Err(fail(502, "no_token", "The vault made no pass."));
    }
    let old = app::load(home, name);
    app::save(home, name, &pass)?;
    if let Some(old) = old.filter(|old| old.id != pass.id) {
        revoke(admin.socket, &old);
    }
    Ok(())
}

fn revoke(socket: &FilePath, pass: &AppToken) {
    let _ = vault::client::call(
        socket,
        "DELETE",
        &format!("/tokens/{}", pass.id),
        None,
        &[("X-Vault-Revoke", pass.revoke.as_str())],
    );
}

pub fn save(
    home: &FilePath,
    socket: &FilePath,
    name: &str,
    asked: &NewProvider,
    passphrase: Option<&str>,
) -> Result<Provider, Failure> {
    if !valid_name(name) {
        return Err(fail(
            400,
            "bad_request",
            "A name is letters, digits, - _ and ., at most 64.",
        ));
    }
    if !APIS.contains(&asked.api.as_str()) {
        return Err(fail(
            400,
            "bad_request",
            "The API style is openai-completions or anthropic-messages.",
        ));
    }
    let base_url = asked.base_url.trim().trim_end_matches('/').to_string();
    let prefix = app::prefix(&base_url)
        .map_err(|_| fail(400, "bad_request", "The base URL is a plain https address."))?;
    let admin = Admin { socket, passphrase };
    let key = asked
        .key
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty());
    match key {
        Some(secret) => {
            let mut actions = vec![json!({
                "action": "put_key",
                "name": name,
                "secret": secret,
                "description": format!("The {name} API key, for model calls"),
                "approved_urls": [prefix],
                "inject": inject(&asked.api),
            })];
            if admin.host()?.is_none() {
                actions.push(json!({ "action": "add_sandbox", "name": HOST }));
            }
            actions.push(token_action(name, &prefix));
            let done = admin.ok("POST", "/batch", Some(&json!({ "actions": actions })))?;
            let made = done["results"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|result| result["token"].is_string())
                .cloned()
                .ok_or_else(|| fail(502, "no_token", "The vault made no pass."))?;
            keep_pass(home, &admin, name, &made)?;
        }
        None => {
            if !admin.has_key(name) {
                return Err(fail(
                    400,
                    "bad_request",
                    "The vault has no key for this provider yet.",
                ));
            }
            if app::load(home, name).is_none() {
                let made = admin.ok("POST", "/tokens", Some(&token_action(name, &prefix)))?;
                keep_pass(home, &admin, name, &made)?;
            }
        }
    }
    write_entry(home, name, &asked.api, &base_url)?;
    view(home, socket, name)?.ok_or_else(|| fail(500, "internal", "The provider was not kept."))
}

pub fn remove(
    home: &FilePath,
    socket: &FilePath,
    name: &str,
    passphrase: Option<&str>,
) -> Result<(), Failure> {
    let listed = read(home)?;
    let Some(known) = listed.iter().find(|known| known.name == name) else {
        return Err(fail(404, "not_found", "There is no such provider."));
    };
    let admin = Admin { socket, passphrase };
    if known.api_key.as_deref() == Some(&format!("vault:{name}")) && admin.has_key(name) {
        admin.ok("DELETE", &format!("/keys/{name}"), None)?;
    }
    if let Some(pass) = app::load(home, name) {
        revoke(socket, &pass);
        let _ = std::fs::remove_file(app::file(home, name));
    }
    write(home, |providers| {
        providers.remove(name);
    })?;
    Ok(())
}

pub fn view(home: &FilePath, socket: &FilePath, name: &str) -> anyhow::Result<Option<Provider>> {
    Ok(read(home)?
        .into_iter()
        .find(|known| known.name == name)
        .map(|known| shown(home, socket, known)))
}

fn shown(home: &FilePath, socket: &FilePath, known: Listed) -> Provider {
    let admin = Admin {
        socket,
        passphrase: None,
    };
    let (key, stored) = match known.api_key.as_deref() {
        Some(value) if value == format!("vault:{}", known.name) => {
            ("vault", admin.has_key(&known.name))
        }
        Some(value) if value.starts_with("vault:") => ("vault", false),
        Some(_) => ("other", false),
        None => ("none", false),
    };
    Provider {
        pass: app::load(home, &known.name).is_some(),
        name: known.name,
        api: known.api,
        base_url: known.base_url,
        key: key.into(),
        stored,
        models: known.models,
    }
}

fn reach(home: &FilePath, name: &str) -> Result<(Listed, AppToken), Failure> {
    let known = read(home)?
        .into_iter()
        .find(|known| known.name == name)
        .ok_or_else(|| fail(404, "not_found", "There is no such provider."))?;
    let pass = app::load(home, name)
        .ok_or_else(|| fail(409, "no_pass", "The vault holds no pass for this provider."))?;
    Ok((known, pass))
}

fn through(
    pass: &AppToken,
    name: &str,
    api: &str,
    method: &str,
    url: &str,
    body: &[u8],
) -> Result<(u16, Value), Failure> {
    let mut headers = vec![("Content-Type", "application/json")];
    if api == "anthropic-messages" {
        headers.push(("anthropic-version", "2023-06-01"));
    }
    let reply = pass
        .call(
            name,
            &Call {
                method,
                url,
                headers: &headers,
                body,
                wait: WAIT,
            },
        )
        .map_err(|failure| fail(502, "gateway_unavailable", format!("{failure:#}")))?;
    Ok((
        reply.status,
        serde_json::from_slice(&reply.body).unwrap_or(Value::Null),
    ))
}

fn said(status: u16, answer: &Value) -> String {
    let why = answer["error"]["message"]
        .as_str()
        .or_else(|| answer["message"].as_str())
        .or_else(|| answer["reason"].as_str())
        .or_else(|| answer["error"].as_str())
        .unwrap_or("no reason given");
    format!("It answered {status}: {why}")
}

pub fn list_models(home: &FilePath, name: &str) -> Result<Vec<String>, Failure> {
    let (known, pass) = reach(home, name)?;
    let base = known.base_url.trim_end_matches('/');
    let url = if known.api == "anthropic-messages" {
        format!("{base}/v1/models")
    } else {
        format!("{base}/models")
    };
    let (status, answer) = through(&pass, name, &known.api, "GET", &url, b"")?;
    if !(200..300).contains(&status) {
        return Err(fail(502, "provider_refused", said(status, &answer)));
    }
    let mut ids: Vec<String> = answer["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|model| model["id"].as_str().map(str::to_string))
        .collect();
    ids.sort();
    ids.dedup();
    Ok(ids)
}

pub fn try_model(home: &FilePath, name: &str, model: Option<&str>) -> Tested {
    let Some(model) = model.map(str::trim).filter(|m| !m.is_empty()) else {
        return match list_models(home, name) {
            Ok(models) => Tested {
                ok: true,
                message: format!("It answered with {} models.", models.len()),
            },
            Err(failure) => Tested {
                ok: false,
                message: failure.message,
            },
        };
    };
    let asked = || -> Result<Tested, Failure> {
        let (known, pass) = reach(home, name)?;
        let base = known.base_url.trim_end_matches('/');
        let (url, body) = if known.api == "anthropic-messages" {
            (
                format!("{base}/v1/messages"),
                json!({ "model": model, "max_tokens": 8, "messages": [{ "role": "user", "content": "Say OK." }] }),
            )
        } else {
            (
                format!("{base}/chat/completions"),
                json!({ "model": model, "max_tokens": 8, "messages": [{ "role": "user", "content": "Say OK." }] }),
            )
        };
        let (status, answer) = through(
            &pass,
            name,
            &known.api,
            "POST",
            &url,
            body.to_string().as_bytes(),
        )?;
        Ok(if (200..300).contains(&status) {
            Tested {
                ok: true,
                message: format!("{model} answered."),
            }
        } else {
            Tested {
                ok: false,
                message: said(status, &answer),
            }
        })
    };
    asked().unwrap_or_else(|failure| Tested {
        ok: false,
        message: failure.message,
    })
}

pub async fn test(state: &Shared, name: &str, model: Option<&str>) -> Tested {
    let home = state.home.clone();
    let (name, model) = (name.to_string(), model.map(str::to_string));
    tokio::task::spawn_blocking(move || try_model(&home, &name, model.as_deref()))
        .await
        .unwrap_or_else(|failure| Tested {
            ok: false,
            message: failure.to_string(),
        })
}

fn passphrase(headers: &HeaderMap) -> Result<Option<String>, Failure> {
    match headers.get(PASSPHRASE).map(|v| v.to_str()) {
        None => Ok(None),
        Some(Ok(p)) if !p.chars().any(char::is_control) => Ok(Some(p.to_string())),
        Some(_) => Err(fail(
            400,
            "bad_request",
            "The passphrase has characters it may not.",
        )),
    }
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, Failure> + Send + 'static,
) -> Result<T, Failure> {
    tokio::task::spawn_blocking(work)
        .await
        .unwrap_or_else(|failure| Err(fail(500, "internal", failure.to_string())))
}

#[utoipa::path(
    get,
    path = "/providers",
    tag = "providers",
    responses((status = 200, description = "The model providers in providers.json; never a key", body = Providers))
)]
async fn list(State(state): State<Shared>) -> Response {
    let (home, socket) = (state.home.clone(), state.admin.clone());
    blocking(move || {
        Ok(read(&home)?
            .into_iter()
            .map(|known| shown(&home, &socket, known))
            .collect::<Vec<_>>())
    })
    .await
    .map(|providers| Json(Providers { providers }).into_response())
    .unwrap_or_else(IntoResponse::into_response)
}

#[utoipa::path(
    put,
    path = "/providers/{name}",
    tag = "providers",
    params(
        ("name" = String, Path, description = "The provider's name; its key is kept in the vault under the same name"),
        ("X-Vault-Passphrase" = Option<String>, Header, description = "On Linux, when the vault asks for its passphrase")
    ),
    request_body = NewProvider,
    responses(
        (status = 200, description = "Kept: the key in the vault, the provider in providers.json with apiKey vault:<name>", body = Provider),
        (status = 400, description = "The name, API style or base URL is not right, or no key was given for a new provider", body = crate::Reply),
        (status = 401, description = "The vault needs you present", body = crate::Reply),
        (status = 502, description = "The vault did not answer", body = crate::Reply)
    )
)]
async fn put_provider(
    State(state): State<Shared>,
    Path(name): Path<String>,
    headers: HeaderMap,
    Json(asked): Json<NewProvider>,
) -> Response {
    let passphrase = match passphrase(&headers) {
        Ok(passphrase) => passphrase,
        Err(failure) => return failure.into_response(),
    };
    let (home, socket) = (state.home.clone(), state.admin.clone());
    blocking(move || save(&home, &socket, &name, &asked, passphrase.as_deref()))
        .await
        .map(|provider| Json(provider).into_response())
        .unwrap_or_else(IntoResponse::into_response)
}

#[utoipa::path(
    delete,
    path = "/providers/{name}",
    tag = "providers",
    params(
        ("name" = String, Path, description = "The provider's name"),
        ("X-Vault-Passphrase" = Option<String>, Header, description = "On Linux, when the vault asks for its passphrase")
    ),
    responses(
        (status = 200, description = "Taken out of providers.json, its key out of the vault and its pass ended", body = Tested),
        (status = 401, description = "The vault needs you present", body = crate::Reply),
        (status = 404, description = "There is no such provider", body = crate::Reply)
    )
)]
async fn delete_provider(
    State(state): State<Shared>,
    Path(name): Path<String>,
    headers: HeaderMap,
) -> Response {
    let passphrase = match passphrase(&headers) {
        Ok(passphrase) => passphrase,
        Err(failure) => return failure.into_response(),
    };
    let (home, socket) = (state.home.clone(), state.admin.clone());
    blocking(move || remove(&home, &socket, &name, passphrase.as_deref()))
        .await
        .map(|()| {
            Json(Tested {
                ok: true,
                message: "Taken out.".into(),
            })
            .into_response()
        })
        .unwrap_or_else(IntoResponse::into_response)
}

#[utoipa::path(
    get,
    path = "/providers/{name}/models",
    tag = "providers",
    params(("name" = String, Path, description = "The provider's name")),
    responses(
        (status = 200, description = "The provider's models, asked through the vault", body = Models),
        (status = 404, description = "There is no such provider", body = crate::Reply),
        (status = 409, description = "The vault holds no pass for it, so a model is typed in", body = crate::Reply),
        (status = 502, description = "The provider or the vault did not answer", body = crate::Reply)
    )
)]
async fn models(State(state): State<Shared>, Path(name): Path<String>) -> Response {
    let home = state.home.clone();
    blocking(move || list_models(&home, &name))
        .await
        .map(|models| Json(Models { models }).into_response())
        .unwrap_or_else(IntoResponse::into_response)
}

#[utoipa::path(
    post,
    path = "/providers/{name}/test",
    tag = "providers",
    params(("name" = String, Path, description = "The provider's name")),
    request_body = Asked,
    responses((status = 200, description = "Whether the provider answered through the vault: a model list, or one short reply from the model given", body = Tested))
)]
async fn test_provider(
    State(state): State<Shared>,
    Path(name): Path<String>,
    body: Option<Json<Asked>>,
) -> Response {
    let model = body.and_then(|Json(asked)| asked.model);
    let tested = test(&state, &name, model.as_deref()).await;
    if tested.ok {
        if let Some(model) = model.as_deref().map(str::trim).filter(|m| !m.is_empty()) {
            let _ = remember_model(&state.home, &name, model);
        }
    }
    Json(tested).into_response()
}
