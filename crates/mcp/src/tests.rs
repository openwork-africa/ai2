use crate::reviews::{Child, ReviewAsk, ReviewFound, ReviewList, RunReview};
use crate::{Pieces, Server};
use rmcp::handler::server::common::{schema_for_empty_input, schema_for_input};
use rmcp::handler::server::tool::schema_for_output;
use rmcp::model::{CallToolRequestParams, CallToolResult, JsonObject};
use rmcp::service::RunningService;
use rmcp::{RoleClient, ServiceExt};
use schemars::JsonSchema;
use serde_json::{json, Value};
use std::any::Any;
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
#[cfg(feature = "vault")]
use std::sync::Mutex;

type Schemas = (Arc<JsonObject>, Arc<JsonObject>);

fn takes<I: JsonSchema + Any, O: JsonSchema + Any>() -> Schemas {
    (schema_for_input::<I>().unwrap(), schema_for_output::<O>())
}

fn bare<O: JsonSchema + Any>() -> Schemas {
    (schema_for_empty_input(), schema_for_output::<O>())
}

fn shared() -> BTreeMap<&'static str, Schemas> {
    let mut all = BTreeMap::new();
    all.extend([
        ("reviews_run", takes::<RunReview, ReviewFound>()),
        ("reviews_list", bare::<ReviewList>()),
        ("reviews_get", takes::<ReviewAsk, ReviewFound>()),
    ]);
    #[cfg(feature = "harness")]
    {
        use crate::harness::{CommandAsk, HarnessId, HarnessList, JobReport};
        all.extend([
            ("harness_list", bare::<HarnessList>()),
            ("harness_install", takes::<HarnessId, JobReport>()),
            ("harness_sign_in", takes::<HarnessId, JobReport>()),
            (
                "harness_command",
                takes::<CommandAsk, ::harness::cli::CommandLine>(),
            ),
        ]);
    }
    #[cfg(feature = "vault")]
    {
        use crate::vault::*;
        all.extend([
            ("vault_status", bare::<VaultStatus>()),
            ("vault_list_keys", bare::<Keys>()),
            ("vault_has_key", takes::<Name, Has>()),
            ("vault_put_key", takes::<PutKey, KeyFound>()),
            ("vault_rotate_key", takes::<RotateKey, KeyFound>()),
            ("vault_delete_key", takes::<Name, Gone>()),
            ("vault_list_tokens", bare::<Tokens>()),
            ("vault_create_token", takes::<CreateToken, TokenMade>()),
            ("vault_revoke_token", takes::<TokenId, Gone>()),
            ("vault_list_sandboxes", bare::<Sandboxes>()),
            ("vault_add_sandbox", takes::<AddSandbox, SandboxAdded>()),
            ("vault_remove_sandbox", takes::<Name, Gone>()),
            ("vault_list_approvals", bare::<Approvals>()),
            ("vault_decide_approval", takes::<Decide, Decided>()),
            ("vault_audit", takes::<AuditAsk, Audit>()),
        ]);
    }
    #[cfg(feature = "setup")]
    all.insert("setup_check", bare::<crate::setup::SetupReport>());
    #[cfg(feature = "agents")]
    {
        use crate::agents::*;
        all.extend([
            ("agents_list", bare::<AgentList>()),
            ("agents_create", takes::<NewAgent, AgentFound>()),
            ("agent_run", takes::<RunAsk, Finished>()),
            ("runs_list", bare::<RunList>()),
            ("runs_get", takes::<RunId, RunFound>()),
        ]);
    }
    #[cfg(feature = "apps")]
    {
        use crate::apps::*;
        all.extend([
            ("apps_register", takes::<NewApp, Registered>()),
            ("apps_list", takes::<ListAsk, AppList>()),
            ("apps_start", takes::<AppId, AppFound>()),
            ("apps_stop", takes::<AppId, AppFound>()),
            ("apps_status", takes::<AppId, AppFound>()),
            ("apps_logs", takes::<LogsAsk, Logs>()),
        ]);
    }
    all
}

#[cfg(feature = "vault")]
#[derive(Default)]
struct FakeVault {
    calls: Mutex<Vec<(String, String, Option<Value>)>>,
    replies: Mutex<BTreeMap<String, (u16, Value)>>,
}

