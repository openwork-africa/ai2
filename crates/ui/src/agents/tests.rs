use crate::tests::Ui;
use crate::Options;
use agents::{Agents, Review, Reviewed};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;

const JSON: &[(&str, &str)] = &[("Content-Type", "application/json")];
const HASH: &str = "fedcba9876543210";

struct FakeReview;

impl Review for FakeReview {
    fn review(
        &self,
        folder: &Path,
        objective: &str,
        say: &mut dyn FnMut(String),
    ) -> anyhow::Result<Reviewed> {
        say("ai2: rating the Run Risk Factor".into());
        Ok(Reviewed {
            hash: HASH.into(),
            review: serde_json::json!({
                "objective": objective,
                "edited": folder.join("edited.txt").is_file()
            }),
        })
    }
}

fn fake_agents(home: &Path) -> Agents {
    let script = home.join("fake-agent");
    std::fs::write(
        &script,
        "#!/bin/sh\necho \"working on: $1\"\necho \"$1\" > edited.txt\necho 'a warning' >&2\n",
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let command = script.display().to_string();
    let registry = move || {
        let mut registry = harness::Registry::default();
        registry.put(
            harness::Setup {
                id: "fake".into(),
                label: "Fake".into(),
                command: command.clone(),
                install: "true".into(),
                agent: Some(harness::Agent::default()),
                ..harness::Setup::default()
            },
            None,
        );
        registry
    };
    let ask: agents::Ask = Arc::new(|_: &str, _: &str| {
        Ok(format!(
            "<system_prompt>\nYou are Scout, a careful reviewer of code. {}\n</system_prompt>",
            "You speak plainly and briefly, in their language. ".repeat(8)
        ))
    });
    Agents::local(home, registry)
        .with_review(Arc::new(FakeReview))
        .with_enhancer(Arc::new(move |_: Option<&str>| Ok(ask.clone())))
}

fn ui(home: &Path) -> Ui {
    Ui::start_with(Options {
        home: home.to_path_buf(),
        admin: home.join("admin.sock"),
        agents: fake_agents(home),
    })
}

#[test]
fn agents_are_kept_shown_expanded_and_forgotten_by_name() {
    let dir = tempfile::tempdir().unwrap();
    let ui = ui(dir.path());
    let empty = ui.api("GET", "/api/agents", &[], "");
    assert_eq!(empty.status, 200);
    assert_eq!(empty.json(), serde_json::json!({ "agents": [] }));

    let body =
        serde_json::json!({ "name": "scout", "harness": "fake", "folder": dir.path() }).to_string();
    let made = ui.api("POST", "/api/agents", JSON, &body);
    assert_eq!(made.status, 201, "{}", made.text());
    assert_eq!(made.json()["agent"]["harness"], "fake");
    let again = ui.api("POST", "/api/agents", JSON, &body);
    assert_eq!(again.status, 200);
    let wrong = ui.api(
        "POST",
        "/api/agents",
        JSON,
        r#"{"name":"scout","harness":"nope"}"#,
    );
    assert_eq!(wrong.status, 400);
    assert_eq!(wrong.json()["error"], "unknown_harness");

    let listed = ui.api("GET", "/api/agents", &[], "").json();
    assert_eq!(listed["agents"][0]["name"], "scout");
    assert_eq!(
        ui.api("GET", "/api/agents/scout", &[], "").json()["agent"]["name"],
        "scout"
    );
    let skipped = ui.api("POST", "/api/agents/scout/enhance", &[], "").json();
    assert_eq!(skipped["status"], "skipped");

    let terse = ui.api(
        "POST",
        "/api/agents",
        JSON,
        r#"{"name":"terse","harness":"fake","personality":"a careful reviewer"}"#,
    );
    let personality = &terse.json()["agent"]["personality"];
    assert_eq!(personality["seed"], "a careful reviewer");
    assert_eq!(personality["enhanced"], true);

    let removed = ui.api("DELETE", "/api/agents/scout", &[], "");
    assert_eq!(removed.status, 200);
    assert_eq!(removed.json(), serde_json::json!({ "removed": "scout" }));
    for (method, target) in [
        ("GET", "/api/agents/scout"),
        ("DELETE", "/api/agents/scout"),
        ("POST", "/api/agents/scout/enhance"),
    ] {
        let missing = ui.api(method, target, &[], "");
        assert_eq!(missing.status, 404, "{method} {target}");
        assert_eq!(missing.json()["error"], "not_found");
    }
}

#[test]
fn a_run_starts_streams_its_events_replays_from_the_last_one_and_ends_reviewed() {
    let dir = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let real = folder.path().canonicalize().unwrap();
    let ui = ui(dir.path());
    ui.api(
        "POST",
        "/api/agents",
        JSON,
        &serde_json::json!({ "name": "scout", "harness": "fake", "folder": folder.path() })
            .to_string(),
    );
    let started = ui.api(
        "POST",
        "/api/runs",
        JSON,
        r#"{"agent":"scout","prompt":"fix the login"}"#,
    );
    assert_eq!(started.status, 202, "{}", started.text());
    let run = started.json()["run"].clone();
    let id = run["id"].as_str().unwrap().to_string();
    assert_eq!(run["folder"], real.display().to_string());
    assert!(agents::valid_id(&id));

    let events = ui.api("GET", &format!("/api/runs/{id}/events"), &[], "");
    assert_eq!(events.status, 200);
    let text = events.text();
    let names: Vec<&str> = text
        .lines()
        .filter_map(|line| line.strip_prefix("event: "))
        .collect();
    assert_eq!(names.first(), Some(&"started"), "{text}");
    assert_eq!(
        names[names.len() - 4..],
        ["exited", "review", "reviewed", "done"],
        "{text}"
    );
    assert!(text.contains("working on: fix the login"), "{text}");
    assert!(
        text.contains(r#""stream":"err","text":"a warning""#),
        "{text}"
    );
    assert!(text.contains(&format!(r#""hash":"{HASH}""#)), "{text}");
    let last = names.len() - 1;
    assert!(text.contains(&format!("id: {last}")), "{text}");

    let replay = ui.api(
        "GET",
        &format!("/api/runs/{id}/events"),
        &[("Last-Event-ID", &(last - 1).to_string())],
        "",
    );
    let replay = replay.text();
    let replayed: Vec<&str> = replay
        .lines()
        .filter_map(|line| line.strip_prefix("event: "))
        .collect();
    assert_eq!(replayed, ["done"], "{replay}");

    let found = ui.api("GET", &format!("/api/runs/{id}"), &[], "").json();
    assert_eq!(found["run"]["state"], "ended");
    assert_eq!(found["run"]["exit"], 0);
    assert_eq!(found["run"]["review"], HASH);
    assert_eq!(
        std::fs::read_to_string(real.join("edited.txt")).unwrap(),
        "fix the login\n"
    );
    let listed = ui.api("GET", "/api/runs", &[], "").json();
    assert_eq!(listed["runs"][0]["id"], id.as_str());

    let missing = ui.api(
        "POST",
        "/api/runs",
        JSON,
        &serde_json::json!({ "harness": "fake", "folder": real.join("nope"), "prompt": "hi" })
            .to_string(),
    );
    assert_eq!(missing.status, 400);
    assert_eq!(missing.json()["error"], "not_a_folder");
    for target in ["/api/runs/0000000000000000", "/api/runs/nope/events"] {
        let unknown = ui.api("GET", target, &[], "");
        assert_eq!(unknown.status, 404, "{target}");
    }
}

#[test]
fn the_document_describes_the_agent_and_run_routes() {
    let dir = tempfile::tempdir().unwrap();
    let ui = ui(dir.path());
    let document = ui.api("GET", "/openapi.json", &[], "").json();
    let paths = &document["paths"];
    for (path, method) in [
        ("/api/agents", "get"),
        ("/api/agents", "post"),
        ("/api/agents/{name}", "get"),
        ("/api/agents/{name}", "delete"),
        ("/api/agents/{name}/enhance", "post"),
        ("/api/runs", "get"),
        ("/api/runs", "post"),
        ("/api/runs/{id}", "get"),
        ("/api/runs/{id}/events", "get"),
    ] {
        assert!(paths[path][method].is_object(), "{method} {path}");
    }
    let schemas = &document["components"]["schemas"];
    for name in [
        "Agent",
        "NewAgent",
        "AgentList",
        "AgentFound",
        "Enhanced",
        "RunAsk",
        "Run",
        "RunList",
        "RunFound",
        "RunEvent",
        "Problem",
    ] {
        assert!(schemas[name].is_object(), "{name}");
    }
}
