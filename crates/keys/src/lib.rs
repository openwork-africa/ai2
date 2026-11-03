use anyhow::{bail, Result};
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Mutex;
use zeroize::Zeroizing;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod passphrase;
pub mod tty;

pub const LINUX_CRED: &str = "/etc/ai2-vault/master.cred";
pub const LINUX_STATE: &str = "/var/lib/ai2-vault";
pub const LINUX_PASSPHRASE: &str = "/var/lib/ai2-vault/passphrase";
pub const SYSTEMD_CREDS: u32 = 250;
pub const KEYCHAIN_ENV: &str = "AI2_VAULT_KEYCHAIN";

pub fn keychain_file(value: Option<OsString>) -> Option<PathBuf> {
    value.filter(|value| !value.is_empty()).map(PathBuf::from)
}

pub type Key = Zeroizing<[u8; 32]>;

pub trait MasterKey: Send + Sync {
    fn exists(&self) -> Result<bool>;
    fn create(&self) -> Result<()>;
    fn load(&self) -> Result<Key>;
}

pub trait Presence: Send + Sync {
    fn confirm(&self, action: &str, passphrase: Option<&str>) -> Result<bool>;
}

pub struct Platform {
    pub master: Box<dyn MasterKey>,
    pub presence: Box<dyn Presence>,
}

#[cfg(target_os = "macos")]
pub fn platform() -> Result<Platform> {
    Ok(Platform {
        master: Box::new(macos::Keychain::in_file(keychain_file(std::env::var_os(
            KEYCHAIN_ENV,
        )))),
        presence: Box::new(macos::TouchId),
    })
}

#[cfg(target_os = "linux")]
pub fn platform() -> Result<Platform> {
    Ok(Platform {
        master: Box::new(linux::Creds::default()),
        presence: Box::new(passphrase::Passphrase::new(LINUX_PASSPHRASE)),
    })
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn platform() -> Result<Platform> {
    bail!("this platform has no vault master key yet")
}

pub fn random_key() -> Result<Key> {
    let mut key = Zeroizing::new([0u8; 32]);
    getrandom::fill(key.as_mut()).map_err(|error| anyhow::anyhow!("no random bytes: {error}"))?;
    Ok(key)
}

pub fn key_from(bytes: &[u8]) -> Result<Key> {
    let Ok(key) = <[u8; 32]>::try_from(bytes) else {
        bail!("the master key is {} bytes, not 32", bytes.len());
    };
    Ok(Zeroizing::new(key))
}

#[derive(Default)]
pub struct MemoryKey(Mutex<Option<[u8; 32]>>);

impl MasterKey for MemoryKey {
    fn exists(&self) -> Result<bool> {
        Ok(self.0.lock().expect("key lock").is_some())
    }
    fn create(&self) -> Result<()> {
        let mut key = [0u8; 32];
        for (i, byte) in key.iter_mut().enumerate() {
            *byte = i as u8 ^ 0x5a;
        }
        *self.0.lock().expect("key lock") = Some(key);
        Ok(())
    }
    fn load(&self) -> Result<Key> {
        match *self.0.lock().expect("key lock") {
            Some(key) => Ok(Zeroizing::new(key)),
            None => bail!("no master key"),
        }
    }
}

pub struct MemoryPresence {
    pub present: bool,
}

impl Presence for MemoryPresence {
    fn confirm(&self, _action: &str, _passphrase: Option<&str>) -> Result<bool> {
        Ok(self.present)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_memory_key_loads_only_after_it_is_made() {
        let key = MemoryKey::default();
        assert!(!key.exists().unwrap() && key.load().is_err());
        key.create().unwrap();
        assert!(key.exists().unwrap());
        assert_eq!(key.load().unwrap().len(), 32);
    }

    #[test]
    fn the_keychain_file_is_named_only_by_a_non_empty_value() {
        assert_eq!(keychain_file(None), None);
        assert_eq!(keychain_file(Some("".into())), None);
        assert_eq!(
            keychain_file(Some("/tmp/t.keychain-db".into())),
            Some(PathBuf::from("/tmp/t.keychain-db"))
        );
    }

    #[test]
    fn random_keys_differ_and_a_key_must_be_32_bytes() {
        assert_ne!(*random_key().unwrap(), *random_key().unwrap());
        assert!(key_from(&[1; 31]).is_err() && key_from(&[1; 33]).is_err());
        assert_eq!(*key_from(&[7; 32]).unwrap(), [7; 32]);
    }
}
