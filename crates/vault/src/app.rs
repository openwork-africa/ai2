use crate::http::{self, Response};
use crate::policy;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const FOLDER: &str = "gateway";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppToken {
    pub id: String,
    pub token: String,
    pub revoke: String,
    pub socket: PathBuf,
}

pub fn file(home: &Path, key: &str) -> PathBuf {
    let valid = !key.is_empty()
        && key.len() <= 64
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
        && !key.starts_with('.');
    let name = if valid {
        key.to_string()
    } else {
        crate::crypto::hash(key)
    };
    home.join(FOLDER).join(format!("{name}.json"))
}

pub fn load(home: &Path, key: &str) -> Option<AppToken> {
    let text = std::fs::read_to_string(file(home, key)).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn save(home: &Path, key: &str, token: &AppToken) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
    let path = file(home, key);
    let dir = path.parent().unwrap_or(home);
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .with_context(|| format!("making {}", dir.display()))?;
    let temp = path.with_extension("new");
    let mut out = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&temp)
        .with_context(|| format!("writing {}", temp.display()))?;
    out.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    out.write_all(serde_json::to_string_pretty(token)?.as_bytes())?;
    std::fs::rename(&temp, &path).with_context(|| format!("writing {}", path.display()))
}

pub fn prefix(base_url: &str) -> Result<String> {
    let url = policy::parse(base_url.trim(), false)?;
    if url.rest.contains('?') {
        return Err(anyhow!("{base_url} has a query"));
    }
    let path = url.rest.trim_end_matches('/');
    Ok(format!("https://{}{path}/", url.authority))
}

pub fn route(base_url: &str, key: &str) -> Result<String> {
    let prefix = prefix(base_url)?;
    let rest = prefix.trim_start_matches("https://").trim_end_matches('/');
    Ok(format!("/{key}/{rest}"))
}

pub fn target(url: &str, key: &str) -> Result<String> {
    let parsed = policy::parse(url.trim(), false)?;
    let rest = if parsed.rest.is_empty() {
        "/"
    } else {
        parsed.rest.as_str()
    };
    Ok(format!("/{key}/{}{rest}", parsed.authority))
}

pub struct Call<'a> {
    pub method: &'a str,
    pub url: &'a str,
    pub headers: &'a [(&'a str, &'a str)],
    pub body: &'a [u8],
    pub wait: Duration,
}

impl AppToken {
    pub fn call(&self, key: &str, call: &Call) -> Result<Response> {
        let target = target(call.url, key)?;
        let mut stream = UnixStream::connect(&self.socket).with_context(|| {
            format!(
                "the vault gateway is not answering at {}",
                self.socket.display()
            )
        })?;
        stream.set_read_timeout(Some(call.wait))?;
        stream.set_write_timeout(Some(call.wait))?;
        let bearer = format!("Bearer {}", self.token);
        let mut headers = vec![("Authorization", bearer.as_str())];
        headers.extend(
            call.headers
                .iter()
                .filter(|(name, _)| !name.eq_ignore_ascii_case("authorization"))
                .copied(),
        );
        http::call(&mut stream, call.method, &target, &headers, call.body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_base_url_becomes_a_gateway_route_and_an_approved_prefix() {
        assert_eq!(
            route("https://api.studio.nebius.com/v1/", "nebius").unwrap(),
            "/nebius/api.studio.nebius.com/v1"
        );
        assert_eq!(
            route("https://api.anthropic.com", "anthropic").unwrap(),
            "/anthropic/api.anthropic.com"
        );
        assert_eq!(
            prefix("https://API.openai.com:443/v1").unwrap(),
            "https://api.openai.com/v1/"
        );
        assert!(route("http://example.com/v1", "x").is_err());
        assert!(route("https://example.com/v1?x=1", "x").is_err());
        assert!(route("https://example.com/../v1", "x").is_err());
        assert_eq!(
            target("https://api.openai.com/v1/models", "openai").unwrap(),
            "/openai/api.openai.com/v1/models"
        );
        assert_eq!(
            target("https://api.openai.com", "openai").unwrap(),
            "/openai/api.openai.com/"
        );
    }

    #[test]
    fn a_call_goes_to_the_sandbox_socket_with_the_token_and_never_the_given_authorization() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("gw.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        let seen = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request = http::read_request(&mut stream).unwrap();
            http::Response::json(200, &serde_json::json!({ "ok": true }))
                .write_to(&mut stream)
                .unwrap();
            request
        });
        let token = AppToken {
            id: "t".into(),
            token: "vt_abc".into(),
            revoke: "r".into(),
            socket,
        };
        let answer = token
            .call(
                "openai",
                &Call {
                    method: "POST",
                    url: "https://api.openai.com/v1/chat/completions",
                    headers: &[("Authorization", "Bearer sk-nope"), ("x-extra", "1")],
                    body: b"{}",
                    wait: Duration::from_secs(5),
                },
            )
            .unwrap();
        assert_eq!(answer.status, 200);
        let request = seen.join().unwrap();
        assert_eq!(request.target, "/openai/api.openai.com/v1/chat/completions");
        assert_eq!(request.header("authorization"), Some("Bearer vt_abc"));
        assert_eq!(request.header("x-extra"), Some("1"));
        assert_eq!(request.body, b"{}");
    }

    #[test]
    fn a_token_is_kept_private_and_read_back() {
        let home = tempfile::tempdir().unwrap();
        assert!(load(home.path(), "nebius").is_none());
        let token = AppToken {
            id: "t1".into(),
            token: "vt_x".into(),
            revoke: "r".into(),
            socket: PathBuf::from("/tmp/gw.sock"),
        };
        save(home.path(), "nebius", &token).unwrap();
        assert_eq!(load(home.path(), "nebius"), Some(token));
        use std::os::unix::fs::PermissionsExt;
        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&file(home.path(), "nebius")), 0o600);
        assert_eq!(mode(&home.path().join(FOLDER)), 0o700);
    }
}
