use super::{with, Harnesses};
use crate::{Options, State};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use harness::{Agent, ModelChoice, Registry, Setup};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;
use utoipa_axum::router::OpenApiRouter;

fn fake(id: &str, install: &str, sign_in: Option<(&str, &str)>) -> Setup {
    Setup {
        id: id.into(),
        label: format!("Fake {id}"),
        command: format!("ai2-fake-{id}-not-on-path"),
        install: install.into(),
        sign_in: sign_in.map(|(line, _)| line.into()),
        sign_in_hint: sign_in.map(|(_, hint)| hint.into()),
        logo: Some(format!("{id}.svg")),
        agent: Some(Agent {
            models: vec![ModelChoice::new("big", "Big")],
            headless: None,
        }),
        ..Setup::default()
    }
}

fn registry() -> Registry {
    let mut registry = Registry::default();
    registry.put(
        fake(
            "quick",
            "printf 'Fetching quick 1.0\\n'; printf 'Installed quick\\n'",
            Some((
                "printf 'Open https://example.com/device\\n'; printf 'Enter code WXYZ-9876\\n'; printf 'API key: '; read -r key; echo \"got $key\"",
                "Paste an API key when asked.",
            )),
        ),
        None,
    );
    registry.put(fake("broken", "echo nope; exit 3", None), None);
    registry.put(
        fake(
            "waits",
            "printf 'Continue? '; read -r answer; echo \"then $answer\"",
            None,
        ),
        None,
    );
    registry
}

struct Fixture {
    _dir: tempfile::TempDir,
    state: std::path::PathBuf,
    router: Router,
    runtime: tokio::runtime::Runtime,
}

impl Fixture {
    fn new(registry: &Registry) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let state = dir.path().join("state");
        let harnesses = Arc::new(Harnesses::new(
            registry,
            dir.path().join("home"),
            state.clone(),
        ));
        let options = Options {
            home: dir.path().to_path_buf(),
            admin: dir.path().join("admin.sock"),
            agents: ::agents::Agents::local(dir.path(), ::harness::Registry::public),
        };
        let shared = Arc::new(State::new(1, "nonce".into(), &options).unwrap());
        let (router, _) = OpenApiRouter::new()
            .nest("/api", with(harnesses))
            .split_for_parts();
        Self {
            _dir: dir,
            state,
            router: router.with_state(shared),
            runtime: tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap(),
        }
    }

    fn send(&self, method: &str, path: &str, body: Option<Value>) -> (StatusCode, String) {
        self.send_with(method, path, body, &[])
    }

    fn send_with(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        headers: &[(&str, &str)],
    ) -> (StatusCode, String) {
        let mut request = Request::builder().method(method).uri(path);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
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
            let body = tokio::time::timeout(
                Duration::from_secs(30),
                axum::body::to_bytes(response.into_body(), usize::MAX),
            )
            .await
            .expect("the stream ended")
            .unwrap();
            (status, String::from_utf8_lossy(&body).into_owned())
        })
    }

    fn json(&self, method: &str, path: &str, body: Option<Value>) -> (StatusCode, Value) {
        let (status, text) = self.send(method, path, body);
        (status, serde_json::from_str(&text).unwrap_or(Value::Null))
    }

    fn harness(&self, id: &str) -> Value {
        let (status, list) = self.json("GET", "/api/harnesses", None);
        assert_eq!(status, StatusCode::OK);
        list["harnesses"]
            .as_array()
            .unwrap()
            .iter()
            .find(|harness| harness["id"] == id)
            .cloned()
            .unwrap()
    }

    fn start(&self, id: &str, action: &str) -> String {
        let (status, started) = self.json("POST", &format!("/api/harnesses/{id}/{action}"), None);
        assert_eq!(status, StatusCode::ACCEPTED, "{started}");
        assert_eq!(started["job"]["running"], true);
        started["job"]["id"].as_str().unwrap().to_string()
    }
}

#[derive(Debug)]
struct Sent {
    event: String,
    id: usize,
    data: Value,
}