#[cfg(feature = "vault")]
impl FakeVault {
    fn answer(&self, method: &str, path: &str, status: u16, reply: Value) {
        self.replies
            .lock()
            .unwrap()
            .insert(format!("{method} {path}"), (status, reply));
    }
}

#[cfg(feature = "vault")]
impl crate::vault::Admin for FakeVault {
    fn call(&self, method: &str, path: &str, body: Option<&Value>) -> anyhow::Result<(u16, Value)> {
        self.calls
            .lock()
            .unwrap()
            .push((method.into(), path.into(), body.cloned()));
        self.replies
            .lock()
            .unwrap()
            .get(&format!("{method} {path}"))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("the vault daemon is not running"))
    }
}

#[cfg(feature = "setup")]
struct FakeSetup;

#[cfg(feature = "setup")]
impl crate::setup::Setup for FakeSetup {
    fn check(&self) -> anyhow::Result<Value> {
        Ok(json!({
            "ok": false,
            "check": true,
            "steps": {
                "containers": { "status": "ready", "runtime": "container" },
                "master-key": { "status": "missing", "warnings": ["no key yet"] },
                "presence": { "status": "failed", "error": "no Touch ID" },
                "daemon": { "status": "skipped" }
            }
        }))
    }
}

struct Fixture {
    home: tempfile::TempDir,
    #[cfg(feature = "vault")]
    vault: Arc<FakeVault>,
}

impl Fixture {
    fn new() -> Self {
        Self {
            home: tempfile::tempdir().unwrap(),
            #[cfg(feature = "vault")]
            vault: Arc::new(FakeVault::default()),
        }
    }

    fn pieces(&self, bin: PathBuf) -> Pieces {
        let home = self.home.path().to_path_buf();
        Pieces {
            name: "ai2",
            version: "0.0.0-test",
            clients: crate::clients::Clients::at(&home),
            reviews: Arc::new(Child::new(bin, home.clone())),
            #[cfg(feature = "harness")]
            harness: Arc::new(crate::harness::local(registry, home.join("state"))),
            #[cfg(feature = "vault")]
            vault: self.vault.clone(),
            #[cfg(feature = "setup")]
            setup: Arc::new(FakeSetup),
            #[cfg(feature = "apps")]
            apps: Arc::new(crate::apps::Local::detached(&home).unwrap()),
            #[cfg(feature = "agents")]
            agents: ::agents::Agents::local(&home, fake_agents(&home))
                .with_review(Arc::new(FakeReview)),
        }
    }

    async fn client(&self) -> RunningService<RoleClient, ()> {
        self.client_with(PathBuf::from("/bin/false")).await
    }

    async fn client_with(&self, bin: PathBuf) -> RunningService<RoleClient, ()> {
        let (ours, theirs) = tokio::io::duplex(1 << 16);
        let server = Server::new(self.pieces(bin));
        tokio::spawn(async move {
            let (read, write) = tokio::io::split(theirs);
            let _ = crate::serve(server, read, write).await;
        });
        ().serve(ours).await.unwrap()
    }
}

#[cfg(feature = "agents")]
fn fake_agents(home: &Path) -> impl Fn() -> ::harness::Registry + Send + Sync + 'static {
    let script = home.join("fake-agent");
    std::fs::write(
        &script,
        "#!/bin/sh\necho \"working on: $1\"\necho \"$1\" > edited.txt\nexit 3\n",
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let command = script.display().to_string();
    move || {
        let mut registry = ::harness::Registry::default();
        registry.put(
            ::harness::Setup {
                id: "fake".into(),
                label: "Fake".into(),
                command: command.clone(),
                install: "true".into(),
                agent: Some(::harness::Agent::default()),
                ..::harness::Setup::default()
            },
            None,
        );
        registry
    }
}

#[cfg(feature = "agents")]
struct FakeReview;

#[cfg(feature = "agents")]
impl ::agents::Review for FakeReview {
    fn review(
        &self,
        folder: &Path,
        objective: &str,
        say: &mut dyn FnMut(String),
    ) -> anyhow::Result<::agents::Reviewed> {
        say("ai2: rating the Run Risk Factor".into());
        Ok(::agents::Reviewed {
            hash: "fedcba9876543210".into(),
            review: json!({
                "objective": objective,
                "edited": folder.join("edited.txt").is_file()
            }),
        })
    }
}

