use crate::logs::{Follow, Logs, RING};
use crate::model::{ApiState, IconUpload, NewApp, Restart, Stream};
use crate::port::{detect, strip_jsonc};
use crate::process;
use crate::store::{Running, Store};
use crate::supervisor::{Settings, Supervisor};
use std::io::Write;
use std::net::TcpListener;
use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
const DOCUMENT: &str = r#"{"openapi":"3.1.0","info":{"title":"Fake","version":"0.3.0"},"paths":{"/a":{"get":{},"post":{}},"/b":{"delete":{}}}}"#;

struct Home {
    _temp: TempDir,
    store: Store,
    app_dir: std::path::PathBuf,
}

impl Drop for Home {
    fn drop(&mut self) {
        for (_, entry) in self.store.running().unwrap_or_default() {
            if process::same(entry.pid, &entry.started_at, &entry.boot) {
                process::signal_group(entry.pid, libc::SIGKILL);
            }
        }
    }
}

fn home() -> Home {
    let temp = TempDir::new().unwrap();
    let store = Store::open(&temp.path().join("apps")).unwrap();
    let app_dir = temp.path().join("app");
    std::fs::create_dir_all(&app_dir).unwrap();
    std::fs::write(app_dir.join("openapi.json"), DOCUMENT).unwrap();
    std::fs::write(app_dir.join("icon.png"), PNG).unwrap();
    Home {
        _temp: temp,
        store,
        app_dir,
    }
}

