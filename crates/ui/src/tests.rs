use crate::{nonce, Options, Server, NONCE_HEADER};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use vault::http::{self, Request};

struct Seen {
    method: String,
    target: String,
    passphrase: Option<String>,
    body: Vec<u8>,
}

struct Admin {
    path: PathBuf,
    seen: Receiver<Seen>,
    stopped: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Admin {
    fn start(dir: &Path, status: u16, reply: serde_json::Value) -> Self {
        let path = dir.join("admin.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let (to, seen) = channel();
        let stopped = Arc::new(AtomicBool::new(false));
        let flag = stopped.clone();
        let thread = thread::spawn(move || {
            for stream in listener.incoming() {
                if flag.load(Ordering::SeqCst) {
                    break;
                }
                let mut stream = stream.unwrap();
                let request: Request = http::read_request(&mut stream).unwrap();
                let _ = to.send(Seen {
                    method: request.method.clone(),
                    target: request.target.clone(),
                    passphrase: request.header("x-vault-passphrase").map(String::from),
                    body: request.body.clone(),
                });
                http::Response::json(status, &reply)
                    .write_to(&mut stream)
                    .unwrap();
            }
        });
        Self {
            path,
            seen,
            stopped,
            thread: Some(thread),
        }
    }
}

impl Drop for Admin {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
        let _ = std::os::unix::net::UnixStream::connect(&self.path);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub(crate) struct Reply {
    pub(crate) status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Reply {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    pub(crate) fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap()
    }

    fn error(&self) -> String {
        self.json()["error"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }

    pub(crate) fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

pub(crate) struct Ui {
    host: String,
    nonce: String,
    runtime: Option<tokio::runtime::Runtime>,
}

impl Ui {
    fn start(home: &Path, admin: &Path) -> Self {
        Self::start_with(Options {
            home: home.to_path_buf(),
            admin: admin.to_path_buf(),
            agents: ::agents::Agents::local(home, ::harness::Registry::public),
        })
    }

    pub(crate) fn start_with(options: Options) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let server = runtime.block_on(Server::bind(0, &options)).unwrap();
        let state = server.state();
        runtime.spawn(server.serve());
        Self {
            host: state.host.clone(),
            nonce: state.nonce.clone(),
            runtime: Some(runtime),
        }
    }

    fn origin(&self) -> String {
        format!("http://{}", self.host)
    }

    fn send(&self, method: &str, target: &str, headers: &[(&str, &str)], body: &[u8]) -> Reply {
        let mut stream = TcpStream::connect(&self.host).unwrap();
        let mut head = format!(
            "{method} {target} HTTP/1.1\r\nConnection: close\r\nContent-Length: {}\r\n",
            body.len()
        );
        for (name, value) in headers {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        head.push_str("\r\n");
        stream.write_all(head.as_bytes()).unwrap();
        let _ = stream.write_all(body);
        let mut raw = Vec::new();
        let _ = stream.read_to_end(&mut raw);
        parse(&raw)
    }

    fn get(&self, target: &str) -> Reply {
        self.send("GET", target, &[("Host", &self.host)], b"")
    }

    pub(crate) fn api(
        &self,
        method: &str,
        target: &str,
        extra: &[(&str, &str)],
        body: &str,
    ) -> Reply {
        let origin = self.origin();
        let mut headers = vec![
            ("Host", self.host.as_str()),
            (NONCE_HEADER, self.nonce.as_str()),
            ("Origin", origin.as_str()),
        ];
        headers.extend_from_slice(extra);
        self.send(method, target, &headers, body.as_bytes())
    }
}

impl Drop for Ui {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

fn parse(raw: &[u8]) -> Reply {
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("a complete response");
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap();
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    let mut body = raw[split + 4..].to_vec();
    if headers
        .iter()
        .any(|(k, v)| k.eq_ignore_ascii_case("transfer-encoding") && v.contains("chunked"))
    {
        body = unchunk(&body);
    }
    Reply {
        status,
        headers,
        body,
    }
}

fn unchunk(mut raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(end) = raw.windows(2).position(|w| w == b"\r\n") {
        let size = usize::from_str_radix(std::str::from_utf8(&raw[..end]).unwrap().trim(), 16)
            .unwrap_or(0);
        if size == 0 {
            break;
        }
        out.extend_from_slice(&raw[end + 2..end + 2 + size]);
        raw = &raw[end + 4 + size..];
    }
    out
}

pub(crate) fn ui_without_daemon(dir: &Path) -> Ui {
    Ui::start(dir, &dir.join("admin.sock"))
}

#[test]
fn api_requests_need_the_nonce_the_host_and_for_writes_the_origin() {
    let dir = tempfile::tempdir().unwrap();
    let admin = Admin::start(dir.path(), 200, serde_json::json!({ "keys": [] }));
    let ui = Ui::start(dir.path(), &admin.path);
    let host = ui.host.clone();
    let origin = ui.origin();
    let nonce = ui.nonce.clone();
    let wrong = "0".repeat(64);

    let refused = [
        ui.send("GET", "/api/vault/keys", &[("Host", &host)], b""),
        ui.send(
            "GET",
            "/api/vault/keys",
            &[("Host", &host), (NONCE_HEADER, &wrong)],
            b"",
        ),
        ui.send(
            "GET",
            "/api/vault/keys",
            &[("Host", "localhost.evil:1"), (NONCE_HEADER, &nonce)],
            b"",
        ),
        ui.send(
            "GET",
            "/api/vault/keys",
            &[
                ("Host", &format!("localhost:{}", &host[10..])),
                (NONCE_HEADER, &nonce),
            ],
            b"",
        ),
        ui.send(
            "GET",
            "/api/vault/keys",
            &[("Host", &host), ("Host", &host), (NONCE_HEADER, &nonce)],
            b"",
        ),
        ui.send(
            "GET",
            "/api/vault/keys",
            &[
                ("Host", &host),
                (NONCE_HEADER, &nonce),
                ("Origin", "http://evil.example"),
            ],
            b"",
        ),
        ui.send(
            "PUT",
            "/api/vault/keys/stripe",
            &[("Host", &host), (NONCE_HEADER, &nonce)],
            b"{}",
        ),
        ui.send(
            "DELETE",
            "/api/vault/keys/stripe",
            &[
                ("Host", &host),
                (NONCE_HEADER, &nonce),
                ("Origin", "http://127.0.0.1:1"),
            ],
            b"",
        ),
        ui.send("GET", "/api/status", &[("Host", &host)], b""),
        ui.send("GET", "/openapi.json", &[("Host", &host)], b""),
        ui.send("GET", "/mcp", &[("Host", &host)], b""),
        ui.send("GET", &format!("/?nonce={wrong}"), &[("Host", &host)], b""),
        ui.send("GET", "/vault?nonce=nope", &[("Host", &host)], b""),
        ui.send(
            "GET",
            &format!("/?nonce={nonce}"),
            &[("Host", "attacker.example")],
            b"",
        ),
    ];
    for (i, response) in refused.iter().enumerate() {
        assert_eq!(response.status, 403, "request {i}");
        assert_eq!(response.error(), "forbidden", "request {i}");
        assert_eq!(
            response.header("content-security-policy"),
            Some(crate::security::POLICY),
            "request {i}"
        );
    }
    assert!(admin.seen.try_recv().is_err());

    let read = ui.send(
        "GET",
        "/api/vault/keys",
        &[("Host", &host), (NONCE_HEADER, &nonce)],
        b"",
    );
    assert_eq!(read.status, 200);
    assert_eq!(admin.seen.recv().unwrap().target, "/keys");
    let write = ui.send(
        "PUT",
        "/api/vault/keys/stripe",
        &[("Host", &host), (NONCE_HEADER, &nonce), ("Origin", &origin)],
        b"{}",
    );
    assert_eq!(write.status, 200);
    assert_eq!(admin.seen.recv().unwrap().method, "PUT");
}

#[test]
fn every_response_carries_the_security_headers() {
    let dir = tempfile::tempdir().unwrap();
    let ui = ui_without_daemon(dir.path());
    for response in [
        ui.get(&format!("/?nonce={}", ui.nonce)),
        ui.api("GET", "/api/status", &[], ""),
        ui.api("GET", "/api/nothing", &[], ""),
        ui.get("/missing.js"),
    ] {
        let policy = response.header("content-security-policy").unwrap();
        assert!(policy.contains("default-src 'none'"));
        assert!(policy.contains("script-src 'self'"));
        assert!(policy.contains("frame-ancestors 'none'"));
        assert_eq!(response.header("cache-control"), Some("no-store"));
        assert_eq!(response.header("x-content-type-options"), Some("nosniff"));
        assert_eq!(response.header("referrer-policy"), Some("no-referrer"));
        assert_eq!(response.header("x-frame-options"), Some("DENY"));
        assert_eq!(
            response.header("cross-origin-resource-policy"),
            Some("same-origin")
        );
        assert!(response.header("access-control-allow-origin").is_none());
    }
}

#[test]
fn vault_requests_are_proxied_to_the_admin_socket() {
    let dir = tempfile::tempdir().unwrap();
    let admin = Admin::start(
        dir.path(),
        401,
        serde_json::json!({ "error": "presence_required" }),
    );
    let ui = Ui::start(dir.path(), &admin.path);
    let body = r#"{"description":"Payments","approved_urls":["https://api.stripe.com/v1/"],"inject":"bearer","secret":"sk_test"}"#;

    let first = ui.api("PUT", "/api/vault/keys/stripe", &[], body);
    assert_eq!(first.status, 401);
    assert_eq!(first.error(), "presence_required");
    let seen = admin.seen.recv().unwrap();
    assert_eq!(
        (seen.method.as_str(), seen.target.as_str()),
        ("PUT", "/keys/stripe")
    );
    assert_eq!(seen.body, body.as_bytes());
    assert_eq!(seen.passphrase, None);

    ui.api(
        "PUT",
        "/api/vault/keys/stripe",
        &[("X-Vault-Passphrase", "correct horse")],
        body,
    );
    assert_eq!(
        admin.seen.recv().unwrap().passphrase.as_deref(),
        Some("correct horse")
    );

    ui.api("GET", "/api/vault/audit?tail=50", &[], "");
    assert_eq!(admin.seen.recv().unwrap().target, "/audit?tail=50");
    ui.api(
        "POST",
        "/api/vault/approvals/a1",
        &[],
        r#"{"approve":true}"#,
    );
    let seen = admin.seen.recv().unwrap();
    assert_eq!(seen.target, "/approvals/a1");
    assert_eq!(seen.body, br#"{"approve":true}"#);

    for (method, target, body) in [
        ("GET", "/api/vault/keys/stripe/secret", ""),
        ("POST", "/api/vault/keys", "{}"),
        ("GET", "/api/vault/../keys", ""),
        ("GET", "/api/vault/keys/%2e%2e%2fx/../y", ""),
        ("GET", "/api/vault/status/", ""),
        ("GET", "/api/vault//status", ""),
    ] {
        let response = ui.api(method, target, &[], body);
        assert_eq!(response.status, 404, "{method} {target}");
    }
    assert_eq!(
        ui.api("POST", "/api/vault/tokens", &[], "not json").status,
        400
    );
    assert_eq!(
        ui.api("GET", "/api/vault/audit?tail=<script>", &[], "")
            .status,
        400
    );
    assert!(admin.seen.try_recv().is_err());
}

#[test]
fn a_missing_daemon_is_a_bad_gateway() {
    let dir = tempfile::tempdir().unwrap();
    let ui = ui_without_daemon(dir.path());
    let response = ui.api("GET", "/api/vault/status", &[], "");
    assert_eq!(response.status, 502);
    assert_eq!(response.error(), "daemon_unavailable");
}

#[test]
fn bodies_are_capped_at_four_mebibytes() {
    let dir = tempfile::tempdir().unwrap();
    let admin = Admin::start(dir.path(), 200, serde_json::json!({}));
    let ui = Ui::start(dir.path(), &admin.path);
    let big = format!("\"{}\"", "a".repeat(crate::security::BODY_LIMIT + 1));
    let response = ui.api("POST", "/api/vault/tokens", &[], &big);
    assert_eq!(response.status, 413);
    assert!(admin.seen.try_recv().is_err());
}

#[test]
fn status_says_what_this_build_serves() {
    let dir = tempfile::tempdir().unwrap();
    let ui = ui_without_daemon(dir.path());
    let response = ui.api("GET", "/api/status", &[], "");
    assert_eq!(response.status, 200);
    let status = response.json();
    assert_eq!(status["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(status["features"], serde_json::json!(crate::features()));
    assert_eq!(status["vault"]["reachable"], false);
    assert_eq!(status["setup"], "unknown");
}

#[test]
fn the_openapi_document_describes_the_routes() {
    let dir = tempfile::tempdir().unwrap();
    let ui = ui_without_daemon(dir.path());
    let response = ui.api("GET", "/openapi.json", &[], "");
    assert_eq!(response.status, 200);
    let document = response.json();
    assert!(document["openapi"].as_str().unwrap().starts_with("3.1"));
    assert_eq!(document["info"]["title"], "ai2 ui");
    assert_eq!(document["info"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(document["paths"]["/api/status"]["get"].is_object());
    assert!(document["components"]["schemas"]["Status"].is_object());
}

#[test]
fn the_bundle_is_served_with_the_page_for_every_route() {
    let dir = tempfile::tempdir().unwrap();
    let ui = ui_without_daemon(dir.path());
    let page = ui.get(&format!("/?nonce={}", ui.nonce));
    assert_eq!(page.status, 200);
    assert_eq!(
        page.header("content-type"),
        Some("text/html; charset=utf-8")
    );
    let html = page.text();
    assert!(html.contains("<div id=\"root\"></div>"));

    let script = html
        .split("src=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("a script in the page");
    assert!(script.starts_with("/assets/"));
    let served = ui.get(script);
    assert_eq!(served.status, 200);
    assert_eq!(
        served.header("content-type"),
        Some("text/javascript; charset=utf-8")
    );
    assert!(!served.body.is_empty());

    let style = html
        .split("href=\"")
        .filter_map(|rest| rest.split('"').next())
        .find(|href| href.ends_with(".css"))
        .expect("a stylesheet in the page");
    assert_eq!(
        ui.get(style).header("content-type"),
        Some("text/css; charset=utf-8")
    );

    for route in [
        "/",
        "/setup",
        "/vault?tab=keys",
        "/reviews/abc123",
        "/apps/workspot",
        "/harnesses",
    ] {
        let response = ui.get(route);
        assert_eq!(response.status, 200, "{route}");
        assert_eq!(response.text(), html, "{route}");
    }
    assert_eq!(ui.get("/assets/missing.js").status, 404);
    assert_eq!(ui.api("GET", "/api/nothing", &[], "").error(), "not_found");
    assert_eq!(ui.api("POST", "/setup", &[], "").status, 405);
}

#[test]
fn the_nonce_is_kept_private_and_stable_until_rotated() {
    let dir = tempfile::tempdir().unwrap();
    let first = nonce::load(dir.path()).unwrap();
    assert_eq!(first.len(), 64);
    let file = nonce::path(dir.path());
    let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    let folder = std::fs::metadata(file.parent().unwrap())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(folder, 0o700);
    assert_eq!(nonce::load(dir.path()).unwrap(), first);
    let rotated = nonce::rotate(dir.path()).unwrap();
    assert_ne!(rotated, first);
    assert_eq!(nonce::load(dir.path()).unwrap(), rotated);

    std::fs::write(&file, "not a nonce").unwrap();
    assert!(nonce::load(dir.path()).is_err());

    let ui = ui_without_daemon(dir.path());
    assert_ne!(ui.nonce, rotated);
    assert!(nonce::same(&rotated, &rotated));
    assert!(!nonce::same(&rotated, &first));
    assert!(!nonce::same("", &first));
}

#[test]
fn the_command_reads_its_port_and_prints_the_address() {
    let matches = crate::command()
        .try_get_matches_from(["ui", "--port", "7795", "--no-open", "--tab", "vault"])
        .unwrap();
    assert_eq!(matches.get_one::<u16>("port"), Some(&7795));
    assert!(matches.get_flag("no-open"));
    assert!(crate::command()
        .try_get_matches_from(["ui", "--tab", "nowhere"])
        .is_err());
    assert_eq!(
        crate::server::address(7797, "abc", Some("vault")),
        "http://127.0.0.1:7797/vault?nonce=abc"
    );
    assert_eq!(
        crate::server::address(7797, "abc", None),
        "http://127.0.0.1:7797/?nonce=abc"
    );
}

#[cfg(feature = "mcp")]
#[test]
fn mcp_is_served_under_the_nonce_with_its_openapi_document() {
    let dir = tempfile::tempdir().unwrap();
    let ui = ui_without_daemon(dir.path());
    assert_eq!(ui.get("/mcp/openapi.json").status, 403);
    let document = ui.api("GET", "/mcp/openapi.json", &[], "");
    assert_eq!(document.status, 200);
    let document = document.json();
    assert_eq!(
        document["servers"][0]["url"],
        format!("http://{}/mcp", ui.host)
    );
    assert!(document["paths"]["/tools/apps_list"].is_object());
    let refused = ui.send(
        "POST",
        "/mcp",
        &[("Host", &ui.host), ("Content-Type", "application/json")],
        b"{}",
    );
    assert_eq!(refused.status, 403);
    let initialize = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#;
    let answered = ui.api(
        "POST",
        "/mcp",
        &[
            ("Content-Type", "application/json"),
            ("Accept", "application/json, text/event-stream"),
        ],
        initialize,
    );
    assert_eq!(answered.status, 200, "{}", answered.text());
    let text = answered.text();
    let reply: serde_json::Value = text
        .lines()
        .find_map(|line| serde_json::from_str(line.trim_start_matches("data:").trim()).ok())
        .unwrap_or_else(|| panic!("no JSON-RPC reply in {text}"));
    assert_eq!(reply["result"]["serverInfo"]["name"], "ai2");
    let status = ui.api("GET", "/api/status", &[], "").json();
    assert!(status["features"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("mcp")));
}

struct Listener {
    port: u16,
    seen: Receiver<String>,
}

impl Listener {
    fn answering(status: u16, body: &'static str) -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (to, seen) = channel();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") {
                    match stream.read(&mut byte) {
                        Ok(1) => head.push(byte[0]),
                        _ => break,
                    }
                }
                if head.is_empty() {
                    continue;
                }
                let _ = to.send(String::from_utf8_lossy(&head).into_owned());
                let reply = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(reply.as_bytes());
            }
        });
        Self { port, seen }
    }
}

#[test]
fn a_dashboard_already_listening_is_reused_with_its_nonce() {
    let home = tempfile::tempdir().unwrap();
    let nonce = nonce::load(home.path()).unwrap();
    let dashboard = Listener::answering(200, r#"{"version":"0.1.2","features":["apps"]}"#);
    let url = crate::service::running(home.path(), dashboard.port, None).unwrap();
    assert_eq!(
        url,
        format!("http://127.0.0.1:{}/?nonce={nonce}", dashboard.port)
    );
    let asked = dashboard.seen.recv().unwrap().to_ascii_lowercase();
    assert!(asked.starts_with("get /api/status "), "{asked}");
    assert!(
        asked.contains(&format!("x-ai2-ui-nonce: {nonce}")),
        "{asked}"
    );
    assert_eq!(
        crate::service::running(home.path(), dashboard.port, Some("vault")).unwrap(),
        format!("http://127.0.0.1:{}/vault?nonce={nonce}", dashboard.port)
    );
}

#[test]
fn something_else_on_the_port_or_nothing_at_all_is_not_reused() {
    let home = tempfile::tempdir().unwrap();
    nonce::load(home.path()).unwrap();
    let refusing = Listener::answering(403, r#"{"error":"forbidden"}"#);
    assert!(crate::service::running(home.path(), refusing.port, None).is_none());
    let stranger = Listener::answering(200, r#"{"hello":"world"}"#);
    assert!(crate::service::running(home.path(), stranger.port, None).is_none());
    let free = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = free.local_addr().unwrap().port();
    drop(free);
    assert!(crate::service::running(home.path(), port, None).is_none());
    assert!(crate::service::running(home.path(), 0, None).is_none());
    let fresh = tempfile::tempdir().unwrap();
    let dashboard = Listener::answering(200, r#"{"version":"0.1.2","features":["apps"]}"#);
    assert!(crate::service::running(fresh.path(), dashboard.port, None).is_none());
    assert!(!nonce::path(fresh.path()).exists());
}

#[test]
fn the_skill_is_served_without_the_nonce_with_the_security_headers() {
    let dir = tempfile::tempdir().unwrap();
    let ui = ui_without_daemon(dir.path());
    let served = ui.get("/skill.md");
    assert_eq!(served.status, 200);
    assert_eq!(served.text(), harness::skill::AI2);
    assert_eq!(
        served.header("content-type"),
        Some("text/markdown; charset=utf-8")
    );
    assert_eq!(served.header("x-content-type-options"), Some("nosniff"));
    assert_eq!(
        served.header("content-security-policy"),
        Some(crate::security::POLICY)
    );
    let refused = ui.send("GET", "/skill.md", &[("Host", "evil.example:1")], b"");
    assert_eq!(refused.status, 403);
}

#[cfg(feature = "mcp")]
fn initialize(ui: &Ui, headers: &[(&str, &str)]) -> Reply {
    let body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"cursor","version":"1.2"}}}"#;
    let mut all = vec![
        ("Host", ui.host.as_str()),
        ("Content-Type", "application/json"),
        ("Accept", "application/json, text/event-stream"),
    ];
    all.extend_from_slice(headers);
    ui.send("POST", "/mcp", &all, body.as_bytes())
}

#[cfg(feature = "mcp")]
#[test]
fn an_http_client_pairs_once_with_a_code_and_its_token_opens_mcp_until_revoked() {
    let dir = tempfile::tempdir().unwrap();
    let ui = ui_without_daemon(dir.path());
    let pair = |code: &str| {
        ui.send(
            "POST",
            "/mcp/pair",
            &[("Host", &ui.host), ("Content-Type", "application/json")],
            serde_json::json!({ "code": code, "name": "Cursor" })
                .to_string()
                .as_bytes(),
        )
    };
    let empty = ui.api("GET", "/api/mcp/clients", &[], "").json();
    assert_eq!(empty["clients"], serde_json::json!([]));
    assert_eq!(empty["pairing"], serde_json::Value::Null);
    assert_eq!(empty["url"], format!("http://{}/mcp", ui.host));
    assert_eq!(pair("ABCDEFGH").status, 403);
    assert_eq!(
        ui.send("POST", "/api/mcp/pairing", &[("Host", &ui.host)], b"")
            .status,
        403
    );

    let code = ui.api("POST", "/api/mcp/pairing", &[], "").json();
    let code = code["code"].as_str().unwrap().to_string();
    assert_eq!(code.len(), 8);
    let listed = ui.api("GET", "/api/mcp/clients", &[], "").json();
    assert_eq!(listed["pairing"]["code"], code);
    let foreign = ui.send(
        "POST",
        "/mcp/pair",
        &[
            ("Host", &ui.host),
            ("Origin", "http://evil.example"),
            ("Content-Type", "application/json"),
        ],
        serde_json::json!({ "code": code }).to_string().as_bytes(),
    );
    assert_eq!(foreign.status, 403);

    let paired = pair(&code);
    assert_eq!(paired.status, 200, "{}", paired.text());
    let paired = paired.json();
    let token = paired["token"].as_str().unwrap().to_string();
    assert!(token.starts_with("ai2_"));
    assert_eq!(paired["header"], format!("Authorization: Bearer {token}"));
    assert_eq!(pair(&code).status, 403);
    let stored = std::fs::read_to_string(dir.path().join("mcp/clients.json")).unwrap();
    assert!(!stored.contains(&token));

    assert_eq!(initialize(&ui, &[]).status, 403);
    assert_eq!(
        initialize(&ui, &[("Authorization", "Bearer ai2_not_a_token")]).status,
        403
    );
    let bearer = format!("Bearer {token}");
    let answered = initialize(&ui, &[("Authorization", &bearer)]);
    assert_eq!(answered.status, 200, "{}", answered.text());
    assert!(answered.text().contains("\"serverInfo\""));

    let listed = ui.api("GET", "/api/mcp/clients", &[], "").json();
    assert_eq!(listed["pairing"], serde_json::Value::Null);
    assert_eq!(listed["clients"][0]["name"], "cursor");
    assert_eq!(listed["clients"][0]["transport"], "http");
    assert_eq!(listed["clients"][0]["token"], paired["id"]);
    assert_eq!(listed["tokens"][0]["id"], paired["id"]);
    assert_eq!(listed["tokens"][0]["name"], "Cursor");
    assert!(listed["tokens"][0].get("hash").is_none());
    assert!(!listed.to_string().contains(&token));

    let id = paired["id"].as_str().unwrap();
    let revoked = ui.api("DELETE", &format!("/api/mcp/tokens/{id}"), &[], "");
    assert_eq!(revoked.status, 200);
    assert_eq!(
        ui.api("DELETE", &format!("/api/mcp/tokens/{id}"), &[], "")
            .status,
        404
    );
    assert_eq!(initialize(&ui, &[("Authorization", &bearer)]).status, 403);
    let listed = ui.api("GET", "/api/mcp/clients", &[], "").json();
    assert_eq!(listed["tokens"], serde_json::json!([]));
}