#[cfg(feature = "harness")]
fn registry() -> ::harness::Registry {
    let mut registry = ::harness::Registry::default();
    registry.put(
        ::harness::Setup {
            id: "echo".into(),
            label: "Echo".into(),
            command: "echo-agent-that-is-not-installed".into(),
            install: "printf 'Visit https://example.com/device\\nEnter WXYZ-9876\\n'".into(),
            agent: Some(::harness::Agent::default()),
            ..::harness::Setup::default()
        },
        None,
    );
    registry
}

async fn call(
    client: &RunningService<RoleClient, ()>,
    name: &'static str,
    arguments: Value,
) -> CallToolResult {
    let mut params = CallToolRequestParams::new(name);
    if let Value::Object(arguments) = arguments {
        params = params.with_arguments(arguments);
    }
    client.call_tool(params).await.unwrap()
}

fn structured(result: &CallToolResult) -> Value {
    assert_ne!(result.is_error, Some(true), "{result:?}");
    result
        .structured_content
        .clone()
        .expect("structured content")
}

fn failure(result: &CallToolResult) -> Value {
    assert_eq!(result.is_error, Some(true), "{result:?}");
    let text = serde_json::to_value(&result.content[0]).unwrap()["text"]
        .as_str()
        .unwrap()
        .to_string();
    serde_json::from_str(&text).unwrap()
}

#[cfg(feature = "vault")]
fn property_names(value: &Value, into: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            if let Some(Value::Object(properties)) = object.get("properties") {
                into.extend(properties.keys().cloned());
            }
            object
                .values()
                .for_each(|value| property_names(value, into));
        }
        Value::Array(items) => items.iter().for_each(|value| property_names(value, into)),
        _ => {}
    }
}

#[test]
fn the_tool_list_is_one_router_per_piece_named_after_its_crate() {
    let fixture = Fixture::new();
    let server = Server::new(fixture.pieces(PathBuf::from("/bin/false")));
    let names: Vec<String> = server
        .tool_list()
        .iter()
        .map(|tool| tool.name.to_string())
        .collect();
    let expected: Vec<String> = shared().keys().map(|name| name.to_string()).collect();
    assert_eq!(names, expected);
    let mut pieces: Vec<&str> = names
        .iter()
        .map(|name| match name.split('_').next().unwrap() {
            "agent" | "runs" => "agents",
            piece => piece,
        })
        .collect();
    pieces.sort();
    pieces.dedup();
    let mut features = crate::features();
    features.sort();
    assert_eq!(pieces, features);
    for tool in server.tool_list() {
        assert!(
            tool.description
                .as_deref()
                .is_some_and(|text| !text.is_empty()),
            "{}",
            tool.name
        );
    }
}

#[test]
fn every_tool_schema_is_derived_from_its_shared_serde_types() {
    let fixture = Fixture::new();
    let server = Server::new(fixture.pieces(PathBuf::from("/bin/false")));
    let shared = shared();
    for tool in server.tool_list() {
        let (input, output) = &shared[tool.name.as_ref()];
        assert_eq!(&tool.input_schema, input, "{} input", tool.name);
        assert_eq!(
            tool.output_schema.as_ref(),
            Some(output),
            "{} output",
            tool.name
        );
    }
}

#[test]
fn the_openapi_document_has_one_operation_per_tool_with_the_tools_own_schemas() {
    let fixture = Fixture::new();
    let server = Server::new(fixture.pieces(PathBuf::from("/bin/false")));
    let document = server.openapi("http://127.0.0.1:7790/mcp");
    assert_eq!(document["openapi"], "3.1.0");
    assert_eq!(document["servers"][0]["url"], "http://127.0.0.1:7790/mcp");
    let paths = document["paths"].as_object().unwrap();
    let tools = server.tool_list();
    assert_eq!(paths.len(), tools.len());
    let components = document["components"]["schemas"].as_object().unwrap();
    let text = document.to_string();
    assert!(!text.contains("#/$defs/") && !text.contains("\"$defs\""));
    for reference in text.split("\"#/components/schemas/").skip(1) {
        let name = reference.split('"').next().unwrap();
        assert!(components.contains_key(name), "{name} is not in components");
    }
    for tool in &tools {
        let operation = &paths[&format!("/tools/{}", tool.name)]["post"];
        assert_eq!(operation["operationId"], tool.name.as_ref());
        assert_eq!(operation["x-mcp"]["tool"], tool.name.as_ref());
        let request = &operation["requestBody"]["content"]["application/json"]["schema"];
        let mut expected = Value::Object((*tool.input_schema).clone());
        expected.as_object_mut().unwrap().remove("$defs");
        expected.as_object_mut().unwrap().remove("$schema");
        let rewritten: Value = serde_json::from_str(
            &expected
                .to_string()
                .replace("#/$defs/", "#/components/schemas/"),
        )
        .unwrap();
        assert_eq!(request, &rewritten, "{}", tool.name);
        let response = &operation["responses"]["200"]["content"]["application/json"]["schema"];
        assert!(response.is_object(), "{} has a 200 schema", tool.name);
    }
}

