use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

pub const HOST_SANDBOX: &str = "host";
pub const LINUX_ADMIN: &str = "/run/ai2-vault/admin.sock";

pub trait Admin {
    fn sandboxes(&self) -> Result<Vec<String>>;
    fn add_sandbox(&self, name: &str, passphrase: Option<&str>) -> Result<(u16, Value)>;
}

#[cfg(target_os = "linux")]
pub fn default_admin(_state: &Path) -> PathBuf {
    PathBuf::from(LINUX_ADMIN)
}

#[cfg(not(target_os = "linux"))]
pub fn default_admin(state: &Path) -> PathBuf {
    state.join("admin.sock")
}

pub struct Socket {
    pub path: PathBuf,
    pub wait: Duration,
}

impl Socket {
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            wait: Duration::from_secs(5),
        }
    }

    fn connect(&self) -> Result<UnixStream> {
        let until = Instant::now() + self.wait;
        loop {
            match UnixStream::connect(&self.path) {
                Ok(stream) => return Ok(stream),
                Err(error)
                    if matches!(
                        error.kind(),
                        ErrorKind::NotFound | ErrorKind::ConnectionRefused
                    ) && Instant::now() < until =>
                {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("connecting to {}", self.path.display()))
                }
            }
        }
    }

    fn call(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
        passphrase: Option<&str>,
    ) -> Result<(u16, Value)> {
        if passphrase.is_some_and(|passphrase| passphrase.contains(['\r', '\n'])) {
            return Err(anyhow!("a vault passphrase cannot hold a line break"));
        }
        let body = body
            .map(serde_json::to_vec)
            .transpose()?
            .unwrap_or_default();
        let mut request = Zeroizing::new(format!(
            "{method} {path} HTTP/1.1\r\nHost: vault\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        ));
        if let Some(passphrase) = passphrase {
            request.push_str("X-Vault-Passphrase: ");
            request.push_str(passphrase);
            request.push_str("\r\n");
        }
        request.push_str("\r\n");
        request.push_str(&String::from_utf8_lossy(&body));
        let mut stream = self.connect()?;
        stream.set_read_timeout(Some(Duration::from_secs(120)))?;
        stream.write_all(request.as_bytes())?;
        let mut reply = Vec::new();
        stream.read_to_end(&mut reply)?;
        parse(&reply)
    }
}

pub fn parse(reply: &[u8]) -> Result<(u16, Value)> {
    let text = String::from_utf8_lossy(reply);
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or_else(|| anyhow!("the vault daemon gave no HTTP answer"))?;
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| anyhow!("the vault daemon gave no HTTP status"))?;
    Ok((status, serde_json::from_str(body).unwrap_or(Value::Null)))
}

impl Admin for Socket {
    fn sandboxes(&self) -> Result<Vec<String>> {
        let (status, value) = self.call("GET", "/sandboxes", None, None)?;
        if status != 200 {
            return Err(anyhow!(
                "the vault answered {status}: {}",
                value["message"].as_str().unwrap_or("no message")
            ));
        }
        Ok(value["sandboxes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|sandbox| sandbox["name"].as_str().map(str::to_string))
            .collect())
    }

    fn add_sandbox(&self, name: &str, passphrase: Option<&str>) -> Result<(u16, Value)> {
        self.call(
            "POST",
            "/sandboxes",
            Some(&json!({ "name": name })),
            passphrase,
        )
    }
}

#[cfg(any(test, feature = "fake"))]
pub mod fake {
    use super::*;
    use std::cell::RefCell;

    pub struct FakeAdmin {
        pub listed: RefCell<Option<Vec<String>>>,
        pub added: RefCell<Vec<(String, Option<String>)>>,
        pub answer: RefCell<(u16, Value)>,
    }

    impl Default for FakeAdmin {
        fn default() -> Self {
            Self {
                listed: RefCell::new(Some(Vec::new())),
                added: RefCell::default(),
                answer: RefCell::new((200, json!({}))),
            }
        }
    }

    impl FakeAdmin {
        pub fn with(names: &[&str]) -> Self {
            let admin = Self::default();
            *admin.listed.borrow_mut() = Some(names.iter().map(|name| name.to_string()).collect());
            admin
        }

        pub fn unreachable() -> Self {
            let admin = Self::default();
            *admin.listed.borrow_mut() = None;
            admin
        }

        pub fn answering(self, status: u16, value: Value) -> Self {
            *self.answer.borrow_mut() = (status, value);
            self
        }
    }

    impl Admin for FakeAdmin {
        fn sandboxes(&self) -> Result<Vec<String>> {
            self.listed
                .borrow()
                .clone()
                .ok_or_else(|| anyhow!("connecting to the admin socket: no such file"))
        }

        fn add_sandbox(&self, name: &str, passphrase: Option<&str>) -> Result<(u16, Value)> {
            self.added
                .borrow_mut()
                .push((name.into(), passphrase.map(str::to_string)));
            let answer = self.answer.borrow().clone();
            if (200..300).contains(&answer.0) {
                if let Some(listed) = self.listed.borrow_mut().as_mut() {
                    listed.push(name.into());
                }
            }
            Ok(answer)
        }
    }
}
