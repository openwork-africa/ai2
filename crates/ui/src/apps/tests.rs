use crate::{Options, Server, NONCE_HEADER};
use base64::Engine;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::time::{Duration, Instant};

const PNG: &[u8] =
    b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0\x1f\x15\xc4\x89";

const APP: &str = r#"
import http.server, json, os
port = int(os.environ["PORT"])
doc = {"openapi": "3.1.0", "info": {"title": "Demo", "version": "1.2.0"},
       "paths": {"/hello": {"get": {"summary": "Say hello"}}, "/items": {"get": {}, "post": {}}}}
class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        body = json.dumps(doc).encode() if self.path == "/openapi.json" else b"hello"
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def log_message(self, format, *args):
        print("request " + self.path, flush=True)
print("listening on " + str(port), flush=True)
http.server.HTTPServer(("127.0.0.1", port), Handler).serve_forever()
"#;

struct Reply {
    status: u16,
    head: String,
    body: Vec<u8>,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }

    fn error(&self) -> String {
        self.json()["error"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }
}

struct Ui {
    host: String,
    nonce: String,
    runtime: Option<tokio::runtime::Runtime>,
}

impl Ui {
    fn start(home: &Path) -> Ui {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let options = Options {
            home: home.to_path_buf(),
            admin: home.join("admin.sock"),
        };
        let server = runtime.block_on(Server::bind(0, &options)).unwrap();
        let state = server.state();
        runtime.spawn(server.serve());
        Ui {
            host: state.host.clone(),
            nonce: state.nonce.clone(),
            runtime: Some(runtime),
        }
    }

    fn head(&self, method: &str, target: &str, kind: &str, length: usize) -> String {
        format!(
            "{method} {target} HTTP/1.1\r\nHost: {host}\r\nOrigin: http://{host}\r\n{NONCE_HEADER}: {nonce}\r\nConnection: close\r\nContent-Type: {kind}\r\nContent-Length: {length}\r\n\r\n",
            host = self.host,
            nonce = self.nonce,
        )
    }

    fn send(&self, method: &str, target: &str, kind: &str, body: &[u8]) -> Reply {
        let mut stream = TcpStream::connect(&self.host).unwrap();
        stream
            .write_all(self.head(method, target, kind, body.len()).as_bytes())
            .unwrap();
        let _ = stream.write_all(body);
        let mut raw = Vec::new();
        let _ = stream.read_to_end(&mut raw);
        parse(&raw)
    }

    fn call(&self, method: &str, target: &str, body: Option<Value>) -> Reply {
        let body = body.map(|b| b.to_string()).unwrap_or_default();
        self.send(method, target, "application/json", body.as_bytes())
    }

    fn stream_until(&self, target: &str, wanted: &str) -> String {
        let mut stream = TcpStream::connect(&self.host).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        stream
            .write_all(self.head("GET", target, "text/plain", 0).as_bytes())
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut seen = Vec::new();
        let mut buffer = [0u8; 4096];
        while Instant::now() < deadline {
            match stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => seen.extend_from_slice(&buffer[..n]),
                Err(_) => {}
            }
            if String::from_utf8_lossy(&seen).contains(wanted) {
                break;
            }
        }
        String::from_utf8_lossy(&seen).into_owned()
    }
}

impl Drop for Ui {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

struct Stopping<'a> {
    ui: &'a Ui,
    id: String,
}

impl Drop for Stopping<'_> {
    fn drop(&mut self) {
        self.ui
            .call("POST", &format!("/api/apps/{}/stop", self.id), None);
    }
}

fn parse(raw: &[u8]) -> Reply {
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("a complete response");
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let status = head
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap();
    let mut body = raw[split + 4..].to_vec();
    if head.to_lowercase().contains("transfer-encoding: chunked") {
        body = unchunk(&body);
    }
    Reply { status, head, body }
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

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn python() -> bool {
    std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success())
}

fn png() -> Value {
    json!({ "type": "image/png", "base64": base64::engine::general_purpose::STANDARD.encode(PNG) })
}

