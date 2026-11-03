use anyhow::{anyhow, Context, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};
use zeroize::Zeroizing;

pub const CODE_LIFE: u64 = 600;
pub const CODE_TRIES: u32 = 5;
pub const CODE_LENGTH: usize = 8;
pub const TOKEN_PREFIX: &str = "ai2_";
const ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const USE_GAP: u64 = 60;
const NAME_LIMIT: usize = 80;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Client {
    pub name: String,
    #[serde(default)]
    pub version: String,
    pub transport: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    pub first_seen: u64,
    pub last_seen: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Kept {
    id: String,
    name: String,
    hash: String,
    created: u64,
    #[serde(default)]
    last_used: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Token {
    pub id: String,
    pub name: String,
    pub created: u64,
    pub last_used: Option<u64>,
}

#[derive(Default, Serialize, Deserialize)]
struct Book {
    #[serde(default)]
    clients: Vec<Client>,
    #[serde(default)]
    tokens: Vec<Kept>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Seen {
    pub name: String,
    pub version: String,
    pub transport: String,
    pub token: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Caller {
    pub token: String,
}

pub struct Minted {
    pub token: Token,
    pub secret: Zeroizing<String>,
}

#[derive(Clone, Debug)]
pub struct Clients {
    path: PathBuf,
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0)
}

fn random<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).map_err(|error| anyhow!("no randomness: {error}"))?;
    Ok(bytes)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hash(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

pub fn clean_name(name: &str, fallback: &str) -> String {
    let kept: String = name
        .chars()
        .filter(|c| !c.is_control())
        .take(NAME_LIMIT)
        .collect();
    let kept = kept.trim();
    if kept.is_empty() {
        fallback.to_string()
    } else {
        kept.to_string()
    }
}

impl Clients {
    pub fn at(home: &Path) -> Self {
        Self {
            path: home.join("mcp").join("clients.json"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn load(&self) -> Result<Book> {
        match std::fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .with_context(|| format!("reading {}", self.path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Book::default()),
            Err(error) => Err(error).with_context(|| format!("reading {}", self.path.display())),
        }
    }

    fn save(&self, book: &Book) -> Result<()> {
        let dir = self.path.parent().unwrap_or(Path::new("."));
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
            .with_context(|| format!("making {}", dir.display()))?;
        let staged = dir.join(format!(
            ".clients.{}.{}",
            std::process::id(),
            hex(&random::<4>()?)
        ));
        let mut out = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&staged)
            .with_context(|| format!("writing {}", staged.display()))?;
        out.write_all(&serde_json::to_vec_pretty(book)?)?;
        out.sync_all()?;
        std::fs::rename(&staged, &self.path)
            .with_context(|| format!("writing {}", self.path.display()))
    }

    pub fn record(&self, seen: Seen, at: u64) -> Result<()> {
        let mut book = self.load()?;
        let name = clean_name(&seen.name, "unnamed");
        let version = clean_name(&seen.version, "");
        match book.clients.iter_mut().find(|client| {
            client.name == name && client.transport == seen.transport && client.token == seen.token
        }) {
            Some(client) => {
                client.version = version;
                client.last_seen = at;
            }
            None => book.clients.push(Client {
                name,
                version,
                transport: seen.transport,
                token: seen.token,
                first_seen: at,
                last_seen: at,
            }),
        }
        self.save(&book)
    }

    pub fn clients(&self) -> Result<Vec<Client>> {
        Ok(self.load()?.clients)
    }

    pub fn tokens(&self) -> Result<Vec<Token>> {
        Ok(self.load()?.tokens.into_iter().map(shown).collect())
    }

    pub fn mint(&self, name: &str, at: u64) -> Result<Minted> {
        let mut book = self.load()?;
        let secret = Zeroizing::new(format!("{TOKEN_PREFIX}{}", hex(&random::<32>()?)));
        let kept = Kept {
            id: format!("mcp_{}", hex(&random::<6>()?)),
            name: clean_name(name, "HTTP client"),
            hash: hash(&secret),
            created: at,
            last_used: None,
        };
        let token = shown(kept.clone());
        book.tokens.push(kept);
        self.save(&book)?;
        Ok(Minted { token, secret })
    }

    pub fn check(&self, given: &str, at: u64) -> Option<String> {
        if !given.starts_with(TOKEN_PREFIX) {
            return None;
        }
        let mut book = self.load().ok()?;
        let given = hash(given);
        let kept = book
            .tokens
            .iter_mut()
            .find(|kept| same(&kept.hash, &given))?;
        let id = kept.id.clone();
        if kept
            .last_used
            .is_none_or(|used| at.saturating_sub(used) >= USE_GAP)
        {
            kept.last_used = Some(at);
            let _ = self.save(&book);
        }
        Some(id)
    }

    pub fn revoke(&self, id: &str) -> Result<bool> {
        let mut book = self.load()?;
        let before = book.tokens.len();
        book.tokens.retain(|kept| kept.id != id);
        if book.tokens.len() == before {
            return Ok(false);
        }
        self.save(&book)?;
        Ok(true)
    }
}

fn shown(kept: Kept) -> Token {
    Token {
        id: kept.id,
        name: kept.name,
        created: kept.created,
        last_used: kept.last_used,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Code {
    pub code: String,
    pub expires: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Refusal {
    None,
    Wrong,
    Expired,
}

impl Refusal {
    pub fn message(self) -> &'static str {
        match self {
            Refusal::None => "There is no pairing code waiting. Make one in ai2 ui.",
            Refusal::Wrong => "That pairing code is not the one ai2 ui shows.",
            Refusal::Expired => "That pairing code ran out. Make a new one in ai2 ui.",
        }
    }
}

struct Live {
    code: String,
    expires: u64,
    tries: u32,
}

#[derive(Default)]
pub struct Pairing {
    live: Mutex<Option<Live>>,
}

fn normal(code: &str) -> String {
    code.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

impl Pairing {
    fn lock(&self) -> MutexGuard<'_, Option<Live>> {
        self.live
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn issue(&self, at: u64) -> Result<Code> {
        let code: String = random::<CODE_LENGTH>()?
            .iter()
            .map(|byte| ALPHABET[(byte & 31) as usize] as char)
            .collect();
        let expires = at + CODE_LIFE;
        *self.lock() = Some(Live {
            code: code.clone(),
            expires,
            tries: 0,
        });
        Ok(Code { code, expires })
    }

    pub fn waiting(&self, at: u64) -> Option<Code> {
        self.lock()
            .as_ref()
            .filter(|live| at < live.expires)
            .map(|live| Code {
                code: live.code.clone(),
                expires: live.expires,
            })
    }

    pub fn take(&self, given: &str, at: u64) -> Result<(), Refusal> {
        let mut live = self.lock();
        let Some(waiting) = live.as_mut() else {
            return Err(Refusal::None);
        };
        if at >= waiting.expires {
            *live = None;
            return Err(Refusal::Expired);
        }
        if !same(&normal(given), &waiting.code) {
            waiting.tries += 1;
            if waiting.tries >= CODE_TRIES {
                *live = None;
            }
            return Err(Refusal::Wrong);
        }
        *live = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn a_pairing_code_is_taken_once_before_it_runs_out() {
        let pairing = Pairing::default();
        assert_eq!(pairing.take("ABCDEFGH", 0), Err(Refusal::None));
        let code = pairing.issue(1000).unwrap();
        assert_eq!(code.code.len(), CODE_LENGTH);
        assert!(code.code.bytes().all(|byte| ALPHABET.contains(&byte)));
        assert_eq!(code.expires, 1000 + CODE_LIFE);
        assert_eq!(pairing.waiting(1001), Some(code.clone()));
        let lower = format!("{}-{}", &code.code[..4], &code.code[4..]).to_lowercase();
        assert_eq!(pairing.take(&lower, 1001), Ok(()));
        assert_eq!(pairing.take(&code.code, 1002), Err(Refusal::None));
        assert_eq!(pairing.waiting(1002), None);

        let late = pairing.issue(2000).unwrap();
        assert_eq!(pairing.waiting(2000 + CODE_LIFE), None);
        assert_eq!(
            pairing.take(&late.code, 2000 + CODE_LIFE),
            Err(Refusal::Expired)
        );
        assert_eq!(pairing.take(&late.code, 2001), Err(Refusal::None));
    }

    #[test]
    fn a_pairing_code_is_dropped_after_too_many_wrong_tries() {
        let pairing = Pairing::default();
        let code = pairing.issue(0).unwrap();
        for _ in 0..CODE_TRIES {
            assert_eq!(pairing.take("22222222", 1), Err(Refusal::Wrong));
        }
        assert_eq!(pairing.take(&code.code, 1), Err(Refusal::None));
        let first = pairing.issue(0).unwrap();
        let second = pairing.issue(0).unwrap();
        if first.code != second.code {
            assert_eq!(pairing.take(&first.code, 1), Err(Refusal::Wrong));
        }
        assert_eq!(pairing.take(&second.code, 1), Ok(()));
    }

    #[test]
    fn a_token_is_kept_hashed_checked_and_revoked() {
        let home = tempfile::tempdir().unwrap();
        let clients = Clients::at(home.path());
        assert!(clients.tokens().unwrap().is_empty());
        let minted = clients.mint("Cursor\u{7}", 10).unwrap();
        assert!(minted.secret.starts_with(TOKEN_PREFIX));
        assert_eq!(minted.token.name, "Cursor");
        let text = std::fs::read_to_string(clients.path()).unwrap();
        assert!(!text.contains(minted.secret.as_str()));
        assert!(text.contains(&hash(&minted.secret)));
        let mode = std::fs::metadata(clients.path())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);

        assert_eq!(
            clients.check(&minted.secret, 20),
            Some(minted.token.id.clone())
        );
        assert_eq!(clients.tokens().unwrap()[0].last_used, Some(20));
        clients.check(&minted.secret, 30).unwrap();
        assert_eq!(clients.tokens().unwrap()[0].last_used, Some(20));
        assert_eq!(clients.check("ai2_wrong", 30), None);
        assert_eq!(clients.check("nonsense", 30), None);

        assert!(clients.revoke(&minted.token.id).unwrap());
        assert!(!clients.revoke(&minted.token.id).unwrap());
        assert_eq!(clients.check(&minted.secret, 40), None);
    }

    #[test]
    fn a_client_is_recorded_once_per_name_transport_and_token() {
        let home = tempfile::tempdir().unwrap();
        let clients = Clients::at(home.path());
        let seen = |version: &str, transport: &str| Seen {
            name: "claude-code".into(),
            version: version.into(),
            transport: transport.into(),
            token: None,
        };
        clients.record(seen("1", "stdio"), 5).unwrap();
        clients.record(seen("2", "stdio"), 9).unwrap();
        clients.record(seen("2", "http"), 9).unwrap();
        let all = clients.clients().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(
            all[0],
            Client {
                name: "claude-code".into(),
                version: "2".into(),
                transport: "stdio".into(),
                token: None,
                first_seen: 5,
                last_seen: 9,
            }
        );
    }
}
