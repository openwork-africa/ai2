use super::bridge::CANCELLED;
use super::reporting::SUDO_REFUSED;
use super::*;
use crate::{Options, NONCE_HEADER};
use ::setup::admin::fake::FakeAdmin;
use ::setup::admin::HOST_SANDBOX;
use ::setup::runner::fake::Fake;
use ::setup::runner::{Cmd, Ran};
use ::setup::{all, run_steps, Context as SetupContext, NOT_SAFE};
use keys::MemoryKey;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::broadcast::Receiver;

const PASSPHRASE: &str = "a long passphrase";

fn linux_options() -> ::setup::Options {
    ::setup::Options {
        bin: "/home/ada/.local/bin/ai2".into(),
        home: "/home/ada/.ai2".into(),
        state: "/var/lib/ai2-vault".into(),
        label: ::setup::files::LABEL.into(),
        keychain: None,
        admin: "/run/ai2-vault/admin.sock".into(),
        dashboard: ::setup::files::Dashboard::default(),
    }
}

fn linux_fresh(fake: &Fake) {
    fake.answer("root sh -c if test -s", Ran::ok("yes\n"))
        .once("root sh -c if test -s", Ran::ok("no\n"))
        .answer("stat -c", Ran::ok("600 root\n"))
        .once("stat -c", Ran::failed())
        .answer("getent passwd ai2-vault", Ran::ok("ai2-vault:x:990"))
        .once("getent passwd ai2-vault", Ran::failed())
        .once("getent group ai2-vault-admin", Ran::failed())
        .answer("id -nG ada", Ran::ok("ada sudo ai2-vault-admin\n"))
        .once("id -nG ada", Ran::ok("ada sudo\n"))
        .answer("cmp -s", Ran::ok(""))
        .once("cmp -s", Ran::failed())
        .answer("systemctl is-active", Ran::ok(""))
        .once("systemctl is-active", Ran::failed());
}

fn needs_sudo(fake: &Fake) {
    linux_fresh(fake);
    fake.once("sudo -n -v", Ran::failed())
        .once("sudo -S -v", Ran::failed());
}

fn engine(host: Host, prepare: fn(&Fake)) -> Engine {
    let seen = host.clone();
    Engine {
        host: Arc::new(move || seen.clone()),
        runner: Arc::new(move || {
            let fake = Fake::default();
            prepare(&fake);
            Box::new(fake)
        }),
        live: false,
        drive: Arc::new(move |runner, check| {
            let master = MemoryKey::default();
            let admin = FakeAdmin::with(&[HOST_SANDBOX]);
            let presence = || -> Result<bool> { Ok(false) };
            let options = linux_options();
            let ctx = SetupContext {
                host: &host,
                options: &options,
                runner,
                master: &master,
                admin: &admin,
                presence: &presence,
                check_only: check,
            };
            Ok(run_steps(&all(), &ctx))
        }),
    }
}

fn next(receiver: &mut Receiver<Event>) -> Event {
    receiver.blocking_recv().expect("an event")
}

fn drive(
    bridge: &Arc<Bridge>,
    engine: Engine,
    mut answer: impl FnMut(&Input) -> Answer,
) -> Vec<Event> {
    let (_, mut receiver) = bridge.subscribe(None);
    assert!(start(bridge, Arc::new(engine), false));
    let mut seen = Vec::new();
    loop {
        let event = next(&mut receiver);
        seen.push(event.clone());
        match event.kind {
            "input" => {
                let input: Input = serde_json::from_value(event.data.clone()).unwrap();
                assert!(bridge.answer(&input.id, answer(&input)));
            }
            "done" => return seen,
            _ => {}
        }
    }
}

fn steps(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter(|event| event.kind == "step")
        .map(|event| {
            format!(
                "{} {}",
                event.data["id"].as_str().unwrap(),
                event.data["status"].as_str().unwrap()
            )
        })
        .collect()
}

fn kinds(events: &[Event]) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = events
        .iter()
        .map(|event| event.kind)
        .filter(|kind| *kind != "log")
        .collect();
    out.dedup();
    out
}

fn same_twice(_: &Input) -> Answer {
    Answer::Values(vec![
        PASSPHRASE.to_string().into(),
        PASSPHRASE.to_string().into(),
    ])
}