fn demo(dir: &Path, name: &str, port: u16, restart: &str) -> Value {
    std::fs::write(dir.join("app.py"), APP).unwrap();
    json!({
        "name": name,
        "description": "A tiny python server for the tests",
        "command": "python3 -u app.py",
        "openapi": "/openapi.json",
        "cwd": dir.display().to_string(),
        "port": port,
        "restart": restart,
        "icon": png(),
    })
}

#[test]
fn register_refuses_an_app_without_openapi() {
    let home = tempfile::tempdir().unwrap();
    let ui = Ui::start(home.path());
    let mut app = demo(home.path(), "No API", free_port(), "never");
    app.as_object_mut().unwrap().remove("openapi");
    let reply = ui.call("POST", "/api/apps", Some(app));
    assert_eq!(reply.status, 422);
    assert_eq!(reply.error(), "openapi_required");
}

#[test]
fn register_checks_the_icon() {
    let home = tempfile::tempdir().unwrap();
    let ui = Ui::start(home.path());
    let port = free_port();

    let mut big = PNG.to_vec();
    big.resize(600 * 1024, 0);
    let mut app = demo(home.path(), "Big", port, "never");
    app["icon"] = json!({ "type": "image/png", "base64": base64::engine::general_purpose::STANDARD.encode(&big) });
    let reply = ui.call("POST", "/api/apps", Some(app));
    assert_eq!(reply.status, 413);
    assert_eq!(reply.error(), "icon_too_large");

    let mut app = demo(home.path(), "Script", port, "never");
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#;
    app["icon"] = json!({ "type": "image/svg+xml", "base64": base64::engine::general_purpose::STANDARD.encode(svg) });
    let reply = ui.call("POST", "/api/apps", Some(app));
    assert_eq!(reply.status, 400);
    assert_eq!(reply.error(), "icon_unsafe");

    let mut app = demo(home.path(), "Path", port, "never");
    app["icon"] = json!({ "path": "/etc/hosts" });
    let reply = ui.call("POST", "/api/apps", Some(app));
    assert_eq!(reply.status, 400);
    assert_eq!(reply.error(), "icon_path");

    let reply = ui.call("GET", "/api/apps", None);
    assert_eq!(reply.json()["apps"], json!([]));
}

#[test]
fn register_takes_multipart_with_an_icon_file() {
    let home = tempfile::tempdir().unwrap();
    let ui = Ui::start(home.path());
    let mut app = demo(home.path(), "Form", free_port(), "never");
    app.as_object_mut().unwrap().remove("icon");
    let boundary = "ai2boundary";
    let mut body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"app\"\r\nContent-Type: application/json\r\n\r\n{app}\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"icon\"; filename=\"icon.png\"\r\nContent-Type: image/png\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(PNG);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let reply = ui.send(
        "POST",
        "/api/apps",
        &format!("multipart/form-data; boundary={boundary}"),
        &body,
    );
    assert_eq!(
        reply.status,
        201,
        "{}",
        String::from_utf8_lossy(&reply.body)
    );
    let shown = &reply.json()["app"];
    assert_eq!(shown["id"], "form");
    assert_eq!(shown["icon"]["type"], "image/png");
    assert_eq!(shown["running"], false);
    assert_eq!(shown["api"]["state"], "unchecked");
    assert_eq!(shown["api"]["href"], "/api/apps/form/openapi.json");

    let icon = ui.call("GET", "/api/apps/form/icon", None);
    assert_eq!(icon.status, 200);
    assert!(icon.head.to_lowercase().contains("content-type: image/png"));
    assert_eq!(icon.body, PNG);

    let document = ui.call("GET", "/api/apps/form/openapi.json", None);
    assert_eq!(document.status, 404);
    assert_eq!(document.error(), "openapi_unchecked");
    assert!(document.json()["message"]
        .as_str()
        .unwrap()
        .contains("/openapi.json"));

    let again = ui.call(
        "POST",
        "/api/apps",
        Some(demo(home.path(), "Form", free_port(), "never")),
    );
    assert_eq!(again.status, 200);

    let patched = ui.call(
        "PATCH",
        "/api/apps/form",
        Some(json!({ "description": "Changed" })),
    );
    assert_eq!(patched.status, 200);
    assert_eq!(patched.json()["app"]["description"], "Changed");

    let refused = ui.call("POST", "/api/apps/form/stop", None);
    assert_eq!(refused.status, 409);
    assert_eq!(refused.error(), "not_ours");

    assert_eq!(ui.call("DELETE", "/api/apps/form", None).status, 204);
    assert_eq!(ui.call("GET", "/api/apps/form", None).status, 404);
}

