use anyhow::{bail, Result};
use keys::{Caller, MasterKey, MemoryKey, Platform, Presence};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use vault::{Config, KeyInfo, NoVetter, Running, Verdict, Vetter, Vetting};

const SECRET: &str = "sk_test_SECRET42";

struct Upstream {
    base: String,
    seen: Arc<Mutex<Vec<String>>>,
}

fn upstream() -> Upstream {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let log = Arc::clone(&log);
            std::thread::spawn(move || answer(stream, &log));
        }
    });
    Upstream { base, seen }
}

fn answer(mut stream: TcpStream, log: &Mutex<Vec<String>>) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        let read = stream.read(&mut chunk).unwrap();
        buf.extend_from_slice(&chunk[..read]);
        if let Some(at) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break at + 4;
        }
        if read == 0 {
            return;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let len = head
        .lines()
        .find_map(|l| {
            l.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(|v| v.trim().parse::<usize>().unwrap())
        })
        .unwrap_or(0);
    while buf.len() < head_end + len {
        let read = stream.read(&mut chunk).unwrap();
        buf.extend_from_slice(&chunk[..read]);
    }
    let text = String::from_utf8_lossy(&buf).to_string();
    log.lock().unwrap().push(text);
    let path = head.split(' ').nth(1).unwrap_or("").to_string();
    if path.starts_with("/v1/events") {
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\nb\r\ndata: one\n\n\r\n");
        let _ = stream.flush();
        std::thread::sleep(Duration::from_millis(1500));
        let _ = stream.write_all(b"b\r\ndata: two\n\n\r\n0\r\n\r\n");
        return;
    }
    let reply = if path.starts_with("/v1/zip") {
        if head.to_ascii_lowercase().contains("accept-encoding") {
            "HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: 4\r\n\r\nzzzz".to_string()
        } else {
            "HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nplain".to_string()
        }
    } else if path.starts_with("/v1/redirect") {
        format!("HTTP/1.1 302 Found\r\nLocation: https://evil.example/?k={SECRET}\r\nContent-Length: 0\r\n\r\n")
    } else {
        let (a, b) = SECRET.split_at(7);
        let encoded = "c2tfdGVzdF9TRUNSRVQ0Mg==";
        let body1 = format!("data: {a}");
        let body2 = format!("{b} and {encoded}\n\n");
        format!(
            "HTTP/1.1 200 OK\r\nX-Echo: {SECRET}\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{body1}\r\n{:x}\r\n{body2}\r\n0\r\n\r\n",
            body1.len(),
            body2.len()
        )
    };
    let _ = stream.write_all(reply.as_bytes());
}

#[derive(Default)]
struct Gate {
    present: std::sync::atomic::AtomicBool,
    asked: Mutex<Vec<String>>,
    callers: Mutex<Vec<Caller>>,
    delay: Mutex<Duration>,
}

struct Door(Arc<Gate>);

impl Presence for Door {
    fn confirm(&self, action: &str, caller: &Caller, _passphrase: Option<&str>) -> Result<bool> {
        self.0.asked.lock().unwrap().push(action.to_string());
        self.0.callers.lock().unwrap().push(*caller);
        let delay = *self.0.delay.lock().unwrap();
        std::thread::sleep(delay);
        Ok(self.0.present.load(Ordering::SeqCst))
    }

    fn kind(&self) -> &'static str {
        keys::MEMORY
    }
}

struct Test {
    _dir: tempfile::TempDir,
    gate: Arc<Gate>,
    running: Option<Running>,
    admin: PathBuf,
    state: PathBuf,
    upstream: Upstream,
}

impl Drop for Test {
    fn drop(&mut self) {
        if let Some(running) = self.running.take() {
            running.stop();
        }
    }
}

fn start(present: bool, vetter: Box<dyn Vetter>) -> Test {
    start_owned(present, vetter, None)
}

fn start_owned(present: bool, vetter: Box<dyn Vetter>, owners: Option<Vec<u32>>) -> Test {
    start_with(present, vetter, |config| {
        if let Some(owners) = owners {
            config.owners = owners;
        }
    })
}

fn start_with(present: bool, vetter: Box<dyn Vetter>, tweak: impl FnOnce(&mut Config)) -> Test {
    let upstream = upstream();
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("v");
    let admin = state.join("admin.sock");
    let mut config = Config::new(state.clone(), admin.clone());
    config.sockets = dir.path().join("s");
    config.http_prefixes = true;
    config.plain_http = vec![upstream.base.trim_start_matches("http://").to_string()];
    config.approval_timeout = Duration::from_millis(400);
    config.throttle = Duration::ZERO;
    tweak(&mut config);
    let master = MemoryKey::default();
    master.create().unwrap();
    let gate = Arc::new(Gate::default());
    gate.present.store(present, Ordering::SeqCst);
    let platform = Platform {
        master: Box::new(master),
        presence: Box::new(Door(Arc::clone(&gate))),
    };
    let running = vault::start(config, platform, |_| vetter).unwrap();
    Test {
        _dir: dir,
        gate,
        running: Some(running),
        admin,
        state,
        upstream,
    }
}