#[tokio::test]
async fn the_stdio_transport_answers_initialize_lists_tools_and_calls_a_tool() {
    let fixture = Fixture::new();
    let client = fixture.client().await;
    let info = client.peer_info().unwrap();
    assert_eq!(info.server_info.as_ref().unwrap().name, "ai2");
    assert!(info
        .instructions
        .as_deref()
        .is_some_and(|text| text.contains("reviews_")));
    let listed = client.list_all_tools().await.unwrap();
    assert_eq!(listed.len(), shared().len());
    #[cfg(feature = "harness")]
    {
        let result = call(&client, "harness_list", json!({})).await;
        let harnesses = structured(&result)["harnesses"].clone();
        assert_eq!(harnesses[0]["id"], "echo");
        assert_eq!(harnesses[0]["kind"], "agent");
        assert_eq!(harnesses[0]["installed"], false);
    }
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn a_stdio_client_is_recorded_by_its_name_and_version_on_initialize() {
    let fixture = Fixture::new();
    let clients = crate::clients::Clients::at(fixture.home.path());
    assert!(clients.clients().unwrap().is_empty());
    let (ours, theirs) = tokio::io::duplex(1 << 16);
    let server = Server::new(fixture.pieces(PathBuf::from("/bin/false")));
    tokio::spawn(async move {
        let (read, write) = tokio::io::split(theirs);
        let _ = crate::serve(server, read, write).await;
    });
    let info = rmcp::model::ClientConfig::new(
        rmcp::model::ClientCapabilities::default(),
        rmcp::model::Implementation::new("claude-code", "2.1.0"),
    );
    let client = info.serve(ours).await.unwrap();
    let listed = client.list_all_tools().await.unwrap();
    assert!(!listed.is_empty());
    let seen = clients.clients().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].name, "claude-code");
    assert_eq!(seen[0].version, "2.1.0");
    assert_eq!(seen[0].transport, "stdio");
    assert_eq!(seen[0].token, None);
    assert!(seen[0].first_seen > 0);
    client.cancel().await.unwrap();
}

#[cfg(feature = "harness")]
#[tokio::test]
async fn a_harness_install_returns_what_it_printed_and_marks_it_installed() {
    let fixture = Fixture::new();
    let client = fixture.client().await;
    let report = structured(&call(&client, "harness_install", json!({ "id": "echo" })).await);
    assert_eq!(report["ok"], true);
    let events = report["events"].as_array().unwrap();
    assert!(events.contains(&json!({ "type": "link", "url": "https://example.com/device" })));
    assert!(events.contains(&json!({ "type": "code", "code": "WXYZ-9876" })));
    let listed = structured(&call(&client, "harness_list", json!({})).await);
    assert_eq!(listed["harnesses"][0]["installed"], true);
    let line = structured(
        &call(
            &client,
            "harness_command",
            json!({ "id": "echo", "prompt": "hi", "folder": "/w" }),
        )
        .await,
    );
    assert_eq!(
        line["line"],
        "cd '/w' && exec echo-agent-that-is-not-installed 'hi'"
    );
    let missing = failure(&call(&client, "harness_install", json!({ "id": "nope" })).await);
    assert_eq!(missing["message"], "There is no harness called nope.");
}