#[test]
fn an_app_starts_serves_its_openapi_streams_logs_and_stops() {
    if !python() {
        eprintln!("skipped: no python3");
        return;
    }
    let home = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let ui = Ui::start(home.path());
    let port = free_port();
    let reply = ui.call(
        "POST",
        "/api/apps",
        Some(demo(dir.path(), "Demo", port, "on-failure")),
    );
    assert_eq!(
        reply.status,
        201,
        "{}",
        String::from_utf8_lossy(&reply.body)
    );

    let listed = ui.call("GET", "/api/apps", None).json();
    assert_eq!(listed["apps"][0]["running"], false);
    assert_eq!(listed["apps"][0]["api"]["state"], "unchecked");

    let started = ui.call("POST", "/api/apps/demo/start", None);
    let _stopping = Stopping {
        ui: &ui,
        id: "demo".to_string(),
    };
    assert_eq!(
        started.status,
        200,
        "{}",
        String::from_utf8_lossy(&started.body)
    );
    let app = &started.json()["app"];
    assert_eq!(app["running"], true);
    assert_eq!(app["started_here"], true);
    assert_eq!(app["api"]["state"], "valid");
    assert_eq!(app["api"]["operations"], 3);
    assert_eq!(app["api"]["title"], "Demo");

    let again = ui.call("POST", "/api/apps/demo/start", None);
    assert_eq!(again.status, 409);
    assert_eq!(again.error(), "already_running");

    let document = ui.call("GET", "/api/apps/demo/openapi.json", None);
    assert_eq!(document.status, 200);
    assert_eq!(document.json()["info"]["version"], "1.2.0");

    let inline = ui.call("GET", "/api/apps?openapi=1", None).json();
    assert_eq!(inline["apps"][0]["document"]["openapi"], "3.1.0");
    let plain = ui.call("GET", "/api/apps", None).json();
    assert!(plain["apps"][0].get("document").is_none());

    let checked = ui.call("POST", "/api/apps/demo/check", None);
    assert_eq!(checked.json()["api"]["state"], "valid");

    let streamed = ui.stream_until("/api/apps/demo/logs/events", "listening on");
    assert!(streamed.contains("event: log"), "{streamed}");
    let data = streamed
        .lines()
        .find_map(|line| line.strip_prefix("data:"))
        .expect("a data line");
    let line: apps::LogLine = serde_json::from_str(data.trim()).unwrap();
    assert!(line.line.contains("listening on"));

    let tail = ui.call("GET", "/api/apps/demo/logs?tail=50", None).json();
    assert!(tail["lines"].as_array().unwrap().iter().any(|l| l["line"]
        .as_str()
        .unwrap_or_default()
        .contains("listening on")));

    let stopped = ui.call("POST", "/api/apps/demo/stop", None);
    assert_eq!(stopped.status, 200);
    assert_eq!(stopped.json()["app"]["running"], false);
    assert_eq!(stopped.json()["app"]["started_here"], false);

    assert_eq!(ui.call("DELETE", "/api/apps/demo", None).status, 204);
}

#[test]
fn the_document_lists_the_apps_routes() {
    let home = tempfile::tempdir().unwrap();
    let ui = Ui::start(home.path());
    let document = ui.call("GET", "/openapi.json", None).json();
    let paths = document["paths"].as_object().unwrap();
    for path in [
        "/api/apps",
        "/api/apps/events",
        "/api/apps/{id}",
        "/api/apps/{id}/openapi.json",
        "/api/apps/{id}/check",
        "/api/apps/{id}/icon",
        "/api/apps/{id}/start",
        "/api/apps/{id}/stop",
        "/api/apps/{id}/restart",
        "/api/apps/{id}/logs",
        "/api/apps/{id}/logs/events",
    ] {
        assert!(paths.contains_key(path), "{path} is missing");
    }
    assert!(paths["/api/apps/{id}"].get("patch").is_some());
    assert!(paths["/api/apps/{id}"].get("delete").is_some());
}
