use crate::{key_from, random_key, Caller, Key, MasterKey, Presence, TOUCH_ID};
use anyhow::{anyhow, bail, Result};
use block2::RcBlock;
use objc2::runtime::Bool;
use objc2_foundation::{NSError, NSString};
use objc2_local_authentication::{LAContext, LAPolicy};
use security_framework::item::{ItemClass, ItemSearchOptions};
use security_framework::os::macos::item::ItemSearchOptionsExt;
use security_framework::os::macos::keychain::SecKeychain;
use security_framework::passwords::{get_generic_password, set_generic_password};
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::Duration;

pub const SERVICE: &str = "ai2-vault";
pub const ACCOUNT: &str = "master";
const NOT_FOUND: i32 = -25300;
const PROMPT_LIMIT: Duration = Duration::from_secs(120);

pub struct Keychain {
    pub service: String,
    pub account: String,
    pub file: Option<PathBuf>,
}

impl Default for Keychain {
    fn default() -> Self {
        Self {
            service: SERVICE.into(),
            account: ACCOUNT.into(),
            file: None,
        }
    }
}

impl Keychain {
    pub fn in_file(file: Option<PathBuf>) -> Self {
        Self {
            file,
            ..Self::default()
        }
    }

    fn get(&self) -> security_framework::base::Result<Vec<u8>> {
        match &self.file {
            None => get_generic_password(&self.service, &self.account),
            Some(file) => SecKeychain::open(file)?
                .find_generic_password(&self.service, &self.account)
                .map(|(password, _)| password.to_vec()),
        }
    }

    fn find(&self) -> security_framework::base::Result<bool> {
        let mut search = ItemSearchOptions::new();
        search
            .class(ItemClass::generic_password())
            .service(&self.service)
            .account(&self.account)
            .load_attributes(true)
            .limit(1);
        if let Some(file) = &self.file {
            search.keychains(&[SecKeychain::open(file)?]);
        }
        search.search().map(|found| !found.is_empty())
    }

    fn set(&self, key: &[u8]) -> security_framework::base::Result<()> {
        match &self.file {
            None => set_generic_password(&self.service, &self.account, key),
            Some(file) => {
                SecKeychain::open(file)?.set_generic_password(&self.service, &self.account, key)
            }
        }
    }
}

impl MasterKey for Keychain {
    fn exists(&self) -> Result<bool> {
        match self.find() {
            Ok(found) => Ok(found),
            Err(error) if error.code() == NOT_FOUND => Ok(false),
            Err(error) => Err(anyhow!("reading the keychain: {error}")),
        }
    }

    fn create(&self) -> Result<()> {
        if self.exists()? {
            bail!("the keychain already has a vault master key");
        }
        let key = random_key()?;
        self.set(key.as_ref())
            .map_err(|error| anyhow!("writing the keychain: {error}"))
    }

    fn load(&self) -> Result<Key> {
        let bytes = zeroize::Zeroizing::new(
            self.get()
                .map_err(|error| anyhow!("reading the vault master key: {error}"))?,
        );
        key_from(&bytes)
    }
}

pub struct TouchId;

pub const POLICY: LAPolicy = LAPolicy::DeviceOwnerAuthentication;

pub fn available() -> Result<bool> {
    let context = unsafe { LAContext::new() };
    Ok(unsafe { context.canEvaluatePolicy_error(POLICY) }.is_ok())
}

impl Presence for TouchId {
    fn confirm(&self, action: &str, _caller: &Caller, _passphrase: Option<&str>) -> Result<bool> {
        if action.trim().is_empty() {
            bail!("say what needs a human present");
        }
        let context = unsafe { LAContext::new() };
        let reason = NSString::from_str(action);
        let (to, from) = channel();
        let reply = RcBlock::new(move |ok: Bool, _error: *mut NSError| {
            let _ = to.send(ok.as_bool());
        });
        unsafe { context.evaluatePolicy_localizedReason_reply(POLICY, &reason, &reply) };
        match from.recv_timeout(PROMPT_LIMIT) {
            Ok(ok) => Ok(ok),
            Err(_) => {
                unsafe { context.invalidate() };
                Ok(false)
            }
        }
    }

    fn kind(&self) -> &'static str {
        TOUCH_ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn this_process_can_ask_for_touch_id_or_the_password() {
        assert!(available().unwrap());
    }

    #[test]
    #[ignore]
    fn a_touch_id_prompt_confirms() {
        assert!(TouchId
            .confirm("test the ai2 vault prompt", &Caller::default(), None)
            .unwrap());
    }

    #[test]
    fn the_keychain_is_the_login_one_unless_a_file_is_named() {
        let keychain = Keychain::default();
        assert_eq!(
            (keychain.service.as_str(), keychain.account.as_str()),
            ("ai2-vault", "master")
        );
        assert!(keychain.file.is_none());
        let named = Keychain::in_file(Some("/tmp/t.keychain-db".into()));
        assert_eq!(
            named.file.as_deref(),
            Some(std::path::Path::new("/tmp/t.keychain-db"))
        );
        assert_eq!(named.service, SERVICE);
    }

    #[test]
    fn an_empty_action_is_refused_before_any_prompt() {
        assert!(TouchId.confirm("  ", &Caller::default(), None).is_err());
    }
}