fn raw(stream: &mut (impl Read + Write), request: &[u8]) -> (u16, String, Vec<u8>) {
    stream.write_all(request).unwrap();
    let mut all = Vec::new();
    stream.read_to_end(&mut all).unwrap();
    let at = all.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8_lossy(&all[..at]).to_string();
    let status = head.split(' ').nth(1).unwrap().parse().unwrap();
    let mut body = all[at + 4..].to_vec();
    if head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        body = dechunk(&body);
    }
    (status, head, body)
}

fn dechunk(mut data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let line_end = data.windows(2).position(|w| w == b"\r\n").unwrap();
        let size =
            usize::from_str_radix(std::str::from_utf8(&data[..line_end]).unwrap(), 16).unwrap();
        if size == 0 {
            return out;
        }
        out.extend_from_slice(&data[line_end + 2..line_end + 2 + size]);
        data = &data[line_end + 4 + size..];
    }
}

fn admin(test: &Test, method: &str, path: &str, body: Value) -> (u16, Value) {
    let body = body.to_string();
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: v\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    let mut stream = UnixStream::connect(&test.admin).unwrap();
    let (status, _, body) = raw(&mut stream, request.as_bytes());
    (status, serde_json::from_slice(&body).unwrap())
}

fn call_on(
    stream: &mut (impl Read + Write),
    method: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> (u16, String, String) {
    let find = |wanted: &str| {
        headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(wanted))
            .map(|(_, value)| *value)
    };
    let target = match (find("X-Vault-Key"), find("X-Vault-Url")) {
        (Some(key), Some(url)) => format!("/{key}/{}", url.split_once("://").unwrap().1),
        (Some(key), None) => format!("/{key}"),
        _ => "/".to_string(),
    };
    let mut request = format!(
        "{method} {target} HTTP/1.1\r\nHost: vault\r\nContent-Length: {}\r\n",
        body.len()
    );
    if let Some(token) = find("X-Vault-Token") {
        request.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    for (name, value) in headers {
        if !["x-vault-token", "x-vault-key", "x-vault-url"]
            .contains(&name.to_ascii_lowercase().as_str())
        {
            request.push_str(&format!("{name}: {value}\r\n"));
        }
    }
    request.push_str("\r\n");
    request.push_str(body);
    let (status, head, body) = raw(stream, request.as_bytes());
    (status, head, String::from_utf8(body).unwrap())
}

fn call(
    socket: &Path,
    method: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> (u16, String, String) {
    call_on(
        &mut UnixStream::connect(socket).unwrap(),
        method,
        headers,
        body,
    )
}

fn reason(answer: &(u16, String, String)) -> String {
    assert_eq!(answer.0, 403, "{answer:?}");
    serde_json::from_str::<Value>(&answer.2).unwrap()["reason"]
        .as_str()
        .unwrap()
        .to_string()
}

fn put_key(test: &Test) {
    let (status, value) = admin(
        test,
        "PUT",
        "/keys/svc",
        json!({ "secret": SECRET, "description": "a test API", "approved_urls": [format!("{}/v1/", test.upstream.base)], "inject": "bearer" }),
    );
    assert_eq!(status, 200, "{value}");
}

fn sandbox(test: &Test, name: &str) -> PathBuf {
    let (status, value) = admin(test, "POST", "/sandboxes", json!({ "name": name }));
    assert_eq!(status, 200, "{value}");
    PathBuf::from(value["sandbox"]["path"].as_str().unwrap())
}

fn token(test: &Test, body: Value) -> String {
    let (status, value) = admin(test, "POST", "/tokens", body);
    assert_eq!(status, 200, "{value}");
    value["token"].as_str().unwrap().to_string()
}

#[test]
fn the_gateway_never_asks_for_compressed_answers() {
    let test = start(true, Box::new(NoVetter));
    put_key(&test);
    let socket = sandbox(&test, "box");
    let token = token(
        &test,
        json!({ "keys": ["svc"], "sandbox": "box", "mode": "app", "methods": "read" }),
    );
    let url = format!("{}/v1/zip", test.upstream.base);
    let headers = [
        ("X-Vault-Token", token.as_str()),
        ("X-Vault-Key", "svc"),
        ("X-Vault-Url", url.as_str()),
        ("Accept-Encoding", "gzip"),
    ];
    let (status, head, body) = call(&socket, "GET", &headers, "");
    assert_eq!((status, body.as_str()), (200, "plain"), "{head}");
    let seen = test.upstream.seen.lock().unwrap().join("\n");
    assert!(!seen.to_ascii_lowercase().contains("accept-encoding"));
}

#[test]
fn a_token_use_that_cannot_be_saved_is_an_internal_error() {
    use std::os::unix::fs::PermissionsExt;
    let test = start(true, Box::new(NoVetter));
    put_key(&test);
    let socket = sandbox(&test, "box");
    let token = token(
        &test,
        json!({ "keys": ["svc"], "sandbox": "box", "mode": "app", "uses": 2 }),
    );
    let url = format!("{}/v1/echo", test.upstream.base);
    let headers = [
        ("X-Vault-Token", token.as_str()),
        ("X-Vault-Key", "svc"),
        ("X-Vault-Url", url.as_str()),
    ];
    let locked = std::fs::Permissions::from_mode(0o500);
    std::fs::set_permissions(&test.state, locked).unwrap();
    let (status, _, body) = call(&socket, "GET", &headers, "");
    std::fs::set_permissions(&test.state, std::fs::Permissions::from_mode(0o700)).unwrap();
    if unsafe { libc::geteuid() } != 0 {
        assert_eq!(status, 500, "{body}");
        let value: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(value["error"], "internal");
    }
}

#[test]
fn an_app_call_gets_the_secret_upstream_and_never_back() {
    let test = start(true, Box::new(NoVetter));
    put_key(&test);
    let socket = sandbox(&test, "box");
    let other = sandbox(&test, "other");
    let token = token(
        &test,
        json!({ "keys": ["svc"], "sandbox": "box", "mode": "app", "methods": "read" }),
    );
    let url = format!("{}/v1/echo", test.upstream.base);
    let headers = [
        ("X-Vault-Token", token.as_str()),
        ("X-Vault-Key", "svc"),
        ("X-Vault-Url", url.as_str()),
        ("X-Vault-Extra", "x"),
        ("X-Mine", "1"),
    ];
    let (status, head, body) = call(&socket, "GET", &headers, "");
    assert_eq!(status, 200, "{head} {body}");
    assert_eq!(body, "data: [redacted] and [redacted]\n\n");
    assert!(
        head.contains("X-Echo: [redacted]") || head.contains("x-echo: [redacted]"),
        "{head}"
    );
    assert!(!head.contains(SECRET));
    let seen = test
        .upstream
        .seen
        .lock()
        .unwrap()
        .join("\n")
        .to_ascii_lowercase();
    assert!(seen.contains(&format!(
        "authorization: bearer {}",
        SECRET.to_ascii_lowercase()
    )));
    assert!(seen.contains("x-mine: 1") && !seen.contains("x-vault"));

    assert_eq!(
        reason(&call(&socket, "POST", &headers, "{}")),
        "approval_timeout"
    );
    let outside = format!("{}/v2/echo", test.upstream.base);
    let mut moved = headers;
    moved[2].1 = &outside;
    assert_eq!(
        reason(&call(&socket, "GET", &moved, "")),
        "approval_timeout"
    );
    assert_eq!(reason(&call(&other, "GET", &headers, "")), "wrong_sandbox");
    let mut unknown = headers;
    unknown[0].1 = "vt_nope";
    assert_eq!(reason(&call(&socket, "GET", &unknown, "")), "token_unknown");
    let mut wrong_key = headers;
    wrong_key[1].1 = "other";
    assert_eq!(
        reason(&call(&socket, "GET", &wrong_key, "")),
        "key_not_allowed"
    );

    let redirect = format!("{}/v1/redirect", test.upstream.base);
    let mut redirected = headers;
    redirected[2].1 = &redirect;
    let (status, head, _) = call(&socket, "GET", &redirected, "");
    assert_eq!(status, 302);
    assert!(head.contains("?k=[redacted]"), "{head}");

    let once = self::token(
        &test,
        json!({ "keys": ["svc"], "sandbox": "box", "mode": "app", "uses": 1 }),
    );
    let mut single = headers;
    single[0].1 = &once;
    assert_eq!(call(&socket, "GET", &single, "").0, 200);
    assert_eq!(reason(&call(&socket, "GET", &single, "")), "token_used_up");

    let (_, audit) = admin(&test, "GET", "/audit?tail=100", json!({}));
    let entries = audit["entries"].as_array().unwrap();
    assert!(entries
        .iter()
        .any(|e| e["decision"] == "allow" && e["status"] == 200));
    assert!(entries.iter().any(|e| e["reason"] == "token_used_up"));
    for file in ["audit.jsonl", "tokens.json", "keys.json"] {
        let text = std::fs::read_to_string(test.state.join(file)).unwrap();
        assert!(!text.contains(SECRET) && !text.contains(&token), "{file}");
    }
    let (_, listed) = admin(&test, "GET", "/tokens", json!({}));
    assert!(!listed.to_string().contains(&token));
    assert!(listed["tokens"][0]["expires"].is_string());
}

struct Judge {
    asked: AtomicUsize,
}

impl Vetter for Judge {
    fn propose(&self, key: &KeyInfo) -> Result<Vec<String>> {
        Ok(vec![
            format!("https://{}.example/", key.name),
            "nonsense".into(),
        ])
    }
    fn vet(&self, call: &Vetting) -> Result<Verdict> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        if call.request.contains("garbage") {
            bail!("unparseable");
        }
        let base = call.url.rsplit_once('/').unwrap().0;
        Ok(Verdict {
            allow: !call.request.contains("forbidden"),
            prefix: if call.request.contains("wide") {
                "http://".into()
            } else {
                format!("{base}/")
            },
            why: "test".into(),
        })
    }
}

#[test]
fn an_agent_call_is_vetted_once_and_writes_wait_for_a_human() {
    let judge = Arc::new(Judge {
        asked: AtomicUsize::new(0),
    });
    struct Shared(Arc<Judge>);
    impl Vetter for Shared {
        fn propose(&self, key: &KeyInfo) -> Result<Vec<String>> {
            self.0.propose(key)
        }
        fn vet(&self, call: &Vetting) -> Result<Verdict> {
            self.0.vet(call)
        }
    }
    let test = Arc::new(start(true, Box::new(Shared(Arc::clone(&judge)))));
    put_key(&test);
    let (_, proposed) = admin(
        &test,
        "POST",
        "/keys/stripe/propose",
        json!({ "description": "Stripe" }),
    );
    assert_eq!(
        proposed["approved_urls"],
        json!(["https://stripe.example/"])
    );
    let socket = sandbox(&test, "agentbox");
    let made = |intent: &str| {
        token(
            &test,
            json!({ "keys": ["svc"], "sandbox": "agentbox", "mode": "agent", "methods": "all", "intent": intent }),
        )
    };
    let (status, value) = admin(
        &test,
        "POST",
        "/tokens",
        json!({ "keys": ["svc"], "sandbox": "agentbox", "mode": "agent" }),
    );
    assert_eq!(
        (status, value["error"].as_str()),
        (400, Some("bad_request"))
    );
    let agent = made("read the test data");
    let url = format!("{}/v1/echo", test.upstream.base);
    let headers = |token: &str, request: &str| -> Vec<(String, String)> {
        vec![
            ("X-Vault-Token".into(), token.into()),
            ("X-Vault-Key".into(), "svc".into()),
            ("X-Vault-Url".into(), url.clone()),
            ("X-Vault-Request".into(), request.into()),
        ]
    };
    let send = |method: &str, h: &[(String, String)]| {
        let pairs: Vec<(&str, &str)> = h.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        call(&socket, method, &pairs, "{}")
    };
    assert_eq!(send("GET", &headers(&agent, "read%20it")).0, 200);
    assert_eq!(send("GET", &headers(&agent, "again")).0, 200);
    assert_eq!(judge.asked.load(Ordering::SeqCst), 1);
    assert_eq!(send("GET", &headers(&agent, "bad%zz")).0, 400);

    assert_eq!(
        reason(&send("POST", &headers(&agent, "write"))),
        "approval_timeout"
    );
    let waiting = {
        let test = Arc::clone(&test);
        let h = headers(&agent, "write%20it");
        let socket = socket.clone();
        std::thread::spawn(move || {
            let pairs: Vec<(&str, &str)> =
                h.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
            let _keep = &test;
            call(&socket, "POST", &pairs, "{}")
        })
    };
    let id = loop {
        let (_, listed) = admin(&test, "GET", "/approvals", json!({}));
        if let Some(first) = listed["approvals"]
            .as_array()
            .and_then(|a| a.first().cloned())
        {
            assert_eq!(first["request"], "write it");
            break first["id"].as_str().unwrap().to_string();
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(
        admin(
            &test,
            "POST",
            &format!("/approvals/{id}"),
            json!({ "approve": true })
        )
        .0,
        200
    );
    assert_eq!(waiting.join().unwrap().0, 200);

    let denied = made("only reads");
    let answer = send("GET", &headers(&denied, "forbidden"));
    assert_eq!(reason(&answer), "vetting_denied");
    assert!(answer.2.contains("only reads"));
    assert_eq!(
        reason(&send("GET", &headers(&made("x"), "garbage"))),
        "vetting_unavailable"
    );
    assert_eq!(
        reason(&send("GET", &headers(&made("y"), "wide"))),
        "vetting_unavailable"
    );
}

#[test]
fn events_pass_through_as_they_come() {
    let test = start(true, Box::new(NoVetter));
    put_key(&test);
    let socket = sandbox(&test, "box");
    let app = token(
        &test,
        json!({ "keys": ["svc"], "sandbox": "box", "mode": "app" }),
    );
    let url = format!("{}/v1/events", test.upstream.base);
    let mut stream = UnixStream::connect(&socket).unwrap();
    let request = format!(
        "GET /svc/{} HTTP/1.1\r\nAuthorization: Bearer {app}\r\n\r\n",
        url.split_once("://").unwrap().1
    );
    stream.write_all(request.as_bytes()).unwrap();
    let began = std::time::Instant::now();
    let mut seen = Vec::new();
    let mut chunk = [0u8; 1024];
    while !String::from_utf8_lossy(&seen).contains("data: one\n\n") {
        let read = stream.read(&mut chunk).unwrap();
        assert!(read > 0);
        seen.extend_from_slice(&chunk[..read]);
    }
    assert!(began.elapsed() < Duration::from_millis(1200));
    stream.read_to_end(&mut seen).unwrap();
    assert!(String::from_utf8_lossy(&seen).contains("data: two"));
}

#[test]
fn no_model_means_agent_calls_are_refused() {
    let test = start(true, Box::new(NoVetter));
    put_key(&test);
    let socket = sandbox(&test, "box");
    let agent = token(
        &test,
        json!({ "keys": ["svc"], "sandbox": "box", "mode": "agent", "intent": "read" }),
    );
    let url = format!("{}/v1/echo", test.upstream.base);
    let answer = call(
        &socket,
        "GET",
        &[
            ("X-Vault-Token", &agent),
            ("X-Vault-Key", "svc"),
            ("X-Vault-Url", &url),
        ],
        "",
    );
    assert_eq!(reason(&answer), "vetting_unavailable");
}

#[test]
fn changes_need_a_human_present() {
    let test = start(false, Box::new(NoVetter));
    let (status, value) = admin(
        &test,
        "PUT",
        "/keys/svc",
        json!({ "secret": SECRET, "approved_urls": ["https://h.example/"] }),
    );
    assert_eq!(
        (status, value["error"].as_str()),
        (401, Some("presence_required"))
    );
    assert_eq!(
        admin(&test, "GET", "/keys", json!({})).1,
        json!({ "keys": [] })
    );
    let (status, value) = admin(&test, "GET", "/status", json!({}));
    assert_eq!(status, 200);
    assert_eq!(value["master_key"], true);
    assert_eq!(value["platform"], std::env::consts::OS);
    assert_eq!(value["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(value["presence"], keys::MEMORY);
    let (status, doc) = admin(&test, "GET", "/openapi.json", json!({}));
    assert_eq!(status, 200);
    assert!(doc["paths"]["/batch"]["post"].is_object());
    let (status, doc) = admin(&test, "GET", "/openapi/gateway.json", json!({}));
    assert_eq!(status, 200);
    assert!(doc["paths"]["/{key}/{host}/{path}"]["get"].is_object());
    let callers = test.gate.callers.lock().unwrap().clone();
    assert_eq!(
        callers,
        [Caller {
            pid: Some(std::process::id()),
            uid: Some(unsafe { libc::geteuid() }),
        }]
    );
}

fn first_waiting(test: &Test) -> Value {
    loop {
        let (_, listed) = admin(test, "GET", "/approvals", json!({}));
        if let Some(first) = listed["approvals"]
            .as_array()
            .and_then(|a| a.first().cloned())
        {
            return first;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn an_app_call_outside_its_scope_waits_for_a_present_human() {
    let test = start(true, Box::new(NoVetter));
    put_key(&test);
    let socket = sandbox(&test, "box");
    let app = token(
        &test,
        json!({ "keys": ["svc"], "sandbox": "box", "mode": "app", "methods": "read", "ttl": "2h", "intent": "sync invoices" }),
    );
    let asked = test.gate.asked.lock().unwrap().clone();
    let made = asked
        .iter()
        .find(|a| a.starts_with("Make an app token"))
        .unwrap();
    for part in [
        "svc",
        "box",
        "GET, HEAD, OPTIONS",
        "the keys' approved URLs",
        "2h",
        "sync invoices",
    ] {
        assert!(made.contains(part), "{made} lacks {part}");
    }
    assert!(asked
        .iter()
        .any(|a| a.starts_with("Store the vault key svc") && a.contains("/v1/")));
    let url = format!("{}/v1/echo", test.upstream.base);
    let waiting = {
        let socket = socket.clone();
        let (app, url) = (app.clone(), url.clone());
        std::thread::spawn(move || {
            call(
                &socket,
                "POST",
                &[
                    ("X-Vault-Token", &app),
                    ("X-Vault-Key", "svc"),
                    ("X-Vault-Url", &url),
                ],
                "{}",
            )
        })
    };
    let first = first_waiting(&test);
    let line = first["line"].as_str().unwrap().to_string();
    let bare = url.split_once("://").unwrap().1;
    assert!(
        line.starts_with("app token tok_")
            && line.ends_with(&format!("wants POST {bare} with key svc")),
        "{line}"
    );
    let id = first["id"].as_str().unwrap().to_string();
    test.gate.present.store(false, Ordering::SeqCst);
    let (status, value) = admin(
        &test,
        "POST",
        &format!("/approvals/{id}"),
        json!({ "approve": true }),
    );
    assert_eq!(
        (status, value["error"].as_str()),
        (401, Some("presence_required"))
    );
    test.gate.present.store(true, Ordering::SeqCst);
    assert_eq!(
        admin(
            &test,
            "POST",
            &format!("/approvals/{id}"),
            json!({ "approve": true })
        )
        .0,
        200
    );
    assert_eq!(waiting.join().unwrap().0, 200);
    let asked = test.gate.asked.lock().unwrap().clone();
    assert!(asked
        .iter()
        .any(|a| a == &format!("Allow this call: {line}")));
}

#[test]
fn only_the_daemon_owner_may_call_its_sockets() {
    let other = unsafe { libc::geteuid() } + 1;
    let test = start_owned(true, Box::new(NoVetter), Some(vec![other]));
    let (status, value) = admin(&test, "GET", "/keys", json!({}));
    assert_eq!(
        (status, value["reason"].as_str()),
        (403, Some("wrong_user"))
    );
}

#[test]
fn a_tcp_endpoint_takes_calls_only_from_its_peers() {
    let test = start(true, Box::new(NoVetter));
    put_key(&test);
    let open = |name: &str, peer: &str| {
        let (status, value) = admin(
            &test,
            "POST",
            "/sandboxes",
            json!({ "name": name, "tcp": "127.0.0.1:0", "peers": [peer] }),
        );
        assert_eq!(status, 200, "{value}");
        value["sandbox"]["listen"].as_str().unwrap().to_string()
    };
    let near = open("near", "127.0.0.1");
    let far = open("far", "10.9.9.9");
    let url = format!("{}/v1/echo", test.upstream.base);
    let near_token = token(
        &test,
        json!({ "keys": ["svc"], "sandbox": "near", "mode": "app" }),
    );
    let far_token = token(
        &test,
        json!({ "keys": ["svc"], "sandbox": "far", "mode": "app" }),
    );
    let send = |addr: &str, token: &str| {
        call_on(
            &mut TcpStream::connect(addr).unwrap(),
            "GET",
            &[
                ("X-Vault-Token", token),
                ("X-Vault-Key", "svc"),
                ("X-Vault-Url", &url),
            ],
            "",
        )
    };
    assert_eq!(send(&near, &near_token).0, 200);
    assert_eq!(reason(&send(&far, &far_token)), "wrong_sandbox");
    assert_eq!(admin(&test, "DELETE", "/sandboxes/near", json!({})).0, 200);
    assert!(TcpStream::connect(&near).is_err() || send(&near, &near_token).0 != 200);
}

#[test]
fn the_cli_talks_to_the_daemon_and_hands_back_extra_subcommands() {
    let test = start(true, Box::new(NoVetter));
    put_key(&test);
    let opts = vault::Options {
        admin: test.admin.clone(),
        state: test.state.clone(),
        sockets: test.state.join("s"),
        ask_passphrase: false,
        serve: Box::new(|_| Ok(())),
    };
    let command = vault::command_with(vec![clap::Command::new("ui")]);
    let has = command
        .clone()
        .try_get_matches_from(["vault", "has", "--name", "svc"])
        .unwrap();
    assert!(matches!(
        vault::outcome(&has, &opts).unwrap(),
        vault::Outcome::Code(0)
    ));
    let missing = command
        .clone()
        .try_get_matches_from(["vault", "has", "--name", "nope"])
        .unwrap();
    assert!(matches!(
        vault::outcome(&missing, &opts).unwrap(),
        vault::Outcome::Code(1)
    ));
    let ui = command
        .clone()
        .try_get_matches_from(["vault", "ui"])
        .unwrap();
    assert!(
        matches!(vault::outcome(&ui, &opts).unwrap(), vault::Outcome::Extra(name, _) if name == "ui")
    );
    let file = test.state.join("changes.json");
    std::fs::write(
        &file,
        json!([
            { "action": "add_sandbox", "name": "applied" },
            { "action": "create_token", "keys": ["svc"], "sandbox": "applied", "mode": "app" },
        ])
        .to_string(),
    )
    .unwrap();
    let asked = test.gate.asked.lock().unwrap().len();
    let apply = command
        .clone()
        .try_get_matches_from(["vault", "apply", file.to_str().unwrap()])
        .unwrap();
    assert!(matches!(
        vault::outcome(&apply, &opts).unwrap(),
        vault::Outcome::Code(0)
    ));
    assert_eq!(test.gate.asked.lock().unwrap().len(), asked + 1);
    let (_, tokens) = admin(&test, "GET", "/tokens", json!({}));
    assert_eq!(tokens["tokens"][0]["sandbox"], "applied");
    std::fs::write(&file, "nope").unwrap();
    assert!(matches!(
        vault::outcome(&apply, &opts).unwrap(),
        vault::Outcome::Code(64)
    ));
    let sandbox = command
        .try_get_matches_from(["vault", "sandbox", "add", "--name", "cli"])
        .unwrap();
    assert!(matches!(
        vault::outcome(&sandbox, &opts).unwrap(),
        vault::Outcome::Code(0)
    ));
    assert!(vault::command()
        .try_get_matches_from([
            "vault",
            "token",
            "create",
            "--sandbox",
            "s",
            "--mode",
            "app"
        ])
        .is_err());
}

#[test]
fn refused_prompts_slow_down_and_only_one_is_open_at_a_time() {
    let test = Arc::new(start_with(false, Box::new(NoVetter), |config| {
        config.throttle = Duration::from_millis(300);
        config.throttle_limit = Duration::from_millis(500);
    }));
    let put = |test: &Test| {
        admin(
            test,
            "PUT",
            "/keys/svc",
            json!({ "secret": SECRET, "approved_urls": ["https://h.example/"] }),
        )
    };
    assert_eq!(put(&test).0, 401);
    let (status, value) = put(&test);
    assert_eq!(
        (status, value["error"].as_str(), value["seconds"].as_u64()),
        (429, Some("throttled"), Some(1))
    );
    assert_eq!(test.gate.asked.lock().unwrap().len(), 1);
    std::thread::sleep(Duration::from_millis(350));
    assert_eq!(put(&test).0, 401);
    std::thread::sleep(Duration::from_millis(350));
    assert_eq!(put(&test).0, 429);
    std::thread::sleep(Duration::from_millis(200));

    test.gate.present.store(true, Ordering::SeqCst);
    *test.gate.delay.lock().unwrap() = Duration::from_millis(400);
    let first = {
        let test = Arc::clone(&test);
        std::thread::spawn(move || put(&test).0)
    };
    while test.gate.asked.lock().unwrap().len() < 3 {
        std::thread::sleep(Duration::from_millis(5));
    }
    let (status, value) = admin(&test, "DELETE", "/sandboxes/none", json!({}));
    assert_eq!((status, value["error"].as_str()), (404, Some("not_found")));
    let (status, value) = admin(&test, "POST", "/sandboxes", json!({ "name": "box" }));
    assert_eq!((status, value["error"].as_str()), (409, Some("busy")));
    assert_eq!(first.join().unwrap(), 200);
    *test.gate.delay.lock().unwrap() = Duration::ZERO;
    test.gate.present.store(false, Ordering::SeqCst);
    assert_eq!(put(&test).0, 401);
    let (_, value) = put(&test);
    assert_eq!(value["seconds"].as_u64(), Some(1));
}

#[test]
fn a_group_of_changes_is_one_prompt_and_applies_all_or_none() {
    let test = start(true, Box::new(NoVetter));
    let url = format!("{}/v1/", test.upstream.base);
    let asked = |test: &Test| test.gate.asked.lock().unwrap().clone();
    let (status, value) = admin(
        &test,
        "PUT",
        "/keys",
        json!([
            { "name": "a", "secret": "sa", "approved_urls": [url] },
            { "name": "b", "secret": "sb", "approved_urls": [url], "inject": "header:X-Key" },
        ]),
    );
    assert_eq!(status, 200, "{value}");
    assert_eq!(value["keys"][1]["inject"], "header:X-Key");
    let prompts = asked(&test);
    assert_eq!(prompts.len(), 1);
    assert!(
        prompts[0]
            .starts_with("Make these 2 vault changes, all or none: (1) Store the vault key a")
            && prompts[0].contains("(2) Store the vault key b"),
        "{}",
        prompts[0]
    );

    let (status, value) = admin(
        &test,
        "POST",
        "/batch",
        json!({ "actions": [
            { "action": "put_key", "name": "svc", "secret": SECRET, "approved_urls": [url] },
            { "action": "add_sandbox", "name": "box" },
            { "action": "create_token", "keys": ["svc", "a"], "sandbox": "box", "mode": "app" },
            { "action": "delete_key", "name": "b" },
        ] }),
    );
    assert_eq!(status, 200, "{value}");
    assert_eq!(asked(&test).len(), 2);
    let results = value["results"].as_array().unwrap();
    assert_eq!(results[3], json!({ "deleted": "b" }));
    let socket = PathBuf::from(results[1]["sandbox"]["path"].as_str().unwrap());
    let token = results[2]["token"].as_str().unwrap();
    let echo = format!("{url}echo");
    let headers = [
        ("X-Vault-Token", token),
        ("X-Vault-Key", "svc"),
        ("X-Vault-Url", echo.as_str()),
    ];
    assert_eq!(call(&socket, "GET", &headers, "").0, 200);

    let keys_before = admin(&test, "GET", "/keys", json!({})).1;
    let refused = [
        (
            json!([
                { "action": "put_key", "name": "c", "secret": "sc", "approved_urls": [url] },
            ]),
            400,
        ),
        (
            json!({ "actions": [
                { "action": "put_key", "name": "c", "secret": "sc", "approved_urls": [url] },
                { "action": "create_token", "keys": ["nope"], "sandbox": "box", "mode": "app" },
            ] }),
            404,
        ),
        (
            json!({ "actions": [
                { "action": "add_sandbox", "name": "two" },
                { "action": "remove_sandbox", "name": "two" },
            ] }),
            400,
        ),
        (
            json!({ "actions": [
                { "action": "put_key", "name": "c", "secret": "sc", "approved_urls": [url] },
                { "action": "add_sandbox", "name": "box" },
            ] }),
            409,
        ),
        (json!({ "actions": [{ "action": "launch_rockets" }] }), 400),
        (json!({ "actions": [] }), 400),
    ];
    for (body, expected) in refused {
        let (status, value) = admin(&test, "POST", "/batch", body);
        assert_eq!(status, expected, "{value}");
    }
    assert_eq!(asked(&test).len(), 2);

    std::fs::write(test.state.join("not-a-socket"), "x").unwrap();
    let (status, value) = admin(
        &test,
        "POST",
        "/batch",
        json!({ "actions": [
            { "action": "put_key", "name": "c", "secret": "sc", "approved_urls": [url] },
            { "action": "add_sandbox", "name": "fine" },
            { "action": "add_sandbox", "name": "broken", "socket": test.state.join("not-a-socket") },
        ] }),
    );
    assert_eq!((status, value["error"].as_str()), (500, Some("internal")));
    assert_eq!(admin(&test, "GET", "/keys", json!({})).1, keys_before);
    let sandboxes = admin(&test, "GET", "/sandboxes", json!({})).1;
    assert_eq!(sandboxes["sandboxes"].as_array().unwrap().len(), 1);
    let fine = test.state.parent().unwrap().join("s").join("fine");
    assert!(UnixStream::connect(fine.join("gateway.sock")).is_err());

    test.gate.present.store(false, Ordering::SeqCst);
    let pair = json!([{ "name": "x1" }, { "name": "x2" }]);
    assert_eq!(admin(&test, "POST", "/sandboxes", pair.clone()).0, 401);
    test.gate.present.store(true, Ordering::SeqCst);
    let (status, value) = admin(&test, "POST", "/sandboxes", pair);
    assert_eq!(status, 200, "{value}");
    assert_eq!(value["sandboxes"][1]["name"], "x2");
    let (status, value) = admin(
        &test,
        "POST",
        "/tokens",
        json!([
            { "keys": ["a"], "sandbox": "x1", "mode": "app" },
            { "keys": ["a"], "sandbox": "x2", "mode": "app" },
        ]),
    );
    assert_eq!(status, 200, "{value}");
    let ids: Vec<Value> = value["tokens"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].clone())
        .collect();
    let (status, value) = admin(&test, "DELETE", "/tokens", json!(ids));
    assert_eq!((status, &value["revoked"]), (200, &json!(ids)));
    let left = admin(&test, "GET", "/tokens", json!({})).1;
    assert_eq!(left["tokens"].as_array().unwrap().len(), 1);
    assert_eq!(admin(&test, "PUT", "/keys", json!({ "name": "z" })).0, 400);
}
