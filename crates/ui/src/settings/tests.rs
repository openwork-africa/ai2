use super::*;
use crate::state::State as UiState;
use crate::Options;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use harness::{Agent, Registry, Setup};
use serde_json::{json, Value};
use std::os::unix::net::UnixListener;
use std::path::Path as FilePath;
use std::sync::Mutex;
use tower::ServiceExt;
use vault::http::{self, Response as Answer};

#[derive(Clone, Debug)]
struct Seen {
    method: String,
    target: String,
    headers: Vec<(String, String)>,
    body: Value,
}

impl Seen {
    fn header(&self, name: &str) -> Option<&str> {
        http::header(&self.headers, name)
    }
}

type Handler = Box<dyn Fn(&Seen) -> (u16, Value) + Send>;

fn serve(socket: &FilePath, handler: Handler) -> Arc<Mutex<Vec<Seen>>> {
    let listener = UnixListener::bind(socket).unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let kept = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let Ok(request) = http::read_request(&mut stream) else {
                continue;
            };
            let asked = Seen {
                method: request.method.clone(),
                target: request.target.clone(),
                headers: request.headers.clone(),
                body: serde_json::from_slice(&request.body).unwrap_or(Value::Null),
            };
            let (status, reply) = handler(&asked);
            kept.lock().unwrap().push(asked);
            let _ = Answer::json(status, &reply).write_to(&mut stream);
        }
    });
    seen
}

fn agent(id: &str) -> Setup {
    Setup {
        id: id.into(),
        label: id.to_uppercase(),
        command: format!("{id}-surely-not-installed"),
        install: "true".into(),
        agent: Some(Agent {
            models: Vec::new(),
            headless: Some(harness::list::headless(id, "")),
        }),
        ..Setup::default()
    }
}

struct Fixture {
    dir: tempfile::TempDir,
    router: Router,
    runtime: tokio::runtime::Runtime,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("ai2s")
            .tempdir_in("/tmp")
            .unwrap();
        let mut registry = Registry::default();
        registry.put(agent("claude"), None);
        registry.put(agent("codex"), None);
        let harnesses = Arc::new(Harnesses::new(
            &registry,
            dir.path().join("user"),
            dir.path().to_path_buf(),
        ));
        let options = Options {
            home: dir.path().to_path_buf(),
            admin: dir.path().join("admin.sock"),
            agents: ::agents::Agents::local(dir.path(), ::harness::Registry::public),
        };
        let shared = Arc::new(UiState::new(1, "nonce".into(), &options).unwrap());
        let restart: Restart = Arc::new(|| Ok("The vault started again.".into()));
        let (router, _) = OpenApiRouter::new()
            .nest("/api", with(Context { harnesses, restart }))
            .split_for_parts();
        Self {
            dir,
            router: router.with_state(shared),
            runtime: tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap(),
        }
    }

    fn home(&self) -> &FilePath {
        self.dir.path()
    }

    fn send(&self, method: &str, path: &str, body: Option<Value>) -> (StatusCode, Value) {
        let request = Request::builder().method(method).uri(path);
        let request = match body {
            Some(body) => request
                .header("content-type", "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        }
        .unwrap();
        let router = self.router.clone();
        self.runtime.block_on(async move {
            let response = router.oneshot(request).await.unwrap();
            let status = response.status();
            let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            (
                status,
                serde_json::from_slice(&bytes).unwrap_or(Value::Null),
            )
        })
    }
}

fn mark_ready(home: &FilePath, id: &str) {
    let bin = home.join("user/.local/bin");
    std::fs::create_dir_all(&bin).unwrap();
    let file = bin.join(format!("{id}-surely-not-installed"));
    std::fs::write(&file, "#!/bin/sh\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    harness::state::States::at(home)
        .set(
            id,
            harness::state::State {
                installed: true,
                signed_in: true,
            },
        )
        .unwrap();
}

#[test]
fn the_settings_come_back_as_saved_and_an_unknown_choice_is_refused() {
    let ui = Fixture::new();
    let (status, fresh) = ui.send("GET", "/api/settings/security", None);
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fresh["saved"], false);
    assert_eq!(fresh["choice"]["runs"], "auto");
    let (_, vault) = ui.send("GET", "/api/settings/vault", None);
    assert_eq!(vault["choice"]["runs"], "model");

    let (status, saved) = ui.send(
        "PUT",
        "/api/settings/security",
        Some(json!({ "runs": "claude", "model": " opus ", "provider": "ignored" })),
    );
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["restart"], false);
    let file: Value =
        serde_json::from_str(&std::fs::read_to_string(ui.home().join("security.json")).unwrap())
            .unwrap();
    assert_eq!(file, json!({ "reviewer": "claude", "model": "opus" }));
    let (_, again) = ui.send("GET", "/api/settings/security", None);
    assert_eq!(again["saved"], true);
    assert_eq!(
        again["choice"],
        json!({ "runs": "claude", "provider": null, "model": "opus" })
    );

    let (status, _) = ui.send(
        "PUT",
        "/api/settings/security",
        Some(json!({ "runs": "none", "model": "x" })),
    );
    assert_eq!(status, StatusCode::OK);
    let (_, none) = ui.send("GET", "/api/settings/security", None);
    assert_eq!(none["choice"]["model"], Value::Null);

    for bad in [
        json!({ "runs": "gh" }),
        json!({ "runs": "model" }),
        json!({ "runs": "model", "provider": "nobody", "model": "m" }),
    ] {
        let (status, _) = ui.send("PUT", "/api/settings/security", Some(bad.clone()));
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
    }
    let (status, _) = ui.send(
        "PUT",
        "/api/settings/vault",
        Some(json!({ "runs": "auto" })),
    );
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = ui.send("GET", "/api/settings/other", None);
    assert_eq!(status, StatusCode::NOT_FOUND);

    providers::write_entry(
        ui.home(),
        "anthropic",
        "anthropic-messages",
        "https://api.anthropic.com",
    )
    .unwrap();
    let (status, vault) = ui.send(
        "PUT",
        "/api/settings/vault",
        Some(json!({ "runs": "model", "provider": "anthropic", "model": "claude-x" })),
    );
    assert_eq!(status, StatusCode::OK, "{vault}");
    assert_eq!(vault["restart"], true);
    let file: Value =
        serde_json::from_str(&std::fs::read_to_string(ui.home().join("vault.json")).unwrap())
            .unwrap();
    assert_eq!(
        file,
        json!({ "vetter": "model", "provider": "anthropic", "model": "claude-x" })
    );
}

