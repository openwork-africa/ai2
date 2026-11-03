use std::path::PathBuf;
use std::sync::Arc;

pub type Shared = Arc<State>;

#[derive(Debug)]
pub struct State {
    pub nonce: String,
    pub port: u16,
    pub host: String,
    pub origin: String,
    pub home: PathBuf,
    pub admin: PathBuf,
}

impl State {
    pub fn new(port: u16, nonce: String, options: &crate::Options) -> Self {
        let host = format!("127.0.0.1:{port}");
        Self {
            nonce,
            port,
            origin: format!("http://{host}"),
            host,
            home: options.home.clone(),
            admin: options.admin.clone(),
        }
    }
}