fn new_app(home: &Home, name: &str, command: &str, port: u16) -> NewApp {
    NewApp {
        name: name.to_string(),
        description: "A fake app for tests".to_string(),
        command: command.to_string(),
        openapi: "/openapi.json".to_string(),
        icon: Some(IconUpload {
            path: Some(home.app_dir.join("icon.png").to_string_lossy().into_owned()),
            ..IconUpload::default()
        }),
        cwd: Some(home.app_dir.to_string_lossy().into_owned()),
        port: Some(port),
        ..NewApp::default()
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn python() -> bool {
    let found = std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success());
    if !found {
        eprintln!("python3 is not installed; skipping");
    }
    found
}

/// A plain file server. Not `python3 -m http.server`: its `HTTPServer` looks up the host name
/// between bind and listen, and on a runner with slow DNS the port stays shut past the wait.
const SERVER: &str = "import os,socketserver,http.server as h; socketserver.TCPServer.allow_reuse_address=True; socketserver.TCPServer(('127.0.0.1',int(os.environ['PORT'])),h.SimpleHTTPRequestHandler).serve_forever()";
const SERVE: &str = "exec python3 -c \"import os,socketserver,http.server as h; socketserver.TCPServer.allow_reuse_address=True; socketserver.TCPServer(('127.0.0.1',int(os.environ['PORT'])),h.SimpleHTTPRequestHandler).serve_forever()\"";

fn fast(supervise: bool) -> Settings {
    Settings {
        supervise,
        grace: Duration::from_secs(3),
        port_wait: Duration::from_secs(15),
        port_close: Duration::from_secs(3),
        backoff: Duration::from_millis(50),
        backoff_max: Duration::from_millis(200),
        restart_window: Duration::from_secs(60),
        restart_limit: 3,
        poll: Duration::from_millis(50),
        recheck: Duration::from_secs(60),
    }
}

async fn eventually(what: &str, mut test: impl FnMut() -> bool) {
    for _ in 0..200 {
        if test() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("timed out waiting for {what}");
}

#[test]
fn register_stores_the_app_its_icon_and_a_unique_id() {
    let home = home();
    let (app, created) = home
        .store
        .register(new_app(&home, "Work spot", "deno task start", 7791))
        .unwrap();
    assert!(created);
    assert_eq!(app.id, "work-spot");
    assert_eq!(app.url, "http://localhost:7791");
    assert!(app.set_port);
    assert_eq!(app.restart, Restart::Never);
    let icon = app.icon.clone().unwrap();
    assert_eq!(icon.content_type, "image/png");
    assert_eq!(icon.file, "icons/work-spot.png");
    assert_eq!(icon.bytes, PNG.len() as u64);
    assert_eq!(
        home.store.icon("work-spot").unwrap().unwrap(),
        ("image/png".to_string(), PNG.to_vec())
    );

    let mut again = new_app(&home, "Work spot", "deno task serve", 7792);
    again.icon = None;
    let (updated, created) = home.store.register(again).unwrap();
    assert!(!created);
    assert_eq!(updated.id, "work-spot");
    assert_eq!(updated.command, "deno task serve");
    assert_eq!(updated.added, app.added);
    assert_eq!(updated.icon, app.icon);

    let (other, _) = home
        .store
        .register(new_app(&home, "work-spot", "x", 7793))
        .unwrap();
    assert_eq!(other.id, "work-spot-2");

    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(home.store.dir().join("apps.json")).unwrap())
            .unwrap();
    assert_eq!(saved.as_array().unwrap().len(), 2);

    home.store.remove("work-spot").unwrap();
    assert!(!home.store.dir().join("icons/work-spot.png").exists());
    assert_eq!(home.store.apps().unwrap().len(), 1);
    assert_eq!(home.store.app("work-spot").unwrap_err().status, 404);
}

#[test]
fn update_changes_only_what_is_sent() {
    let home = home();
    home.store
        .register(new_app(&home, "Patch", "run", 7000))
        .unwrap();
    let app = home
        .store
        .update(
            "patch",
            crate::AppPatch {
                port: Some(7001),
                restart: Some(Restart::Always),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(app.port, 7001);
    assert_eq!(app.url, "http://localhost:7001");
    assert_eq!(app.restart, Restart::Always);
    assert_eq!(app.command, "run");
    let refused = home
        .store
        .update(
            "patch",
            crate::AppPatch {
                openapi: Some(" ".into()),
                ..Default::default()
            },
        )
        .unwrap_err();
    assert_eq!(refused.error, "openapi_required");
}

#[test]
fn register_refuses_an_app_without_openapi_icon_or_description() {
    let home = home();
    let mut app = new_app(&home, "A", "run", 7000);
    app.openapi = "  ".into();
    let refused = home.store.register(app).unwrap_err();
    assert_eq!(
        (refused.status, refused.error.as_str()),
        (422, "openapi_required")
    );

    let mut app = new_app(&home, "A", "run", 7000);
    app.icon = None;
    let refused = home.store.register(app).unwrap_err();
    assert_eq!(
        (refused.status, refused.error.as_str()),
        (422, "icon_required")
    );

    let mut app = new_app(&home, "A", "run", 7000);
    app.description = String::new();
    assert_eq!(
        home.store.register(app).unwrap_err().error,
        "description_required"
    );

    let mut app = new_app(&home, "A", "run", 7000);
    app.cwd = Some("relative/dir".into());
    assert_eq!(home.store.register(app).unwrap_err().error, "cwd_required");

    let mut app = new_app(&home, "A", "run-something", 7000);
    app.port = None;
    assert_eq!(home.store.register(app).unwrap_err().error, "port_required");

    assert!(home.store.apps().unwrap().is_empty());

    let parsed = NewApp::parse(serde_json::json!({
        "name": "A", "description": "d", "command": "c"
    }))
    .unwrap_err();
    assert_eq!(
        (parsed.status, parsed.error.as_str()),
        (422, "openapi_required")
    );
    let parsed = NewApp::parse(serde_json::json!({
        "name": "A", "description": "d", "command": "c", "openapi": "/openapi.json",
        "icon": { "type": "image/png", "base64": "iVBORw0KGgo=" }
    }))
    .unwrap();
    assert_eq!(parsed.openapi, "/openapi.json");
}

#[test]
fn icons_are_sniffed_capped_and_svgs_sanitised() {
    use crate::icon::check;
    assert_eq!(check(None, PNG.to_vec()).unwrap().0, "image/png");
    assert_eq!(
        check(None, vec![0xff, 0xd8, 0xff, 0xe0]).unwrap().0,
        "image/jpeg"
    );
    assert_eq!(check(None, vec![0, 0, 1, 0, 1]).unwrap().0, "image/x-icon");
    let webp = b"RIFF\0\0\0\0WEBPVP8 ".to_vec();
    assert_eq!(check(None, webp).unwrap().0, "image/webp");
    let svg =
        br##"<svg xmlns="http://www.w3.org/2000/svg"><use href="#a"/><rect fill="red"/></svg>"##;
    assert_eq!(check(None, svg.to_vec()).unwrap().0, "image/svg+xml");

    assert_eq!(
        check(None, b"hello".to_vec()).unwrap_err().error,
        "icon_type"
    );
    assert_eq!(
        check(Some("image/jpeg"), PNG.to_vec()).unwrap_err().error,
        "icon_type"
    );
    let mut big = PNG.to_vec();
    big.resize(600 * 1024, 0);
    assert_eq!(check(None, big).unwrap_err().status, 413);
    for bad in [
        r#"<svg><script>alert(1)</script></svg>"#,
        r#"<svg onload="alert(1)"></svg>"#,
        r#"<svg><image href="https://evil.example/x.png"/></svg>"#,
        r#"<svg><style>rect{fill:url(https://evil.example/x)}</style></svg>"#,
        r#"<svg><a xlink:href="javascript:alert(1)"/></svg>"#,
    ] {
        let refused = check(None, bad.as_bytes().to_vec()).unwrap_err();
        assert_eq!(
            (refused.status, refused.error.as_str()),
            (400, "icon_unsafe"),
            "{bad}"
        );
    }
}

#[test]
fn ports_are_detected_as_the_launcher_did() {
    let temp = TempDir::new().unwrap();
    let dir = temp.path();
    assert_eq!(detect(dir, "deno run -A main.ts --port 8000"), Some(8000));
    assert_eq!(detect(dir, "deno run --port=8001 main.ts"), Some(8001));
    assert_eq!(
        detect(dir, "ZAZI_PORT=8790 cargo run --release"),
        Some(8790)
    );
    assert_eq!(detect(dir, "python3 -m http.server -p 8002"), Some(8002));
    assert_eq!(detect(dir, "npx vite"), Some(5173));
    assert_eq!(detect(dir, "npx vite build"), None);
    std::fs::write(
        dir.join("vite.config.ts"),
        "export default { server: { port: 5199 } }",
    )
    .unwrap();
    assert_eq!(detect(dir, "deno run -A npm:vite"), Some(5199));

    std::fs::write(dir.join("a.ts"), "Deno.serve({ port: 8123 }, h)").unwrap();
    std::fs::write(
        dir.join("b.ts"),
        r#"const p = Number(Deno.env.get("PORT") ?? 8124)"#,
    )
    .unwrap();
    std::fs::write(dir.join("c.js"), "app.listen( 8125 )").unwrap();
    assert_eq!(detect(dir, "deno run -A a.ts"), Some(8123));
    assert_eq!(detect(dir, "deno run -A b.ts start"), Some(8124));
    assert_eq!(detect(dir, "node c.js"), Some(8125));
    assert_eq!(detect(dir, "deno run -A missing.ts"), None);

    std::fs::write(
        dir.join("deno.jsonc"),
        r#"{
            // the tasks
            "tasks": {
                "build": "deno run --port 9999 build.ts",
                "start": "deno task build && deno task serve", /* serve it */
                "serve": { "command": "deno run -A a.ts" },
            },
        }"#,
    )
    .unwrap();
    assert_eq!(detect(dir, "deno task start"), Some(8123));
    assert_eq!(detect(dir, "deno task build"), None);
}

#[test]
fn jsonc_comments_and_trailing_commas_are_stripped() {
    let text = r#"{ "a": "x // not a comment", /* c */ "b": [1, 2,], } // end"#;
    let value: serde_json::Value = serde_json::from_str(&strip_jsonc(text)).unwrap();
    assert_eq!(
        value,
        serde_json::json!({ "a": "x // not a comment", "b": [1, 2] })
    );
}

#[test]
fn the_openapi_check_marks_valid_unreachable_and_invalid() {
    let home = home();
    let file = home.app_dir.join("openapi.json");
    let mut app = new_app(&home, "Doc", "run", free_port());
    app.openapi = file.to_string_lossy().into_owned();
    let (app, _) = home.store.register(app).unwrap();
    assert_eq!(home.store.api(&app).state, ApiState::Unchecked);
    let api = crate::openapi::check(&home.store, &app).unwrap();
    assert_eq!(api.state, ApiState::Valid);
    assert_eq!(api.title.as_deref(), Some("Fake"));
    assert_eq!(api.version.as_deref(), Some("0.3.0"));
    assert_eq!(api.operations, Some(3));
    assert_eq!(api.href, "/api/apps/doc/openapi.json");
    assert_eq!(home.store.api(&app), api);
    assert_eq!(home.store.document("doc").unwrap()["info"]["title"], "Fake");

    let (closed, _) = home
        .store
        .register(new_app(&home, "Closed", "run", free_port()))
        .unwrap();
    let api = crate::openapi::check(&home.store, &closed).unwrap();
    assert_eq!(api.state, ApiState::Unreachable);
    assert!(api.error.is_some());
    assert!(home.store.document("closed").is_none());

    std::fs::write(&file, r#"{"openapi":"2.0","paths":{}}"#).unwrap();
    let api = crate::openapi::check(&home.store, &app).unwrap();
    assert_eq!(api.state, ApiState::Invalid);
    assert!(api.error.unwrap().contains("3.x"));

    std::fs::write(&file, "openapi: 3.1.0\npaths: {}\n").unwrap();
    let api = crate::openapi::check(&home.store, &app).unwrap();
    assert_eq!(api.state, ApiState::Invalid);
    assert!(api.error.unwrap().contains("serve it as JSON"));

    std::fs::write(&file, r#"{"openapi":"3.0.3"}"#).unwrap();
    let api = crate::openapi::check(&home.store, &app).unwrap();
    assert_eq!(
        api.error.as_deref(),
        Some("the document has no paths object")
    );
}

#[test]
fn the_ring_keeps_the_last_lines_and_replays_after_an_id() {
    let logs = Logs::default();
    for n in 0..RING + 5 {
        logs.push(Stream::Out, format!("line {n}"));
    }
    let all = logs.since(None);
    assert_eq!(all.len(), RING);
    assert_eq!(all[0].1.line, "line 5");
    let after = logs.since(Some(all[RING - 3].0));
    assert_eq!(after.len(), 2);
    let mut rx = logs.subscribe();
    let id = logs.push(Stream::Err, "live".into());
    assert_eq!(id, RING as u64 + 6);
    assert_eq!(
        rx.try_recv().unwrap(),
        (id, logs.since(Some(id - 1))[0].1.clone())
    );
}

#[test]
fn follow_reads_complete_lines_appended_to_the_file() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("a.log");
    std::fs::write(&path, "old\n").unwrap();
    let mut follow = Follow::from_end(&path);
    assert!(follow.read().is_empty());
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap();
    write!(file, "one\ntw").unwrap();
    assert_eq!(follow.read(), vec!["one"]);
    writeln!(file, "o").unwrap();
    assert_eq!(follow.read(), vec!["two"]);
    std::fs::write(&path, "new\n").unwrap();
    assert_eq!(follow.read(), vec!["new"]);
}

#[test]
fn the_cli_parses_every_subcommand() {
    let command = crate::command();
    command.clone().debug_assert();
    for args in [
        vec![
            "apps",
            "register",
            "--name",
            "N",
            "--command",
            "c",
            "--openapi",
            "/o.json",
            "--env",
            "A=1",
            "--restart",
            "on-failure",
            "--no-port-env",
        ],
        vec!["apps", "list", "--openapi"],
        vec!["apps", "logs", "x", "-n", "5", "--follow"],
        vec!["apps", "restart", "x"],
    ] {
        command.clone().try_get_matches_from(args).unwrap();
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn start_checks_the_api_and_stop_ends_the_process_group() {
    if !python() {
        return;
    }
    let home = home();
    let port = free_port();
    home.store
        .register(new_app(&home, "Serve", SERVE, port))
        .unwrap();
    let supervisor = Supervisor::with(home.store.clone(), fast(false));
    let shown = supervisor.start("serve").await.unwrap();
    assert!(shown.status.running);
    assert!(shown.status.started_here);
    assert_eq!(shown.api.state, ApiState::Valid);
    assert_eq!(shown.api.operations, Some(3));
    let pid = shown.status.pid.unwrap();
    assert!(process::alive(pid));
    assert!(home.store.running().unwrap().contains_key("serve"));

    let again = supervisor.start("serve").await.unwrap_err();
    assert_eq!(
        (again.status, again.error.as_str()),
        (409, "already_running")
    );

    let listed = supervisor.list(true).await.unwrap();
    assert_eq!(
        listed[0].document.as_ref().unwrap()["info"]["title"],
        "Fake"
    );

    let stopped = supervisor.stop("serve").await.unwrap();
    assert!(!stopped.status.running);
    assert!(!stopped.status.started_here);
    assert!(!process::alive(pid));
    assert!(!home.store.running().unwrap().contains_key("serve"));

    let lines = supervisor.logs("serve", 50).unwrap();
    assert!(lines
        .iter()
        .any(|l| l.line.starts_with("--- ") && l.line.ends_with(" start")));
    assert!(lines.iter().any(|l| l.line.contains("GET /openapi.json")));

    let not_ours = supervisor.stop("serve").await.unwrap_err();
    assert_eq!(not_ours.error, "not_ours");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_supervised_app_streams_its_pipes_into_the_ring_and_the_file() {
    if !python() {
        return;
    }
    let home = home();
    let port = free_port();
    let mut app = new_app(
        &home,
        "Piped",
        &format!("echo hello; echo oops >&2; {SERVE}"),
        port,
    );
    app.restart = Some(Restart::OnFailure);
    home.store.register(app).unwrap();
    let supervisor = Supervisor::with(home.store.clone(), fast(true));
    let mut events = supervisor.events();
    supervisor.start("piped").await.unwrap();
    eventually("both streams", || {
        let lines = supervisor.replay("piped", None);
        lines
            .iter()
            .any(|(_, l)| l.stream == Stream::Out && l.line == "hello")
            && lines
                .iter()
                .any(|(_, l)| l.stream == Stream::Err && l.line == "oops")
    })
    .await;
    let mut rx = supervisor.subscribe("piped");
    let _ = crate::openapi::check(&home.store, &home.store.app("piped").unwrap());
    let line = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(line.1.line.contains("GET /openapi.json"));
    let event = events.recv().await.unwrap();
    assert_eq!(event.id, "piped");

    let file = std::fs::read_to_string(home.store.log_path("piped")).unwrap();
    assert!(file.contains("hello\n"));
    assert!(file.contains("oops\n"));
    supervisor.stop("piped").await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    let status = supervisor.status("piped").await.unwrap();
    assert!(!status.running);
    assert_eq!(status.restarts, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn on_failure_restarts_with_backoff_then_gives_up() {
    if !python() {
        return;
    }
    let home = home();
    let crash = "exec python3 -c \"import http.server,socketserver,threading,os,time; socketserver.TCPServer.allow_reuse_address=True; s=socketserver.TCPServer(('127.0.0.1',int(os.environ['PORT'])),http.server.SimpleHTTPRequestHandler); threading.Thread(target=s.serve_forever,daemon=True).start(); time.sleep(0.6); os._exit(3)\"";
    let mut app = new_app(&home, "Crash", crash, free_port());
    app.restart = Some(Restart::OnFailure);
    home.store.register(app).unwrap();
    let supervisor = Supervisor::with(home.store.clone(), fast(true));
    supervisor.start("crash").await.unwrap();
    let mut status = supervisor.status("crash").await.unwrap();
    for _ in 0..200 {
        if status.crashed {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        status = supervisor.status("crash").await.unwrap();
    }
    assert!(status.crashed, "{status:?}");
    assert_eq!(status.restarts, 3);
    assert_eq!(status.last_exit.unwrap().code, Some(3));
    assert!(!status.started_here);
    let starts = std::fs::read_to_string(home.store.log_path("crash"))
        .unwrap()
        .matches(" start\n")
        .count();
    assert_eq!(starts, 4);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_app_that_exits_before_its_port_opens_is_reported_and_not_restarted() {
    let home = home();
    let mut app = new_app(&home, "Quit", "echo bye; exit 4", free_port());
    app.restart = Some(Restart::Always);
    home.store.register(app).unwrap();
    let supervisor = Supervisor::with(home.store.clone(), fast(true));
    let refused = supervisor.start("quit").await.unwrap_err();
    assert_eq!((refused.status, refused.error.as_str()), (500, "exited"));
    assert!(refused.message.contains("with code 4"));
    assert!(refused.lines.iter().any(|l| l.line == "bye"));
    tokio::time::sleep(Duration::from_millis(300)).await;
    let status = supervisor.status("quit").await.unwrap();
    assert_eq!(status.restarts, 0);
    assert!(!status.started_here);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_port_that_never_opens_is_port_closed() {
    let home = home();
    home.store
        .register(new_app(&home, "Sleepy", "exec sleep 30", free_port()))
        .unwrap();
    let mut settings = fast(false);
    settings.port_wait = Duration::from_millis(400);
    let supervisor = Supervisor::with(home.store.clone(), settings);
    let refused = supervisor.start("sleepy").await.unwrap_err();
    assert_eq!(
        (refused.status, refused.error.as_str()),
        (504, "port_closed")
    );
    assert!(supervisor.started_here("sleepy"));
    supervisor.stop("sleepy").await.unwrap();
    assert!(!supervisor.started_here("sleepy"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_process_ai2_did_not_start_is_never_stopped() {
    if !python() {
        return;
    }
    let home = home();
    let port = free_port();
    let mut theirs = std::process::Command::new("python3")
        .args(["-c", SERVER])
        .env("PORT", port.to_string())
        .current_dir(&home.app_dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let pid = theirs.id();
    for _ in 0..100 {
        if crate::port::listening(port) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    home.store
        .register(new_app(&home, "Theirs", SERVE, port))
        .unwrap();
    let supervisor = Supervisor::with(home.store.clone(), fast(false));
    let started = supervisor.start("theirs").await.unwrap_err();
    assert_eq!(
        (started.status, started.error.as_str()),
        (409, "already_running")
    );
    assert!(started.message.contains("not started by ai2"));
    let stopped = supervisor.stop("theirs").await.unwrap_err();
    assert_eq!((stopped.status, stopped.error.as_str()), (409, "not_ours"));

    let start_time = process::start_time(pid).unwrap();
    for forged in [
        Running {
            pid,
            started_at: "1.000000".into(),
            boot: process::boot_id(),
            since: "x".into(),
            piped: false,
        },
        Running {
            pid,
            started_at: start_time,
            boot: "another-boot".into(),
            since: "x".into(),
            piped: false,
        },
    ] {
        home.store.set_running("theirs", Some(forged)).unwrap();
        assert!(supervisor.adopt().await.unwrap().is_empty());
        assert!(!home.store.running().unwrap().contains_key("theirs"));
        assert_eq!(
            supervisor.stop("theirs").await.unwrap_err().error,
            "not_ours"
        );
    }
    assert!(process::alive(pid));
    assert!(crate::port::listening(port));
    theirs.kill().unwrap();
    theirs.wait().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_supervisor_adopts_what_ai2_started_and_can_stop_it() {
    if !python() {
        return;
    }
    let home = home();
    home.store
        .register(new_app(&home, "Adopt", SERVE, free_port()))
        .unwrap();
    let pid = {
        let first = Supervisor::with(home.store.clone(), fast(false));
        first.start("adopt").await.unwrap().status.pid.unwrap()
    };
    let second = Supervisor::with(home.store.clone(), fast(false));
    assert!(!second.started_here("adopt"));
    assert_eq!(second.adopt().await.unwrap(), vec!["adopt".to_string()]);
    let shown = second.show("adopt").await.unwrap();
    assert!(shown.status.running && shown.status.started_here);
    assert_eq!(shown.status.pid, Some(pid));
    assert!(second.adopt().await.unwrap().is_empty());

    let mut rx = second.subscribe("adopt");
    crate::openapi::check(&home.store, &home.store.app("adopt").unwrap()).unwrap();
    let line = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(line.1.stream, Stream::Log);

    let stopped = second.stop("adopt").await.unwrap();
    assert!(!stopped.status.running);
    eventually("the process to end", || !process::alive(pid)).await;
    assert!(!home.store.running().unwrap().contains_key("adopt"));
}

#[test]
fn the_store_uses_ai2_home_apps() {
    let temp = TempDir::new().unwrap();
    let store = Store::at_home(temp.path()).unwrap();
    assert_eq!(store.dir(), temp.path().join("apps"));
    for sub in ["icons", "logs", "openapi"] {
        assert!(Path::new(&store.dir().join(sub)).is_dir());
    }
}