#[test]
fn a_harness_tests_ready_only_once_installed_and_signed_in() {
    let ui = Fixture::new();
    let (_, tested) = ui.send(
        "POST",
        "/api/settings/security/test",
        Some(json!({ "runs": "codex" })),
    );
    assert_eq!(tested["ok"], false);
    mark_ready(ui.home(), "codex");
    let (_, tested) = ui.send(
        "POST",
        "/api/settings/security/test",
        Some(json!({ "runs": "codex" })),
    );
    assert_eq!(tested["ok"], true, "{tested}");
    let (_, auto) = ui.send(
        "POST",
        "/api/settings/security/test",
        Some(json!({ "runs": "auto" })),
    );
    assert_eq!(auto["ok"], true);
}

struct Daemon {
    seen: Arc<Mutex<Vec<Seen>>>,
    gateway: Arc<Mutex<Vec<Seen>>>,
}

fn daemon(home: &FilePath) -> Daemon {
    let gateway_socket = home.join("gw.sock");
    let gateway = serve(
        &gateway_socket,
        Box::new(|asked| {
            if asked.target.ends_with("/models") {
                (
                    200,
                    json!({ "data": [{ "id": "gpt-b" }, { "id": "gpt-a" }] }),
                )
            } else {
                (
                    200,
                    json!({ "choices": [{ "message": { "content": "OK" } }] }),
                )
            }
        }),
    );
    let host = Arc::new(Mutex::new(false));
    let key = Arc::new(Mutex::new(false));
    let path = gateway_socket.display().to_string();
    let seen = serve(
        &home.join("admin.sock"),
        Box::new(move |asked| {
            let parts: Vec<&str> = asked.target.trim_matches('/').split('/').collect();
            match (asked.method.as_str(), parts.as_slice()) {
                ("GET", ["sandboxes"]) => {
                    let listed = if *host.lock().unwrap() {
                        json!([{ "name": "host", "transport": "unix", "path": path }])
                    } else {
                        json!([])
                    };
                    (200, json!({ "sandboxes": listed }))
                }
                ("POST", ["batch"]) => {
                    *host.lock().unwrap() = true;
                    *key.lock().unwrap() = true;
                    (
                        200,
                        json!({ "results": [
                            { "key": { "name": "openai" } },
                            { "sandbox": { "name": "host" } },
                            { "id": "tok1", "token": "vt_one", "revoke": "rv1" }
                        ] }),
                    )
                }
                ("GET", ["keys", "openai"]) if *key.lock().unwrap() => (200, json!({ "key": {} })),
                ("GET", ["keys", _]) => (404, json!({ "error": "not_found", "message": "no key" })),
                ("DELETE", ["keys", "openai"]) => {
                    *key.lock().unwrap() = false;
                    (200, json!({ "deleted": "openai" }))
                }
                ("DELETE", ["tokens", _]) => (200, json!({ "revoked": "tok1" })),
                _ => (404, json!({ "error": "not_found", "message": "no route" })),
            }
        }),
    );
    Daemon { seen, gateway }
}