#[cfg(feature = "vault")]
#[test]
fn no_vault_tool_output_schema_has_a_secret_field_or_any_tool_a_passphrase() {
    let fixture = Fixture::new();
    let server = Server::new(fixture.pieces(PathBuf::from("/bin/false")));
    let forbidden = [
        "secret",
        "token",
        "revoke",
        "passphrase",
        "password",
        "nonce",
    ];
    for tool in server.tool_list() {
        let mut inputs = Vec::new();
        property_names(&Value::Object((*tool.input_schema).clone()), &mut inputs);
        assert!(
            !inputs.iter().any(|name| name == "passphrase"),
            "{}",
            tool.name
        );
        if !tool.name.starts_with("vault_") {
            continue;
        }
        let mut outputs = Vec::new();
        property_names(
            &Value::Object((*tool.output_schema.clone().unwrap()).clone()),
            &mut outputs,
        );
        for name in &outputs {
            if tool.name == "vault_create_token" && name == "token" {
                continue;
            }
            assert!(
                !forbidden.contains(&name.as_str()),
                "{} outputs {name}",
                tool.name
            );
        }
    }
}

#[cfg(feature = "vault")]
#[tokio::test]
async fn vault_tools_go_through_the_admin_socket_and_never_hand_back_a_secret() {
    let fixture = Fixture::new();
    let vault = fixture.vault.clone();
    vault.answer(
        "GET",
        "/keys",
        200,
        json!({ "keys": [ { "name": "gh", "description": "GitHub", "approved_urls": ["https://api.github.com/"], "inject": "bearer", "created": "2026-11-03T00:00:00Z", "rotated": null, "secret": "ghp_leak" } ] }),
    );
    vault.answer(
        "POST",
        "/tokens",
        200,
        json!({ "id": "tok_1", "keys": ["gh"], "sandbox": "host", "mode": "agent", "methods": ["GET"], "url_prefixes": [], "intent": "read issues", "created": "c", "expires": "e", "uses_left": null, "token": "vt_made", "revoke": "rv_leak", "warnings": ["Not safe"] }),
    );
    vault.answer(
        "PUT",
        "/keys/gh",
        401,
        json!({ "error": "presence_required", "message": "Store the vault key gh: needs you present" }),
    );
    vault.answer(
        "GET",
        "/keys/a%2Fb",
        404,
        json!({ "error": "not_found", "message": "there is no vault key a/b" }),
    );
    vault.answer(
        "GET",
        "/audit?tail=5",
        200,
        json!({ "entries": [ { "time": "t", "token": "tok_1", "sandbox": "host", "key": "gh", "method": "GET", "url": "https://api.github.com/", "request": "GET /", "decision": "allow", "status": 200, "reason": null } ] }),
    );
    let client = fixture.client().await;

    let keys = call(&client, "vault_list_keys", json!({})).await;
    assert_eq!(structured(&keys)["keys"][0]["name"], "gh");
    let made = call(
        &client,
        "vault_create_token",
        json!({ "keys": ["gh"], "sandbox": "host", "mode": "agent", "intent": "read issues" }),
    )
    .await;
    let made_json = structured(&made);
    assert_eq!(made_json["id"], "tok_1");
    assert_eq!(made_json["warnings"][0], "Not safe");
    assert_eq!(made_json["withheld"], crate::vault::WITHHELD);
    assert_eq!(made_json["token"], "vt_made");
    assert!(made_json.get("revoke").is_none());
    let audit = structured(&call(&client, "vault_audit", json!({ "tail": 5 })).await);
    assert_eq!(audit["entries"][0]["token_id"], "tok_1");
    for result in [&keys, &made] {
        let text = serde_json::to_string(result).unwrap();
        assert!(!text.contains("leak"), "{text}");
    }

    let put = call(
        &client,
        "vault_put_key",
        json!({ "name": "gh", "secret": "s3cret", "approved_urls": ["https://api.github.com/"] }),
    )
    .await;
    let refused = failure(&put);
    assert_eq!(refused["error"], "presence_required");
    assert!(refused["message"]
        .as_str()
        .unwrap()
        .ends_with(crate::vault::PRESENCE));
    assert!(!serde_json::to_string(&put).unwrap().contains("s3cret"));

    let has = structured(&call(&client, "vault_has_key", json!({ "name": "a/b" })).await);
    assert_eq!(has, json!({ "name": "a/b", "present": false }));
    let status = structured(&call(&client, "vault_status", json!({})).await);
    assert_eq!(status["reachable"], false);
    assert_eq!(status["message"], "the vault daemon is not running");

    let calls = vault.calls.lock().unwrap().clone();
    let (_, _, body) = calls
        .iter()
        .find(|(method, path, _)| method == "PUT" && path == "/keys/gh")
        .unwrap();
    assert_eq!(
        body.as_ref().unwrap()["approved_urls"][0],
        "https://api.github.com/"
    );
    let (_, _, body) = calls
        .iter()
        .find(|(method, _, _)| method == "POST")
        .unwrap();
    assert!(body.as_ref().unwrap().get("methods").is_none());
    client.cancel().await.unwrap();
}

