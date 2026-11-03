use crate::Presence;
use anyhow::{anyhow, bail, Context, Result};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const MIN_CHARS: usize = 8;

pub struct Passphrase {
    hash: PathBuf,
}

impl Passphrase {
    pub fn new(hash: impl Into<PathBuf>) -> Self {
        Self { hash: hash.into() }
    }
}

impl Presence for Passphrase {
    fn confirm(&self, _action: &str, passphrase: Option<&str>) -> Result<bool> {
        let Some(given) = passphrase else {
            return Ok(false);
        };
        let stored = std::fs::read_to_string(&self.hash)
            .with_context(|| format!("no passphrase is set at {}", self.hash.display()))?;
        verify(stored.trim(), given)
    }
}

pub fn hash(passphrase: &str) -> Result<String> {
    if passphrase.chars().count() < MIN_CHARS {
        bail!("the passphrase needs at least {MIN_CHARS} characters");
    }
    let mut salt = [0u8; 16];
    getrandom::fill(&mut salt).map_err(|error| anyhow!("no random bytes: {error}"))?;
    let salt = SaltString::encode_b64(&salt).map_err(|error| anyhow!("{error}"))?;
    Ok(Argon2::default()
        .hash_password(passphrase.as_bytes(), &salt)
        .map_err(|error| anyhow!("hashing the passphrase: {error}"))?
        .to_string())
}

pub fn verify(stored: &str, given: &str) -> Result<bool> {
    let parsed = PasswordHash::new(stored)
        .map_err(|error| anyhow!("the passphrase hash is bad: {error}"))?;
    if parsed.algorithm.as_str() != "argon2id" {
        bail!("the passphrase hash is not Argon2id");
    }
    Ok(Argon2::default()
        .verify_password(given.as_bytes(), &parsed)
        .is_ok())
}

pub fn set(path: &Path, passphrase: &str) -> Result<()> {
    let hashed = hash(passphrase)?;
    let dir = path
        .parent()
        .ok_or_else(|| anyhow!("{} has no folder", path.display()))?;
    std::fs::create_dir_all(dir).with_context(|| format!("making {}", dir.display()))?;
    let temp = path.with_extension("new");
    let mut file = open_private(&temp)?;
    file.write_all(format!("{hashed}\n").as_bytes())?;
    file.sync_all()?;
    std::fs::rename(&temp, path).with_context(|| format!("writing {}", path.display()))
}

fn open_private(path: &Path) -> Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn the_set_passphrase_is_the_only_one_that_confirms() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state").join("passphrase");
        set(&path, "correct horse battery").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .starts_with("$argon2id$"));
        let presence = Passphrase::new(&path);
        assert!(presence
            .confirm("delete stripe", Some("correct horse battery"))
            .unwrap());
        assert!(!presence
            .confirm("delete stripe", Some("wrong one!"))
            .unwrap());
        assert!(!presence.confirm("delete stripe", None).unwrap());
    }

    #[test]
    fn short_passphrases_and_missing_hashes_are_refused() {
        assert!(hash("short").is_err());
        let dir = tempfile::tempdir().unwrap();
        let presence = Passphrase::new(dir.path().join("none"));
        assert!(presence.confirm("x", Some("whatever long")).is_err());
    }

    #[test]
    fn two_hashes_of_one_passphrase_differ_by_salt() {
        let a = hash("the same passphrase").unwrap();
        let b = hash("the same passphrase").unwrap();
        assert_ne!(a, b);
        assert!(verify(&a, "the same passphrase").unwrap());
        assert!(verify(
            "$argon2i$v=19$m=16,t=2,p=1$c29tZXNhbHQ$aaaaaaaaaaaaaaaaaaaaaa",
            "x"
        )
        .is_err());
    }
}