#[test]
fn a_provider_keeps_its_key_in_the_vault_and_only_vault_name_on_disk() {
    let ui = Fixture::new();
    let fake = daemon(ui.home());
    let (status, refused) = ui.send(
        "PUT",
        "/api/providers/openai",
        Some(json!({ "api": "openai-completions", "base_url": "https://api.openai.com/v1" })),
    );
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    for bad in [
        json!({ "api": "x", "base_url": "https://a", "key": "k" }),
        json!({ "api": "openai-completions", "base_url": "http://a", "key": "k" }),
    ] {
        let (status, _) = ui.send("PUT", "/api/providers/openai", Some(bad));
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    let (status, _) = ui.send(
        "PUT",
        "/api/providers/..",
        Some(json!({ "api": "openai-completions", "base_url": "https://a", "key": "k" })),
    );
    assert_ne!(status, StatusCode::OK);

    let (status, provider) = ui.send(
        "PUT",
        "/api/providers/openai",
        Some(json!({ "api": "openai-completions", "base_url": "https://api.openai.com/v1/", "key": "sk-secret-123" })),
    );
    assert_eq!(status, StatusCode::OK, "{provider}");
    assert_eq!(
        provider,
        json!({ "name": "openai", "api": "openai-completions", "base_url": "https://api.openai.com/v1", "key": "vault", "stored": true, "pass": true, "models": [] })
    );
    let batch = fake
        .seen
        .lock()
        .unwrap()
        .iter()
        .find(|seen| seen.target == "/batch")
        .cloned()
        .unwrap();
    assert_eq!(
        batch.body["actions"],
        json!([
            { "action": "put_key", "name": "openai", "secret": "sk-secret-123", "description": "The openai API key, for model calls", "approved_urls": ["https://api.openai.com/v1/"], "inject": "bearer" },
            { "action": "add_sandbox", "name": "host" },
            { "action": "create_token", "keys": ["openai"], "sandbox": "host", "mode": "app", "methods": "GET,POST", "url_prefixes": ["https://api.openai.com/v1/"] }
        ])
    );
    let written = std::fs::read_to_string(providers::providers_path(ui.home())).unwrap();
    assert!(!written.contains("sk-secret-123"));
    let written: Value = serde_json::from_str(&written).unwrap();
    assert_eq!(
        written["providers"]["openai"],
        json!({ "api": "openai-completions", "baseUrl": "https://api.openai.com/v1", "apiKey": "vault:openai", "models": [] })
    );
    let pass = std::fs::read_to_string(vault::app::file(ui.home(), "openai")).unwrap();
    assert!(pass.contains("vt_one") && !pass.contains("sk-secret-123"));
    for file in std::fs::read_dir(ui.home()).unwrap().flatten() {
        if file.file_type().unwrap().is_file() {
            let text = std::fs::read(file.path()).unwrap();
            assert!(
                !String::from_utf8_lossy(&text).contains("sk-secret-123"),
                "{:?}",
                file.path()
            );
        }
    }

    let (status, models) = ui.send("GET", "/api/providers/openai/models", None);
    assert_eq!(status, StatusCode::OK, "{models}");
    assert_eq!(models, json!({ "models": ["gpt-a", "gpt-b"] }));
    let (_, tested) = ui.send(
        "POST",
        "/api/providers/openai/test",
        Some(json!({ "model": "gpt-a" })),
    );
    assert_eq!(tested, json!({ "ok": true, "message": "gpt-a answered." }));
    let asked = fake.gateway.lock().unwrap().clone();
    assert_eq!(asked[0].target, "/openai/api.openai.com/v1/models");
    assert_eq!(asked[0].header("authorization"), Some("Bearer vt_one"));
    assert_eq!(
        asked[1].target,
        "/openai/api.openai.com/v1/chat/completions"
    );
    assert_eq!(asked[1].body["model"], "gpt-a");
    let (_, listed) = ui.send("GET", "/api/providers", None);
    assert_eq!(listed["providers"][0]["models"], json!(["gpt-a"]));

    let (status, saved) = ui.send(
        "PUT",
        "/api/settings/security",
        Some(json!({ "runs": "model", "provider": "openai", "model": "gpt-a" })),
    );
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (_, tested) = ui.send(
        "POST",
        "/api/settings/security/test",
        Some(json!({ "runs": "model", "provider": "openai", "model": "gpt-a" })),
    );
    assert_eq!(tested["ok"], true);

    let (status, removed) = ui.send("DELETE", "/api/providers/openai", None);
    assert_eq!(status, StatusCode::OK, "{removed}");
    let seen = fake.seen.lock().unwrap().clone();
    assert!(seen
        .iter()
        .any(|s| s.method == "DELETE" && s.target == "/keys/openai"));
    let revoked = seen
        .iter()
        .find(|s| s.method == "DELETE" && s.target == "/tokens/tok1")
        .unwrap();
    assert_eq!(revoked.header("x-vault-revoke"), Some("rv1"));
    assert!(!vault::app::file(ui.home(), "openai").exists());
    let (_, listed) = ui.send("GET", "/api/providers", None);
    assert_eq!(listed, json!({ "providers": [] }));
    let (status, _) = ui.send("DELETE", "/api/providers/openai", None);
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[test]
fn without_a_pass_models_are_typed_in() {
    let ui = Fixture::new();
    providers::write_entry(
        ui.home(),
        "muse",
        "openai-completions",
        "https://api.meta.ai/v1",
    )
    .unwrap();
    let (status, refused) = ui.send("GET", "/api/providers/muse/models", None);
    assert_eq!(status, StatusCode::CONFLICT, "{refused}");
    assert_eq!(refused["error"], "no_pass");
    let (_, listed) = ui.send("GET", "/api/providers", None);
    assert_eq!(listed["providers"][0]["stored"], false);
    assert_eq!(listed["providers"][0]["pass"], false);
}

#[test]
fn the_vault_restarts_on_asking_with_the_line_for_its_system() {
    let ui = Fixture::new();
    let (status, done) = ui.send("POST", "/api/settings/vault/restart", None);
    assert_eq!(status, StatusCode::OK);
    assert_eq!(done["ok"], true);
    assert_eq!(
        restart_line(true, 501, "africa.openwork.ai2-vault").line(),
        "launchctl kickstart -k gui/501/africa.openwork.ai2-vault"
    );
    assert_eq!(
        restart_line(false, 1000, "x").line(),
        "systemctl restart ai2-vault.service"
    );
}

#[test]
fn the_api_describes_the_settings_and_provider_routes() {
    let dir = tempfile::tempdir().unwrap();
    let (_, document) = crate::router(Arc::new(
        UiState::new(
            1,
            "n".into(),
            &Options {
                home: dir.path().to_path_buf(),
                admin: dir.path().join("admin.sock"),
                agents: ::agents::Agents::local(dir.path(), ::harness::Registry::public),
            },
        )
        .unwrap(),
    ));
    let paths: Vec<String> = document.paths.paths.keys().cloned().collect();
    for path in [
        "/api/settings/{page}",
        "/api/settings/{page}/test",
        "/api/settings/vault/restart",
        "/api/providers",
        "/api/providers/{name}",
        "/api/providers/{name}/models",
        "/api/providers/{name}/test",
    ] {
        assert!(paths.iter().any(|known| known == path), "{path}");
    }
}
