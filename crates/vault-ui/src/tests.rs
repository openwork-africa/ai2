use crate::server::{with_assets, Server, NONCE_HEADER};
use std::io::Write;
use std::net::TcpStream;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use vault::http::{self, Request, Response};

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
                Response::json(status, &reply)
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

struct Ui {
    host: String,
    nonce: String,
}

impl Ui {
    fn start(admin: &Path, report: Option<PathBuf>) -> Self {
        let server = Server::bind(admin, report).unwrap();
        let url = server.url();
        let (origin, nonce) = url.trim_end().split_once("/?nonce=").unwrap();
        let host = origin.strip_prefix("http://").unwrap().to_string();
        let nonce = nonce.to_string();
        thread::spawn(move || server.serve().unwrap());
        Self { host, nonce }
    }

    fn origin(&self) -> String {
        format!("http://{}", self.host)
    }

    fn send(&self, method: &str, target: &str, headers: &[(&str, &str)], body: &str) -> Response {
        let mut stream = TcpStream::connect(&self.host).unwrap();
        let mut head = format!(
            "{method} {target} HTTP/1.1\r\nContent-Length: {}\r\n",
            body.len()
        );
        for (name, value) in headers {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        head.push_str("\r\n");
        stream.write_all(head.as_bytes()).unwrap();
        stream.write_all(body.as_bytes()).unwrap();
        http::read_response(&mut stream).unwrap()
    }

    fn api(&self, method: &str, target: &str, extra: &[(&str, &str)], body: &str) -> Response {
        let origin = self.origin();
        let mut headers = vec![
            ("Host", self.host.as_str()),
            (NONCE_HEADER, self.nonce.as_str()),
            ("Origin", origin.as_str()),
        ];
        headers.extend_from_slice(extra);
        self.send(method, target, &headers, body)
    }
}

fn error(response: &Response) -> String {
    let value: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
    value["error"].as_str().unwrap_or_default().to_string()
}

#[test]
fn api_requests_need_the_nonce_the_host_and_for_writes_the_origin() {
    let dir = tempfile::tempdir().unwrap();
    let admin = Admin::start(dir.path(), 200, serde_json::json!({ "keys": [] }));
    let ui = Ui::start(&admin.path, None);
    let host = ui.host.clone();
    let origin = ui.origin();
    let nonce = ui.nonce.clone();

    let refused = [
        ui.send("GET", "/api/keys", &[("Host", &host)], ""),
        ui.send(
            "GET",
            "/api/keys",
            &[("Host", &host), (NONCE_HEADER, "0".repeat(64).as_str())],
            "",
        ),
        ui.send(
            "GET",
            "/api/keys",
            &[("Host", "localhost.evil:1"), (NONCE_HEADER, &nonce)],
            "",
        ),
        ui.send("GET", "/api/keys", &[(NONCE_HEADER, &nonce)], ""),
        ui.send(
            "GET",
            "/api/keys",
            &[("Host", &host), ("Host", &host), (NONCE_HEADER, &nonce)],
            "",
        ),
        ui.send(
            "GET",
            "/api/keys",
            &[
                ("Host", &host),
                (NONCE_HEADER, &nonce),
                ("Origin", "http://evil.example"),
            ],
            "",
        ),
        ui.send(
            "PUT",
            "/api/keys/stripe",
            &[("Host", &host), (NONCE_HEADER, &nonce)],
            "{}",
        ),
        ui.send(
            "DELETE",
            "/api/keys/stripe",
            &[
                ("Host", &host),
                (NONCE_HEADER, &nonce),
                ("Origin", "http://127.0.0.1:1"),
            ],
            "",
        ),
        ui.send("GET", "/", &[("Host", &host)], ""),
        ui.send("GET", "/?nonce=nope", &[("Host", &host)], ""),
    ];
    for (i, response) in refused.iter().enumerate() {
        assert_eq!(response.status, 403, "request {i}");
        assert_eq!(error(response), "forbidden", "request {i}");
    }
    assert!(admin.seen.try_recv().is_err());

    let read = ui.send(
        "GET",
        "/api/keys",
        &[("Host", &host), (NONCE_HEADER, &nonce)],
        "",
    );
    assert_eq!(read.status, 200);
    assert_eq!(admin.seen.recv().unwrap().target, "/keys");
    let write = ui.send(
        "PUT",
        "/api/keys/stripe",
        &[("Host", &host), (NONCE_HEADER, &nonce), ("Origin", &origin)],
        "{}",
    );
    assert_eq!(write.status, 200);
    assert_eq!(admin.seen.recv().unwrap().method, "PUT");

    let page = ui.send("GET", &format!("/?nonce={nonce}"), &[("Host", &host)], "");
    assert_eq!(page.status, 200);
    assert!(page
        .header("content-security-policy")
        .is_some_and(|p| p.contains("script-src 'self'")));
    assert!(String::from_utf8_lossy(&page.body).contains("/vault.js"));
}

#[test]
fn api_requests_are_proxied_to_the_admin_socket() {
    let dir = tempfile::tempdir().unwrap();
    let admin = Admin::start(
        dir.path(),
        401,
        serde_json::json!({ "error": "presence_required" }),
    );
    let ui = Ui::start(&admin.path, None);
    let body = r#"{"description":"Payments","approved_urls":["https://api.stripe.com/v1/"],"inject":"bearer","secret":"sk_test"}"#;

    let first = ui.api("PUT", "/api/keys/stripe", &[], body);
    assert_eq!(first.status, 401);
    assert_eq!(error(&first), "presence_required");
    let seen = admin.seen.recv().unwrap();
    assert_eq!(
        (seen.method.as_str(), seen.target.as_str()),
        ("PUT", "/keys/stripe")
    );
    assert_eq!(seen.body, body.as_bytes());
    assert_eq!(seen.passphrase, None);

    ui.api(
        "PUT",
        "/api/keys/stripe",
        &[("X-Vault-Passphrase", "correct horse")],
        body,
    );
    assert_eq!(
        admin.seen.recv().unwrap().passphrase.as_deref(),
        Some("correct horse")
    );

    ui.api("GET", "/api/audit?tail=50", &[], "");
    assert_eq!(admin.seen.recv().unwrap().target, "/audit?tail=50");
    ui.api("POST", "/api/approvals/a1", &[], r#"{"approve":true}"#);
    let seen = admin.seen.recv().unwrap();
    assert_eq!(seen.target, "/approvals/a1");
    assert_eq!(seen.body, br#"{"approve":true}"#);

    for (method, target, body) in [
        ("GET", "/api/keys/stripe/secret", ""),
        ("POST", "/api/keys", "{}"),
        ("GET", "/api/../keys", ""),
        ("GET", "/api/keys/%2e%2e%2fx/../y", ""),
        ("GET", "/api/status/", ""),
    ] {
        let response = ui.api(method, target, &[], body);
        assert_eq!(response.status, 404, "{method} {target}");
    }
    assert_eq!(ui.api("POST", "/api/tokens", &[], "not json").status, 400);
    assert!(admin.seen.try_recv().is_err());
}

#[test]
fn a_missing_daemon_is_a_bad_gateway() {
    let dir = tempfile::tempdir().unwrap();
    let ui = Ui::start(&dir.path().join("admin.sock"), None);
    let response = ui.api("GET", "/api/status", &[], "");
    assert_eq!(response.status, 502);
    assert_eq!(error(&response), "daemon_unavailable");
}

#[test]
fn a_report_is_served_with_the_vault_page_code() {
    let dir = tempfile::tempdir().unwrap();
    let report = dir.path().join("report.html");
    std::fs::write(
        &report,
        "<!doctype html><html><head><title>r</title></head><body><main><section id=\"vault\">Run ai2 vault ui</section></main></body></html>",
    )
    .unwrap();
    let ui = Ui::start(&dir.path().join("admin.sock"), Some(report));
    let page = ui.send(
        "GET",
        &format!("/?nonce={}", ui.nonce),
        &[("Host", &ui.host)],
        "",
    );
    let html = String::from_utf8(page.body).unwrap();
    assert!(html.contains("<script src=\"/vault.js\" defer></script></head>"));
    assert!(html.contains("id=\"vault\""));
    let script = ui.send("GET", "/vault.js", &[("Host", &ui.host)], "");
    assert_eq!(script.status, 200);
    assert!(String::from_utf8_lossy(&script.body).contains("X-Vault-UI-Nonce"));
    assert!(with_assets("<p>no head</p>").starts_with("<link"));
}

#[test]
fn the_page_keeps_no_secrets_in_storage() {
    let script = include_str!("vault.js");
    assert!(!script.contains("localStorage"));
    assert!(!script.contains("sessionStorage"));
    assert!(script.contains("Confirm with Touch ID on this Mac"));
    assert!(script.contains("X-Vault-Passphrase"));
}