fn parse(stream: &str) -> Vec<Sent> {
    stream
        .split("\n\n")
        .filter_map(|block| {
            let mut event = None;
            let mut id = None;
            let mut data = None;
            for line in block.lines() {
                if let Some(value) = line.strip_prefix("event: ") {
                    event = Some(value.to_string());
                } else if let Some(value) = line.strip_prefix("id: ") {
                    id = value.parse().ok();
                } else if let Some(value) = line.strip_prefix("data: ") {
                    data = serde_json::from_str(value).ok();
                }
            }
            Some(Sent {
                event: event?,
                id: id?,
                data: data?,
            })
        })
        .collect()
}

fn lines(sent: &[Sent]) -> Vec<String> {
    sent.iter()
        .filter(|sent| sent.event == "line")
        .map(|sent| sent.data["text"].as_str().unwrap().to_string())
        .collect()
}

fn known(state: &Path) -> Value {
    std::fs::read_to_string(state.join("harness.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null)
}

#[test]
fn the_list_names_every_harness_with_its_state_models_logo_and_what_signing_in_asks() {
    let fixture = Fixture::new(&registry());
    let (status, list) = fixture.json("GET", "/api/harnesses", None);
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<&str> = list["harnesses"]
        .as_array()
        .unwrap()
        .iter()
        .map(|harness| harness["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["quick", "broken", "waits"]);
    let quick = fixture.harness("quick");
    assert_eq!(quick["label"], "Fake quick");
    assert_eq!(quick["kind"], "agent");
    assert_eq!(quick["logo"], "quick.svg");
    assert_eq!(quick["models"], json!([{ "id": "big", "label": "Big" }]));
    assert_eq!(quick["can_sign_in"], true);
    assert_eq!(quick["asks"], "key");
    assert_eq!(quick["installed"], false);
    assert_eq!(quick["signed_in"], false);
    assert_eq!(quick["connected"], false);
    assert_eq!(quick["job"], Value::Null);
    assert_eq!(fixture.harness("broken")["can_sign_in"], false);
}

#[test]
fn the_public_registry_lists_fx_and_says_which_sign_ins_ask_for_a_key_or_a_code() {
    let fixture = Fixture::new(&Registry::public());
    let fx = fixture.harness("fx");
    assert_eq!(fx["label"], "fx");
    assert_eq!(fx["asks"], "key");
    let claude = fixture.harness("claude");
    assert_eq!(claude["logo"], "claude.svg");
    assert_eq!(claude["asks"], "code");
    assert_eq!(fixture.harness("codex")["asks"], Value::Null);
    assert_eq!(fixture.harness("deepseek")["asks"], "key");
    assert_eq!(fixture.harness("pi")["can_sign_in"], false);
    assert_eq!(fixture.harness("gh")["kind"], "tool");
}

#[test]
fn an_install_streams_its_output_ends_with_done_and_is_remembered() {
    let fixture = Fixture::new(&registry());
    let job = fixture.start("quick", "install");
    let (status, stream) = fixture.send("GET", &format!("/api/harnesses/jobs/{job}/events"), None);
    assert_eq!(status, StatusCode::OK);
    let sent = parse(&stream);
    let seen = lines(&sent);
    assert!(seen[0].starts_with("$ printf"), "{seen:?}");
    assert!(seen.contains(&"Fetching quick 1.0".to_string()), "{seen:?}");
    assert!(seen.contains(&"Installed quick".to_string()), "{seen:?}");
    let last = sent.last().unwrap();
    assert_eq!(last.event, "done");
    assert_eq!(last.data["ok"], true);
    assert_eq!(
        sent.iter().map(|sent| sent.id).collect::<Vec<_>>(),
        (0..sent.len()).collect::<Vec<_>>()
    );
    let quick = fixture.harness("quick");
    assert_eq!(quick["installed"], true);
    assert_eq!(quick["signed_in"], false);
    assert_eq!(quick["job"]["action"], "install");
    assert_eq!(quick["job"]["running"], false);
    assert_eq!(quick["job"]["ok"], true);
    assert_eq!(
        known(&fixture.state)["quick"],
        json!({ "installed": true, "signedIn": false })
    );

    let after = (sent.len() - 2).to_string();
    let (_, rest) = fixture.send_with(
        "GET",
        &format!("/api/harnesses/jobs/{job}/events"),
        None,
        &[("last-event-id", &after)],
    );
    let rest = parse(&rest);
    assert_eq!(rest.len(), 1);
    assert_eq!(rest[0].event, "done");
}

#[test]
fn a_sign_in_shows_its_link_and_code_takes_the_typed_key_and_never_shows_it_back() {
    let fixture = Fixture::new(&registry());
    let job = fixture.start("quick", "sign-in");
    let (status, refused) = fixture.json(
        "POST",
        &format!("/api/harnesses/jobs/{job}/answer"),
        Some(json!({ "text": "   " })),
    );
    assert_eq!(status, StatusCode::NO_CONTENT, "{refused}");
    let (status, _) = fixture.send(
        "POST",
        &format!("/api/harnesses/jobs/{job}/answer"),
        Some(json!({ "text": "sk-secret-123" })),
    );
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, stream) = fixture.send("GET", &format!("/api/harnesses/jobs/{job}/events"), None);
    let sent = parse(&stream);
    let link = sent.iter().find(|sent| sent.event == "link").unwrap();
    assert_eq!(link.data["url"], "https://example.com/device");
    let code = sent.iter().find(|sent| sent.event == "code").unwrap();
    assert_eq!(code.data["code"], "WXYZ-9876");
    let error = sent.iter().find(|sent| sent.event == "error").unwrap();
    assert_eq!(error.data["error"], "Paste the code first.");
    let seen = lines(&sent);
    assert!(
        seen.iter().any(|line| line.contains("got ••••")),
        "{seen:?}"
    );
    assert!(!stream.contains("sk-secret-123"));
    assert_eq!(sent.last().unwrap().data["ok"], true);
    let quick = fixture.harness("quick");
    assert_eq!(quick["signed_in"], true);
    assert_eq!(quick["connected"], true);

    let (status, ended) = fixture.json(
        "POST",
        &format!("/api/harnesses/jobs/{job}/answer"),
        Some(json!({ "text": "late" })),
    );
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(ended["error"], "ended");
}

#[test]
fn a_failed_install_says_how_it_ended_and_is_not_remembered() {
    let fixture = Fixture::new(&registry());
    let job = fixture.start("broken", "install");
    let (_, stream) = fixture.send("GET", &format!("/api/harnesses/jobs/{job}/events"), None);
    let done = parse(&stream).pop().unwrap();
    assert_eq!(done.event, "done");
    assert_eq!(done.data["ok"], false);
    assert_eq!(done.data["message"], "It ended with an error (3).");
    let broken = fixture.harness("broken");
    assert_eq!(broken["installed"], false);
    assert_eq!(broken["job"]["ok"], false);
    assert_eq!(broken["job"]["message"], "It ended with an error (3).");
    assert_eq!(known(&fixture.state), Value::Null);
}

#[test]
fn one_job_at_a_time_per_harness_and_unknown_harnesses_jobs_and_sign_ins_are_refused() {
    let fixture = Fixture::new(&registry());
    let job = fixture.start("waits", "install");
    let (status, busy) = fixture.json("POST", "/api/harnesses/waits/install", None);
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(busy["error"], "already_running");
    assert_eq!(fixture.harness("waits")["job"]["running"], true);
    let (status, _) = fixture.send(
        "POST",
        &format!("/api/harnesses/jobs/{job}/answer"),
        Some(json!({ "text": "yes" })),
    );
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, stream) = fixture.send("GET", &format!("/api/harnesses/jobs/{job}/events"), None);
    assert_eq!(parse(&stream).last().unwrap().data["ok"], true);
    fixture.start("waits", "install");

    let (status, missing) = fixture.json("POST", "/api/harnesses/nothing/install", None);
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(missing["message"], "There is no harness called nothing.");
    let (status, first_run) = fixture.json("POST", "/api/harnesses/broken/sign-in", None);
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(first_run["error"], "signs_in_on_first_run");
    let (status, _) = fixture.send("GET", "/api/harnesses/jobs/nope/events", None);
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = fixture.send(
        "POST",
        "/api/harnesses/jobs/nope/answer",
        Some(json!({ "text": "x" })),
    );
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[test]
fn the_api_describes_the_harness_routes() {
    let (_, document) = crate::api().split_for_parts();
    let paths: Vec<&String> = document.paths.paths.keys().collect();
    for path in [
        "/harnesses",
        "/harnesses/{id}/install",
        "/harnesses/{id}/sign-in",
        "/harnesses/jobs/{job}/events",
        "/harnesses/jobs/{job}/answer",
    ] {
        assert!(
            paths.iter().any(|known| *known == path),
            "{path} in {paths:?}"
        );
    }
}
