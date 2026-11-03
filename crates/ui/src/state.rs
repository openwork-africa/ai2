use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::watch;

pub type Shared = Arc<State>;

pub struct State {
    pub nonce: String,
    pub port: u16,
    pub host: String,
    pub origin: String,
    pub home: PathBuf,
    pub admin: PathBuf,
    pub apps: apps::Supervisor,
    pub agents: agents::Agents,
    pub dashboard: u16,
    pub handover: AtomicBool,
    pub stop: watch::Sender<bool>,
}

impl State {
    pub fn new(port: u16, nonce: String, options: &crate::Options) -> anyhow::Result<Self> {
        let host = format!("127.0.0.1:{port}");
        Ok(Self {
            nonce,
            port,
            origin: format!("http://{host}"),
            host,
            home: options.home.clone(),
            admin: options.admin.clone(),
            apps: apps::Supervisor::new(apps::Store::at_home(&options.home)?),
            agents: options.agents.clone(),
            dashboard: crate::service::port(),
            handover: AtomicBool::new(false),
            stop: watch::channel(false).0,
        })
    }
}
