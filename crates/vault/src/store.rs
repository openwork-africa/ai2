use crate::crypto::{self, Sealed};
use crate::vet::KeyInfo;
use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs::{DirBuilder, OpenOptions};
use std::io::Write;
use std::net::IpAddr;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone)]
pub struct KeyRecord {
    pub name: String,
    pub description: String,
    pub approved_urls: Vec<String>,
    pub inject: String,
    pub created: String,
    pub rotated: Option<String>,
    pub nonce: String,
    pub secret: String,
}

impl KeyRecord {
    pub fn sealed(&self) -> Sealed {
        Sealed {
            nonce: self.nonce.clone(),
            secret: self.secret.clone(),
        }
    }

    pub fn info(&self) -> KeyInfo {
        KeyInfo {
            name: self.name.clone(),
            description: self.description.clone(),
            approved_urls: self.approved_urls.clone(),
        }
    }

    pub fn public(&self) -> Value {
        json!({
            "name": self.name,
            "description": self.description,
            "approved_urls": self.approved_urls,
            "inject": self.inject,
            "created": self.created,
            "rotated": self.rotated,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    App,
    Agent,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TokenRecord {
    pub id: String,
    pub hash: String,
    pub keys: Vec<String>,
    pub sandbox: String,
    pub mode: Mode,
    pub methods: Vec<String>,
    pub url_prefixes: Vec<String>,
    pub intent: Option<String>,
    pub created: u64,
    pub expires: u64,
    pub uses: Option<u64>,
    #[serde(default)]
    pub revoke: Option<String>,
}

impl TokenRecord {
    pub fn public(&self) -> Value {
        json!({
            "id": self.id,
            "keys": self.keys,
            "sandbox": self.sandbox,
            "mode": self.mode,
            "methods": self.methods,
            "url_prefixes": self.url_prefixes,
            "intent": self.intent,
            "created": crate::time::rfc3339(self.created),
            "expires": crate::time::rfc3339(self.expires),
            "uses_left": self.uses,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(tag = "transport", rename_all = "lowercase")]
pub enum Endpoint {
    Unix { path: PathBuf },
    Tcp { listen: String, peers: Vec<IpAddr> },
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Sandbox {
    pub name: String,
    #[serde(flatten)]
    pub endpoint: Endpoint,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Vetted {
    pub key: String,
    pub intent: String,
    pub prefix: String,
    pub why: String,
    pub created: String,
}

#[derive(Serialize)]
pub struct AuditLine<'a> {
    pub time: String,
    pub token: Option<&'a str>,
    pub sandbox: &'a str,
    pub key: Option<&'a str>,
    pub method: &'a str,
    pub url: Option<&'a str>,
    pub request: Option<&'a str>,
    pub decision: &'a str,
    pub status: u16,
    pub reason: Option<&'a str>,
}

const AUDIT_LIMIT: u64 = 1024 * 1024;

pub struct Store {
    dir: PathBuf,
    pub audit_limit: u64,
    pub keys: Vec<KeyRecord>,
    pub tokens: Vec<TokenRecord>,
    pub sandboxes: Vec<Sandbox>,
    pub vetted: Vec<Vetted>,
}

impl Store {
    pub fn open(dir: &Path) -> Result<Store> {
        private_dir(dir)?;
        Ok(Store {
            dir: dir.to_path_buf(),
            audit_limit: AUDIT_LIMIT,
            keys: load(dir, "keys")?,
            tokens: load(dir, "tokens")?,
            sandboxes: load(dir, "sandboxes")?,
            vetted: load(dir, "vetted")?,
        })
    }

    pub fn key(&self, name: &str) -> Option<&KeyRecord> {
        self.keys.iter().find(|key| key.name == name)
    }

    pub fn sandbox(&self, name: &str) -> Option<&Sandbox> {
        self.sandboxes.iter().find(|sandbox| sandbox.name == name)
    }

    pub fn token(&self, id: &str) -> Option<&TokenRecord> {
        self.tokens.iter().find(|token| token.id == id)
    }

    pub fn token_by_hash(&self, hash: &str) -> Option<&TokenRecord> {
        self.tokens.iter().find(|token| token.hash == hash)
    }

    pub fn forget_key(&mut self, name: &str) {
        self.keys.retain(|key| key.name != name);
        self.vetted.retain(|vetted| vetted.key != name);
    }

    pub fn save<T: Serialize>(&self, name: &str, items: &[T]) -> Result<()> {
        self.write(&format!("{name}.json"), &json!({ name: items }))
    }

    fn write(&self, file: &str, value: &Value) -> Result<()> {
        let path = self.dir.join(file);
        let temp = self.dir.join(format!(".{file}.{}", crypto::id("")?));
        let written = (|| -> Result<()> {
            let mut out = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temp)?;
            serde_json::to_writer_pretty(&mut out, value)?;
            out.write_all(b"\n")?;
            out.sync_all()?;
            std::fs::rename(&temp, &path)?;
            Ok(())
        })();
        if written.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        written.with_context(|| format!("writing {}", path.display()))
    }

    pub fn audit(&self, line: &AuditLine) -> Result<()> {
        let path = self.dir.join("audit.jsonl");
        if std::fs::metadata(&path).is_ok_and(|meta| meta.len() >= self.audit_limit) {
            std::fs::rename(&path, self.dir.join("audit.jsonl.1"))?;
        }
        let mut out = OpenOptions::new()
            .append(true)
            .create(true)
            .mode(0o600)
            .open(self.dir.join("audit.jsonl"))?;
        let mut text = serde_json::to_vec(line)?;
        text.push(b'\n');
        out.write_all(&text)?;
        Ok(())
    }

    pub fn audit_tail(&self, tail: usize) -> Result<Vec<Value>> {
        let text = format!(
            "{}{}",
            read_or_empty(&self.dir.join("audit.jsonl.1"))?,
            read_or_empty(&self.dir.join("audit.jsonl"))?
        );
        let lines: Vec<&str> = text.lines().filter(|line| !line.is_empty()).collect();
        Ok(lines[lines.len().saturating_sub(tail)..]
            .iter()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect())
    }
}

fn read_or_empty(path: &Path) -> Result<String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error.into()),
    }
}

fn private_dir(dir: &Path) -> Result<()> {
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .with_context(|| format!("making {}", dir.display()))?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn load<T: DeserializeOwned>(dir: &Path, field: &str) -> Result<Vec<T>> {
    let path = dir.join(format!("{field}.json"));
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).with_context(|| format!("reading {}", path.display())),
    };
    let mut value: Value =
        serde_json::from_str(&text).with_context(|| format!("{} is not JSON", path.display()))?;
    serde_json::from_value(value[field].take())
        .with_context(|| format!("{} is not a {field} file", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_private_and_survive_a_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let state = dir.path().join("vault");
        let mut store = Store::open(&state).unwrap();
        let sealed = crypto::seal(&[1; 32], "k", b"s3cret-value").unwrap();
        store.keys.push(KeyRecord {
            name: "k".into(),
            description: String::new(),
            approved_urls: vec!["https://h.io/".into()],
            inject: "bearer".into(),
            created: crate::time::stamp(),
            rotated: None,
            nonce: sealed.nonce,
            secret: sealed.secret,
        });
        store.save("keys", &store.keys).unwrap();
        store
            .audit(&AuditLine {
                time: crate::time::stamp(),
                token: None,
                sandbox: "host",
                key: Some("k"),
                method: "GET",
                url: None,
                request: None,
                decision: "deny",
                status: 403,
                reason: Some("token_unknown"),
            })
            .unwrap();
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&state), 0o700);
        assert_eq!(mode(&state.join("keys.json")), 0o600);
        assert_eq!(mode(&state.join("audit.jsonl")), 0o600);
        let text = std::fs::read_to_string(state.join("keys.json")).unwrap();
        assert!(!text.contains("s3cret"));
        let again = Store::open(&state).unwrap();
        assert_eq!(again.key("k").unwrap().approved_urls, ["https://h.io/"]);
        assert_eq!(again.audit_tail(5).unwrap()[0]["reason"], "token_unknown");
        assert_eq!(std::fs::read_dir(&state).unwrap().count(), 2);
    }

    #[test]
    fn the_audit_log_rotates_to_one_old_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        store.audit_limit = 300;
        for n in 0..20u16 {
            store
                .audit(&AuditLine {
                    time: crate::time::stamp(),
                    token: None,
                    sandbox: "host",
                    key: None,
                    method: "GET",
                    url: None,
                    request: None,
                    decision: "deny",
                    status: n,
                    reason: None,
                })
                .unwrap();
        }
        let size = |name: &str| std::fs::metadata(dir.path().join(name)).unwrap().len();
        assert!(size("audit.jsonl") < 300 + 200 && size("audit.jsonl.1") >= 300);
        assert!(!dir.path().join("audit.jsonl.2").exists());
        let tail = store.audit_tail(100).unwrap();
        assert_eq!(tail.last().unwrap()["status"], 19);
        assert!(tail.len() < 20);
    }
}