#[cfg(feature = "setup")]
#[tokio::test]
async fn setup_check_reports_each_step_in_order() {
    let fixture = Fixture::new();
    let client = fixture.client().await;
    let report = structured(&call(&client, "setup_check", json!({})).await);
    assert_eq!(report["ok"], false);
    let steps = report["steps"].as_array().unwrap();
    let ids: Vec<&str> = steps
        .iter()
        .map(|step| step["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["containers", "master-key", "presence", "daemon"]);
    assert_eq!(steps[0]["info"], json!({ "runtime": "container" }));
    assert_eq!(steps[1]["warnings"], json!(["no key yet"]));
    assert_eq!(steps[2]["status"], "failed");
    assert_eq!(steps[2]["error"], "no Touch ID");
    client.cancel().await.unwrap();
}

fn reviewer(dir: &Path, folder: &Path, home: &Path) -> PathBuf {
    let review = json!({
        "version": "0.1.2",
        "folder": folder.display().to_string(),
        "objective": "fix the login",
        "generated_at": "2026-11-03T10:00:00Z",
        "ratings": [
            { "key": "msr", "name": "Modern Security Rating", "rating": { "score": 7.5 } },
            { "key": "rrf", "name": "Run Risk Factor", "rating": null, "missing": "no model" }
        ]
    });
    let out = home.join("reviews").join("0123456789abcdef");
    let script = dir.join("ai2");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{args}'\necho 'ai2: reading the folder and the environment' >&2\nmkdir -p '{out}'\ncat > '{out}/review.json' <<'EOF'\n{review}\nEOF\ncat '{out}/review.json'\n",
            args = dir.join("args").display(),
            out = out.display(),
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    script
}

#[tokio::test]
async fn a_review_runs_as_a_child_and_is_read_back_by_hash_or_folder() {
    let fixture = Fixture::new();
    let bin = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let folder_path = folder.path().canonicalize().unwrap();
    let script = reviewer(bin.path(), &folder_path, fixture.home.path());
    let client = fixture.client_with(script).await;

    let ran = structured(
        &call(
            &client,
            "reviews_run",
            json!({ "folder": folder_path, "objective": "fix the login", "changed": ["a.rs", "b.rs"], "budget": 5 }),
        )
        .await,
    );
    assert_eq!(ran["summary"]["hash"], "0123456789abcdef");
    assert_eq!(ran["summary"]["ratings"][0]["score"], 7.5);
    assert_eq!(ran["summary"]["ratings"][1]["missing"], "no model");
    let args = std::fs::read_to_string(bin.path().join("args")).unwrap();
    assert_eq!(
        args.lines().collect::<Vec<_>>(),
        [
            "--run-path",
            folder_path.to_str().unwrap(),
            "--json",
            "--objective",
            "fix the login",
            "--changed",
            "a.rs,b.rs",
            "--budget",
            "5"
        ]
    );

    let listed = structured(&call(&client, "reviews_list", json!({})).await);
    assert_eq!(listed["reviews"].as_array().unwrap().len(), 1);
    let by_hash = structured(
        &call(
            &client,
            "reviews_get",
            json!({ "hash": "0123456789abcdef" }),
        )
        .await,
    );
    assert_eq!(by_hash["review"]["objective"], "fix the login");
    let by_folder =
        structured(&call(&client, "reviews_get", json!({ "folder": folder.path() })).await);
    assert_eq!(by_folder["summary"]["hash"], "0123456789abcdef");
    let none = failure(&call(&client, "reviews_get", json!({ "hash": "nope" })).await);
    assert_eq!(none["error"], "not_found");
    let asked = failure(&call(&client, "reviews_get", json!({})).await);
    assert_eq!(asked["error"], "bad_request");
    client.cancel().await.unwrap();
}

#[cfg(feature = "apps")]
#[tokio::test]
async fn apps_tools_register_list_and_show_from_the_apps_store() {
    use base64::Engine as _;
    let fixture = Fixture::new();
    let client = fixture.client().await;
    let refused = failure(
        &call(
            &client,
            "apps_register",
            json!({ "name": "Workspot", "description": "Desks", "command": "deno task start", "openapi": " " }),
        )
        .await,
    );
    assert_eq!(refused["error"], "openapi_required");
    let empty = structured(&call(&client, "apps_list", json!({ "include_openapi": true })).await);
    assert_eq!(empty["apps"], json!([]));
    let icon = base64::engine::general_purpose::STANDARD.encode(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR");
    let registered = structured(
        &call(
            &client,
            "apps_register",
            json!({
                "name": "Workspot",
                "description": "Desks",
                "command": "deno task start",
                "openapi": "/openapi.json",
                "port": 1,
                "cwd": fixture.home.path(),
                "icon": { "type": "image/png", "base64": icon }
            }),
        )
        .await,
    );
    assert_eq!(registered["app"]["id"], "workspot");
    assert_eq!(registered["app"]["running"], false);
    assert_eq!(registered["app"]["api"]["state"], "unchecked");
    assert!(registered["warning"]
        .as_str()
        .unwrap()
        .contains("not running yet"));
    let listed = structured(&call(&client, "apps_list", json!({})).await);
    assert_eq!(listed["apps"][0]["name"], "Workspot");
    let status = structured(&call(&client, "apps_status", json!({ "id": "workspot" })).await);
    assert_eq!(status["app"]["port"], 1);
    let logs = structured(&call(&client, "apps_logs", json!({ "id": "workspot" })).await);
    assert_eq!(logs["lines"], json!([]));
    let missing = failure(&call(&client, "apps_status", json!({ "id": "nope" })).await);
    assert_eq!(missing["error"], "not_found");
    client.cancel().await.unwrap();
}

#[cfg(feature = "agents")]
#[tokio::test]
async fn an_agent_is_kept_run_in_a_folder_and_its_run_is_read_back_with_its_review() {
    let fixture = Fixture::new();
    let folder = tempfile::tempdir().unwrap();
    let real = folder.path().canonicalize().unwrap();
    let client = fixture.client().await;
    let made = structured(
        &call(
            &client,
            "agents_create",
            json!({ "name": "scout", "harness": "fake", "folder": folder.path() }),
        )
        .await,
    );
    assert_eq!(made["agent"]["folder"], real.display().to_string());
    let listed = structured(&call(&client, "agents_list", json!({})).await);
    assert_eq!(listed["agents"][0]["name"], "scout");
    let refused = failure(
        &call(
            &client,
            "agents_create",
            json!({ "name": "scout", "harness": "nope" }),
        )
        .await,
    );
    assert_eq!(refused["error"], "unknown_harness");

    let finished = structured(
        &call(
            &client,
            "agent_run",
            json!({ "agent": "scout", "prompt": "fix the login" }),
        )
        .await,
    );
    let run = &finished["run"];
    assert_eq!(run["agent"], "scout");
    assert_eq!(run["exit"], 3);
    assert_eq!(run["state"], "ended");
    assert_eq!(run["review"], "fedcba9876543210");
    assert_eq!(
        finished["review"],
        json!({ "objective": "fix the login", "edited": true })
    );
    assert_eq!(
        std::fs::read_to_string(real.join("edited.txt")).unwrap(),
        "fix the login\n"
    );
    let log = std::fs::read_to_string(run["log"].as_str().unwrap()).unwrap();
    assert_eq!(log, "working on: fix the login\n");

    let runs = structured(&call(&client, "runs_list", json!({})).await);
    assert_eq!(runs["runs"].as_array().unwrap().len(), 1);
    let id = run["id"].as_str().unwrap();
    let found = structured(&call(&client, "runs_get", json!({ "id": id })).await);
    assert_eq!(&found["run"], run);
    let unknown = failure(&call(&client, "runs_get", json!({ "id": "../x" })).await);
    assert_eq!(unknown["error"], "not_found");
    let missing = failure(
        &call(
            &client,
            "agent_run",
            json!({ "harness": "fake", "folder": real.join("nope"), "prompt": "hi" }),
        )
        .await,
    );
    assert_eq!(missing["error"], "not_a_folder");
    client.cancel().await.unwrap();
}
