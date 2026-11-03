use crate::cli::Options;
use crate::client;
use crate::daemon::{self, Config, Running};
use crate::vet::NoVetter;
use keys::{MasterKey, MemoryKey, MemoryPresence, Platform};
use serde_json::{json, Value};
use std::path::PathBuf;

pub struct Daemon {
    pub dir: tempfile::TempDir,
    pub admin: PathBuf,
    running: Option<Running>,
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
        let platform = Platform {
            master: Box::new(master),
            presence: Box::new(MemoryPresence { present: true }),
        };
        let running = daemon::start(config, platform, |_| Box::new(NoVetter)).unwrap();
        Daemon {
            dir,
            admin,
            running: Some(running),
        }
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
