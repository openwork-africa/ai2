use crate::admin;
use anyhow::{Context, Result};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use vault::http::{self, Request, Response};

pub const NONCE_HEADER: &str = "X-Vault-UI-Nonce";

const PAGE: &str = include_str!("page.html");
const SCRIPT: &str = include_str!("vault.js");
const STYLE: &str = include_str!("vault.css");
const ASSETS: &str =
    "<link rel=\"stylesheet\" href=\"/vault.css\"><script src=\"/vault.js\" defer></script>";
const POLICY: &str = "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

pub struct Server {
    listener: TcpListener,
    state: Arc<State>,
}

struct State {
    nonce: String,
    host: String,
    origin: String,
    admin: PathBuf,
    report: Option<PathBuf>,
}

impl Server {
    pub fn bind(admin: &Path, report: Option<PathBuf>) -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").context("listening on 127.0.0.1")?;
        let addr = listener.local_addr()?;
        let host = format!("127.0.0.1:{}", addr.port());
        Ok(Self {
            listener,
            state: Arc::new(State {
                nonce: vault::crypto::hex(&vault::crypto::random::<32>()?),
                origin: format!("http://{host}"),
                host,
                admin: admin.to_path_buf(),
                report,
            }),
        })
    }

    pub fn url(&self) -> String {
        format!("{}/?nonce={}", self.state.origin, self.state.nonce)
    }

    pub fn serve(&self) -> Result<()> {
        for stream in self.listener.incoming() {
            let Ok(stream) = stream else { continue };
            let state = self.state.clone();
            thread::spawn(move || handle(stream, &state));
        }
        Ok(())
    }
}

fn handle(mut stream: TcpStream, state: &State) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let response = match http::read_request(&mut stream) {
        Ok(request) => respond(state, &request),
        Err(error) => Response::error(400, "bad_request", &error.to_string()),
    };
    let _ = secure(response).write_to(&mut stream);
}

fn secure(response: Response) -> Response {
    response
        .with("Cache-Control", "no-store")
        .with("X-Content-Type-Options", "nosniff")
        .with("Referrer-Policy", "no-referrer")
        .with("X-Frame-Options", "DENY")
        .with("Cross-Origin-Resource-Policy", "same-origin")
}

fn respond(state: &State, request: &Request) -> Response {
    if request.single("host") != Some(state.host.as_str()) {
        return forbidden("The Host header does not match this server.");
    }
    if request
        .header("origin")
        .is_some_and(|origin| origin != state.origin)
    {
        return forbidden("The Origin does not match this server.");
    }
    let path = request.path();
    if let Some(rest) = path.strip_prefix("/api/") {
        return api(state, request, rest);
    }
    match (request.method.as_str(), path) {
        ("GET", "/") => {
            let given = request
                .query()
                .unwrap_or_default()
                .split('&')
                .find_map(|pair| pair.strip_prefix("nonce="));
            if !given.is_some_and(|given| same(given, &state.nonce)) {
                return forbidden("Open the link ai2 vault ui printed.");
            }
            page(state)
        }
        ("GET", "/vault.js") => Response::new(200, "text/javascript; charset=utf-8", SCRIPT),
        ("GET", "/vault.css") => Response::new(200, "text/css; charset=utf-8", STYLE),
        _ => Response::error(404, "not_found", "There is nothing here."),
    }
}

fn page(state: &State) -> Response {
    let html = match &state.report {
        Some(report) => match std::fs::read_to_string(report) {
            Ok(html) => with_assets(&html),
            Err(error) => {
                return Response::error(
                    404,
                    "report_missing",
                    &format!("Could not read {}: {error}", report.display()),
                )
            }
        },
        None => PAGE.to_string(),
    };
    Response::new(200, "text/html; charset=utf-8", html).with("Content-Security-Policy", POLICY)
}

pub fn with_assets(html: &str) -> String {
    match html.find("</head>") {
        Some(at) => format!("{}{ASSETS}{}", &html[..at], &html[at..]),
        None => format!("{ASSETS}{html}"),
    }
}

fn api(state: &State, request: &Request, rest: &str) -> Response {
    let nonce = request.single(NONCE_HEADER).unwrap_or_default();
    if !same(nonce, &state.nonce) {
        return forbidden("The request does not carry this page's nonce.");
    }
    let method = request.method.as_str();
    let write = !matches!(method, "GET" | "HEAD");
    if write && request.single("origin") != Some(state.origin.as_str()) {
        return forbidden("A change must come from this page.");
    }
    if !allowed(method, rest) {
        return Response::error(404, "not_found", "The vault has no such action.");
    }
    let query = request.query().filter(|q| !q.is_empty());
    if query.is_some_and(|q| !q.bytes().all(safe_byte)) {
        return Response::error(400, "bad_request", "The query has characters it may not.");
    }
    if !request.body.is_empty()
        && serde_json::from_slice::<serde_json::Value>(&request.body).is_err()
    {
        return Response::error(400, "bad_request", "The body is not JSON.");
    }
    let passphrase = request.single("x-vault-passphrase");
    if passphrase.is_some_and(|p| p.chars().any(char::is_control)) {
        return Response::error(
            400,
            "bad_request",
            "The passphrase has characters it may not.",
        );
    }
    let target = match query {
        Some(query) => format!("/{rest}?{query}"),
        None => format!("/{rest}"),
    };
    match admin::send(&state.admin, method, &target, &request.body, passphrase) {
        Ok(reply) => {
            let kind = reply
                .header("content-type")
                .unwrap_or("application/json")
                .to_string();
            Response::new(reply.status, &kind, reply.body)
        }
        Err(error) => Response::error(
            502,
            "daemon_unavailable",
            &format!(
                "The vault daemon did not answer at {}: {error:#}. Start it with ai2 setup.",
                state.admin.display()
            ),
        ),
    }
}

fn allowed(method: &str, path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.iter().skip(1).any(|part| !segment(part)) {
        return false;
    }
    matches!(
        (method, parts.as_slice()),
        ("GET", ["status"])
            | ("GET", ["keys"])
            | ("GET" | "PUT" | "DELETE", ["keys", _])
            | ("POST", ["keys", _, "rotate" | "propose"])
            | ("GET" | "POST", ["tokens"])
            | ("DELETE", ["tokens", _])
            | ("GET" | "POST", ["sandboxes"])
            | ("DELETE", ["sandboxes", _])
            | ("GET", ["approvals"])
            | ("POST", ["approvals", _])
            | ("GET", ["audit"])
    )
}

fn segment(part: &str) -> bool {
    !part.is_empty() && part != "." && part != ".." && part.bytes().all(safe_byte)
}

fn safe_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"-._~%=&".contains(&byte)
}

fn same(given: &str, nonce: &str) -> bool {
    given.len() == nonce.len()
        && given
            .bytes()
            .zip(nonce.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

fn forbidden(message: &str) -> Response {
    Response::error(403, "forbidden", message)
}
