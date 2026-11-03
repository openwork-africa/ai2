use crate::{key_from, Key, MasterKey, LINUX_CRED};
use anyhow::{bail, Context, Result};
use std::path::PathBuf;

pub const CRED_NAME: &str = "master";

pub struct Creds {
    pub cred: PathBuf,
}

impl Default for Creds {
    fn default() -> Self {
        Self {
            cred: LINUX_CRED.into(),
        }
    }
}

pub fn loaded() -> Option<PathBuf> {
    std::env::var_os("CREDENTIALS_DIRECTORY").map(|dir| PathBuf::from(dir).join(CRED_NAME))
}

impl MasterKey for Creds {
    fn exists(&self) -> Result<bool> {
        Ok(loaded().is_some_and(|path| path.is_file()) || self.cred.exists())
    }

    fn create(&self) -> Result<()> {
        if self.exists()? {
            bail!("{} already holds a vault master key", self.cred.display());
        }
        bail!("the vault master key is made by ai2 setup")
    }

    fn load(&self) -> Result<Key> {
        let Some(path) = loaded() else {
            bail!("no CREDENTIALS_DIRECTORY: the vault daemon runs as ai2-vault.service with LoadCredentialEncrypted");
        };
        let bytes = zeroize::Zeroizing::new(
            std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?,
        );
        key_from(&bytes)
    }
}