fn fresh_bridge() -> Arc<Bridge> {
    Arc::new(Bridge::new(&step_ids()))
}

#[test]
fn a_passing_run_reports_every_step_and_asks_for_the_passphrase_once() {
    let bridge = fresh_bridge();
    let events = drive(
        &bridge,
        engine(Host::linux(Some(255)), linux_fresh),
        same_twice,
    );
    assert_eq!(
        steps(&events),
        [
            "containers running",
            "containers ready",
            "master-key running",
            "master-key ready",
            "presence running",
            "presence ready",
            "daemon running",
            "daemon ready",
            "dashboard running",
            "dashboard ready",
        ]
    );
    assert_eq!(
        kinds(&events),
        ["snapshot", "step", "input", "answered", "step", "done"]
    );
    let input = events.iter().find(|event| event.kind == "input").unwrap();
    assert_eq!(input.data["kind"], "passphrase");
    assert_eq!(input.data["step"], "presence");
    assert_eq!(input.data["fields"].as_array().unwrap().len(), 2);
    let done = events.last().unwrap();
    assert_eq!(done.data["ok"], true);
    assert_eq!(done.data["report"]["steps"]["daemon"]["status"], "ready");
    assert!(done.data["warnings"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Log out and back in"));
    let ids: Vec<u64> = events.iter().map(|event| event.id).collect();
    assert!(ids.windows(2).all(|pair| pair[1] == pair[0] + 1));
    let finished = events
        .iter()
        .filter(|event| event.kind == "step" && event.data["status"] == "ready");
    assert!(finished.clone().all(|event| event.data["ms"].is_u64()));
    assert!(events
        .iter()
        .all(|event| !event.data.to_string().contains(PASSPHRASE)));
    let snapshot = bridge.snapshot();
    assert_eq!(snapshot.phase, Phase::Done);
    assert_eq!(snapshot.ok, Some(true));
    assert!(snapshot.input.is_none());
    assert!(snapshot.steps.iter().any(|step| !step.log.is_empty()));
}

#[test]
fn a_failing_step_skips_the_rest_and_carries_its_error_and_warnings() {
    let bridge = fresh_bridge();
    let events = drive(&bridge, engine(Host::linux(Some(249)), |_| {}), same_twice);
    assert_eq!(
        steps(&events),
        [
            "containers running",
            "containers unsupported",
            "master-key running",
            "master-key failed",
            "presence skipped",
            "daemon skipped",
            "dashboard skipped",
        ]
    );
    let failed = events
        .iter()
        .find(|event| event.kind == "step" && event.data["status"] == "failed")
        .unwrap();
    assert!(failed.data["error"]
        .as_str()
        .unwrap()
        .contains("systemd 250 or later"));
    assert!(failed.data["info"].get("error").is_none());
    let skipped = events
        .iter()
        .find(|event| event.kind == "step" && event.data["status"] == "skipped")
        .unwrap();
    assert!(skipped.data.get("ms").is_none());
    let done = events.last().unwrap();
    assert_eq!(done.data["ok"], false);
    assert_eq!(done.data["warnings"][0]["step"], "containers");
    assert_eq!(done.data["warnings"][0]["text"], NOT_SAFE);
}

#[test]
fn passphrases_that_differ_fail_the_step() {
    let bridge = fresh_bridge();
    let events = drive(&bridge, engine(Host::linux(Some(255)), linux_fresh), |_| {
        Answer::Values(vec!["one".to_string().into(), "two".to_string().into()])
    });
    let presence = bridge.snapshot().steps[2].clone();
    assert_eq!(presence.status, "failed");
    assert_eq!(
        presence.error.as_deref(),
        Some("the two passphrases differ")
    );
    assert_eq!(events.last().unwrap().data["ok"], false);
}

#[test]
fn cancelling_in_the_browser_fails_the_step_and_skips_the_rest() {
    let bridge = fresh_bridge();
    let events = drive(&bridge, engine(Host::linux(Some(255)), linux_fresh), |_| {
        Answer::Cancel
    });
    let answered = events
        .iter()
        .find(|event| event.kind == "answered")
        .unwrap();
    assert_eq!(answered.data["cancelled"], true);
    let snapshot = bridge.snapshot();
    assert_eq!(snapshot.steps[2].status, "failed");
    assert_eq!(snapshot.steps[2].error.as_deref(), Some(CANCELLED));
    assert_eq!(snapshot.steps[3].status, "skipped");
}

#[test]
fn sudo_is_asked_for_once_in_the_page_and_again_when_refused() {
    let bridge = fresh_bridge();
    let mut asked = Vec::new();
    let events = drive(
        &bridge,
        engine(Host::linux(Some(255)), needs_sudo),
        |input| {
            asked.push((input.kind.clone(), input.error.clone()));
            match input.kind.as_str() {
                "sudo" => Answer::Values(vec!["hunter2".to_string().into()]),
                _ => same_twice(input),
            }
        },
    );
    assert_eq!(
        asked,
        [
            ("sudo".to_string(), None),
            ("sudo".to_string(), Some(SUDO_REFUSED.to_string())),
            ("passphrase".to_string(), None),
        ]
    );
    let sudo = events.iter().find(|event| event.kind == "input").unwrap();
    assert_eq!(sudo.data["step"], "master-key");
    assert_eq!(sudo.data["title"], "Your password, for sudo");
    assert_eq!(events.last().unwrap().data["ok"], true);
    assert!(events
        .iter()
        .all(|event| !event.data.to_string().contains("hunter2")));
}

#[test]
fn a_check_never_asks_anything() {
    let bridge = fresh_bridge();
    let (_, mut receiver) = bridge.subscribe(None);
    assert!(start(
        &bridge,
        Arc::new(engine(Host::linux(Some(255)), needs_sudo)),
        true
    ));
    let mut kinds = Vec::new();
    loop {
        let event = next(&mut receiver);
        kinds.push(event.kind);
        if event.kind == "done" {
            assert_eq!(event.data["check"], true);
            break;
        }
    }
    assert!(!kinds.contains(&"input"));
    assert_eq!(bridge.snapshot().steps[1].status, "missing");
}

#[test]
fn a_second_start_while_running_is_refused() {
    let bridge = fresh_bridge();
    let (_, mut receiver) = bridge.subscribe(None);
    let make = || Arc::new(engine(Host::linux(Some(255)), linux_fresh));
    assert!(start(&bridge, make(), false));
    loop {
        if next(&mut receiver).kind == "input" {
            break;
        }
    }
    assert!(!start(&bridge, make(), false));
    let id = bridge.snapshot().input.unwrap().id;
    assert!(!bridge.answer("p999", Answer::Cancel));
    assert!(bridge.answer(&id, Answer::Cancel));
    while next(&mut receiver).kind != "done" {}
    assert!(start(&bridge, make(), true));
}

#[test]
fn a_reconnect_gets_what_it_missed_and_a_new_page_gets_a_snapshot() {
    let bridge = fresh_bridge();
    let events = drive(
        &bridge,
        engine(Host::linux(Some(255)), linux_fresh),
        same_twice,
    );
    let last = events.last().unwrap().id;
    let (replay, _) = bridge.subscribe(Some(3));
    assert_eq!(replay.first().unwrap().id, 4);
    assert_eq!(replay.last().unwrap().id, last);
    assert_eq!(replay, events[3..]);
    let (fresh, _) = bridge.subscribe(None);
    assert_eq!(fresh.len(), 1);
    assert_eq!(fresh[0].kind, "snapshot");
    assert_eq!(fresh[0].id, last);
    assert_eq!(fresh[0].data["phase"], "done");
    let (nothing, _) = bridge.subscribe(Some(last));
    assert!(nothing.is_empty());
    let (ahead, _) = bridge.subscribe(Some(last + 10));
    assert_eq!(ahead[0].kind, "snapshot");
}

#[test]
fn shown_commands_stream_each_line_into_the_step_log() {
    let bridge = fresh_bridge();
    let reporting = Reporting::new(Box::new(Fake::default()), bridge.clone(), true);
    reporting.step_started("containers");
    let ran = reporting
        .run(&Cmd::new("sh", &["-c", "echo one; echo two >&2; printf 'a\\rb\\n'"]).shown())
        .unwrap();
    assert!(ran.ok);
    assert_eq!(ran.out, "one\nb\n");
    let log = bridge.snapshot().steps[0].log.clone();
    let lines: Vec<(&str, &str)> = log
        .iter()
        .map(|line| (line.stream, line.line.as_str()))
        .collect();
    assert_eq!(
        lines[0],
        ("cmd", "$ sh -c echo one; echo two >&2; printf 'a\\rb\\n'")
    );
    assert!(lines.contains(&("out", "one")));
    assert!(lines.contains(&("err", "two")));
    assert!(lines.contains(&("out", "b")));
    let missing = reporting
        .run(&Cmd::new("ai2-no-such-program", &[]).shown())
        .unwrap();
    assert!(!missing.ok);
}

struct Server {
    host: String,
    nonce: String,
    state: crate::Shared,
    runtime: Option<tokio::runtime::Runtime>,
    _dir: tempfile::TempDir,
}

impl Server {
    fn start(engine: Engine) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let options = Options {
            home: dir.path().to_path_buf(),
            admin: PathBuf::from(dir.path()).join("admin.sock"),
            agents: ::agents::Agents::local(dir.path(), ::harness::Registry::public),
        };
        let server = runtime.block_on(crate::Server::bind(0, &options)).unwrap();
        let state = server.state();
        arm(&state, engine, false);
        runtime.spawn(server.serve());
        Self {
            host: state.host.clone(),
            nonce: state.nonce.clone(),
            state: state.clone(),
            runtime: Some(runtime),
            _dir: dir,
        }
    }

    fn head(&self, method: &str, target: &str, extra: &str, body: &str) -> String {
        format!(
            "{method} {target} HTTP/1.1\r\nHost: {}\r\n{NONCE_HEADER}: {}\r\nOrigin: http://{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{extra}\r\n{body}",
            self.host,
            self.nonce,
            self.host,
            body.len()
        )
    }

    fn call(&self, method: &str, target: &str, body: &str) -> (u16, String) {
        let mut stream = TcpStream::connect(&self.host).unwrap();
        stream
            .write_all(
                self.head(method, target, "Connection: close\r\n", body)
                    .as_bytes(),
            )
            .unwrap();
        let mut raw = String::new();
        stream.read_to_string(&mut raw).unwrap();
        let status = raw
            .split(' ')
            .nth(1)
            .and_then(|code| code.parse().ok())
            .unwrap();
        let body = raw
            .split_once("\r\n\r\n")
            .map(|(_, body)| body.to_string())
            .unwrap_or_default();
        (status, body)
    }

    fn events(&self, last: Option<u64>) -> BufReader<TcpStream> {
        let stream = TcpStream::connect(&self.host).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(20)))
            .unwrap();
        let extra = last
            .map(|id| format!("Last-Event-ID: {id}\r\n"))
            .unwrap_or_default();
        (&stream)
            .write_all(self.head("GET", "/api/setup/events", &extra, "").as_bytes())
            .unwrap();
        BufReader::new(stream)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

fn until(reader: &mut BufReader<TcpStream>, kind: &str) -> (u64, Value) {
    let mut id = 0;
    let mut event = String::new();
    loop {
        let mut line = String::new();
        assert!(reader.read_line(&mut line).unwrap() > 0, "the stream ended");
        let line = line.trim_end();
        if let Some(value) = line.strip_prefix("event: ") {
            event = value.to_string();
        } else if let Some(value) = line.strip_prefix("id: ") {
            id = value.parse().unwrap();
        } else if let Some(value) = line.strip_prefix("data: ") {
            if event == kind {
                return (id, serde_json::from_str(value).unwrap());
            }
        }
    }
}

#[test]
fn the_page_starts_setup_answers_over_post_and_resumes_with_last_event_id() {
    let server = Server::start(engine(Host::linux(Some(255)), linux_fresh));
    let (status, body) = server.call("GET", "/api/setup/steps/info", "");
    assert_eq!(status, 200);
    let info: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(info["steps"][0]["title"], "Sandboxes");
    assert_eq!(info["inputs"][0]["kind"], "passphrase");
    let (_, body) = server.call("GET", "/api/setup/host", "");
    assert!(body.contains("\"os\":\"linux\""));
    assert!(server
        .call("GET", "/api/setup", "")
        .1
        .contains("\"phase\":\"idle\""));

    let mut events = server.events(None);
    let (_, snapshot) = until(&mut events, "snapshot");
    assert_eq!(snapshot["phase"], "running");
    let (seen, input) = until(&mut events, "input");
    let id = input["id"].as_str().unwrap().to_string();
    let (status, _) = server.call("POST", "/api/setup/start", r#"{"check":false}"#);
    assert_eq!(status, 409);
    let (status, _) = server.call(
        "POST",
        "/api/setup/answer",
        r#"{"id":"p999","values":["x","x"]}"#,
    );
    assert_eq!(status, 409);
    let answer = format!(r#"{{"id":"{id}","values":["{PASSPHRASE}","{PASSPHRASE}"]}}"#);
    assert_eq!(server.call("POST", "/api/setup/answer", &answer).0, 204);
    let (last, done) = until(&mut events, "done");
    assert_eq!(done["ok"], true);

    let mut resumed = server.events(Some(seen));
    let (again, answered) = until(&mut resumed, "answered");
    assert_eq!(again, seen + 1);
    assert_eq!(answered["id"], id.as_str());
    let (end, _) = until(&mut resumed, "done");
    assert_eq!(end, last);

    let mut later = server.events(None);
    let (at, snapshot) = until(&mut later, "snapshot");
    assert_eq!(at, last);
    assert_eq!(snapshot["phase"], "done");
    assert_eq!(snapshot["ok"], true);

    let (status, body) = server.call("POST", "/api/setup/start", r#"{"check":true}"#);
    assert_eq!(status, 202);
    assert!(body.contains("checking"));
    let (_, body) = server.call("GET", "/openapi.json", "");
    for path in [
        "/api/setup",
        "/api/setup/host",
        "/api/setup/steps/info",
        "/api/setup/start",
        "/api/setup/events",
        "/api/setup/answer",
        "/api/setup/dashboard",
        "/api/setup/handover",
    ] {
        assert!(body.contains(&format!("\"{path}\"")), "{path}");
    }
}

#[test]
fn the_dashboard_is_here_elsewhere_or_not_up_yet() {
    let here = dashboard_of(7797, 7797, || panic!("no need to look"));
    assert!(here.here && here.url.is_none());
    let elsewhere = dashboard_of(50123, 7797, || {
        Some("http://127.0.0.1:7797/?nonce=abc".into())
    });
    assert!(!elsewhere.here);
    assert_eq!(
        elsewhere.url.as_deref(),
        Some("http://127.0.0.1:7797/?nonce=abc")
    );
    let missing = dashboard_of(50123, 7797, || None);
    assert!(!missing.here && missing.url.is_none());
}

#[test]
fn a_setup_only_server_stops_once_the_page_hands_over_and_the_dashboard_never_does() {
    let server = Server::start(engine(Host::linux(Some(255)), linux_fresh));
    let (status, body) = server.call("GET", "/api/setup/dashboard", "");
    assert_eq!(status, 200);
    let found: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(found["here"], false);
    assert_eq!(found["url"], Value::Null);

    let (status, body) = server.call("POST", "/api/setup/handover", "");
    assert_eq!(status, 202);
    assert_eq!(body, r#"{"stopping":false}"#);
    assert!(!*server.state.stop.borrow());

    server.state.handover.store(true, Ordering::SeqCst);
    let (_, body) = server.call("POST", "/api/setup/handover", "");
    assert_eq!(body, r#"{"stopping":true}"#);
    let (_, again) = server.call("POST", "/api/setup/handover", "");
    assert_eq!(again, r#"{"stopping":false}"#);
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !*server.state.stop.borrow() {
        assert!(
            std::time::Instant::now() < deadline,
            "the server never stopped"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn setup_opens_the_welcome_story_and_a_check_opens_setup() {
    assert_eq!(
        crate::server::address(7797, "abc", Some(first_page(false))),
        "http://127.0.0.1:7797/welcome?nonce=abc"
    );
    assert_eq!(
        crate::server::address(7797, "abc", Some(first_page(true))),
        "http://127.0.0.1:7797/setup?nonce=abc"
    );
}
