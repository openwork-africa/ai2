use crate::cli::Options;
use crate::client;
use crate::daemon::{self, Config, Running};
use crate::vet::NoVetter;
use keys::{Caller, MasterKey, MemoryKey, Platform, Presence};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct Counting(Arc<AtomicUsize>);

impl Presence for Counting {
    fn confirm(
        &self,
        _action: &str,
        _caller: &Caller,
        _passphrase: Option<&str>,
    ) -> anyhow::Result<bool> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(true)
    }

    fn kind(&self) -> &'static str {
        keys::MEMORY
    }
}

pub struct Daemon {
    pub dir: tempfile::TempDir,
    pub admin: PathBuf,
    running: Option<Running>,
    prompts: Arc<AtomicUsize>,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        if let Some(running) = self.running.take() {
            running.stop();
        }
    }
}

impl Daemon {
    pub fn start() -> Daemon {
        let dir = tempfile::Builder::new()
            .prefix("ai2t")
            .tempdir_in("/tmp")
            .unwrap();
        let state = dir.path().join("v");
        let admin = state.join("admin.sock");
        let mut config = Config::new(state, admin.clone());
        config.sockets = dir.path().join("s");
        let master = MemoryKey::default();
        master.create().unwrap();
        let prompts = Arc::new(AtomicUsize::new(0));
        let platform = Platform {
            master: Box::new(master),
            presence: Box::new(Counting(prompts.clone())),
        };
        let running = daemon::start(config, platform, |_| Box::new(NoVetter)).unwrap();
        Daemon {
            dir,
            admin,
            running: Some(running),
            prompts,
        }
    }

    pub fn prompts(&self) -> usize {
        self.prompts.load(Ordering::SeqCst)
    }

    pub fn options(&self) -> Options {
        Options {
            admin: self.admin.clone(),
            state: self.dir.path().join("v"),
            sockets: self.dir.path().join("s"),
            ask_passphrase: false,
            serve: Box::new(|_| Ok(())),
        }
    }

    pub fn call(&self, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
        client::call(&self.admin, method, path, body.as_ref(), &[]).unwrap()
    }

    pub fn put(&self, name: &str) {
        let body = json!({ "secret": "s3cret", "approved_urls": ["https://h.example/"] });
        assert_eq!(
            self.call("PUT", &format!("/keys/{name}"), Some(body)).0,
            200
        );
    }

    pub fn names(&self, path: &str, field: &str, name: &str) -> Vec<String> {
        self.call("GET", path, None).1[field]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item[name].as_str().unwrap().to_string())
            .collect()
    }

    pub fn key_names(&self) -> Vec<String> {
        self.names("/keys", "keys", "name")
    }
}
