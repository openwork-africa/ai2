use crate::approvals::Approvals;
use crate::crypto;
use crate::http::{self, Request};
use crate::store::{Endpoint, Store};
use crate::vet::Vetter;
use crate::{admin, gateway};
use anyhow::{anyhow, bail, Context, Result};
use keys::{Caller, Platform};
use serde_json::json;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, PermissionsExt};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

const ADMIN: &str = "";
const READ_TIMEOUT: Duration = Duration::from_secs(60);

pub struct Config {
    pub state: PathBuf,
    pub admin: PathBuf,
    pub admin_mode: u32,
    pub sockets: PathBuf,
    pub socket_mode: u32,
    pub allow_http: bool,
    pub approval_timeout: Duration,
    pub owners: Vec<u32>,
}

impl Config {
    pub fn new(state: PathBuf, admin: PathBuf) -> Config {
        Config {
            sockets: state.join("sandboxes"),
            state,
            admin,
            admin_mode: if cfg!(target_os = "linux") {
                0o660
            } else {
                0o600
            },
            socket_mode: if cfg!(target_os = "linux") {
                0o660
            } else {
                0o600
            },
            allow_http: false,
            approval_timeout: Duration::from_secs(120),
            owners: vec![unsafe { libc::geteuid() }],
        }
    }
}

pub type Resolver = Arc<dyn Fn(&str) -> Result<Zeroizing<String>> + Send + Sync>;

pub struct Vault {
    pub config: Config,
    platform: Platform,
    vetter: OnceLock<Box<dyn Vetter>>,
    pub store: Mutex<Store>,
    pub approvals: Approvals,
    master: Mutex<Option<keys::Key>>,
    listeners: Mutex<HashMap<String, Listener>>,
    pub agent: ureq::Agent,
    refused_at: Mutex<Option<Instant>>,
}

struct Listener {
    stop: Arc<AtomicBool>,
    wake: Wake,
    thread: Option<JoinHandle<()>>,
}

#[derive(Clone)]
enum Wake {
    Unix(PathBuf),
    Tcp(SocketAddr),
}

pub enum Bound {
    Unix(UnixListener, PathBuf),
    Tcp(TcpListener, Vec<IpAddr>),
}

pub struct Running {
    vault: Arc<Vault>,
}

pub fn start(
    config: Config,
    platform: Platform,
    make: impl FnOnce(Resolver) -> Box<dyn Vetter>,
) -> Result<Running> {
    let store = Store::open(&config.state)?;
    let agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .max_redirects(0)
        .https_only(!config.allow_http)
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_recv_response(Some(Duration::from_secs(300)))
        .build()
        .new_agent();
    let vault = Arc::new(Vault {
        config,
        platform,
        vetter: OnceLock::new(),
        store: Mutex::new(store),
        approvals: Approvals::default(),
        master: Mutex::new(None),
        listeners: Mutex::new(HashMap::new()),
        agent,
        refused_at: Mutex::new(None),
    });
    let weak: Weak<Vault> = Arc::downgrade(&vault);
    let resolver: Resolver = Arc::new(move |name: &str| {
        weak.upgrade()
            .ok_or_else(|| anyhow!("the vault has stopped"))?
            .secret(name)
    });
    let _ = vault.vetter.set(make(resolver));
    let admin = bind_unix(&vault.config.admin, vault.config.admin_mode)?;
    vault.listen(ADMIN, Bound::Unix(admin, vault.config.admin.clone()));
    let sandboxes = vault.store().sandboxes.clone();
    for sandbox in sandboxes {
        if let Err(error) = vault.open_endpoint(&sandbox.name, &sandbox.endpoint) {
            eprintln!(
                "ai2 vault: the {} sandbox endpoint did not open: {error:#}",
                sandbox.name
            );
        }
    }
    Ok(Running { vault })
}

impl Running {
    pub fn wait(self) {
        let thread = self
            .vault
            .listeners
            .lock()
            .expect("listeners lock")
            .get_mut(ADMIN)
            .and_then(|l| l.thread.take());
        if let Some(thread) = thread {
            let _ = thread.join();
        }
    }

    pub fn stop(self) {
        let names: Vec<String> = self
            .vault
            .listeners
            .lock()
            .expect("listeners lock")
            .keys()
            .cloned()
            .collect();
        for name in names {
            self.vault.close_endpoint(&name);
        }
    }
}

impl Vault {
    pub fn store(&self) -> std::sync::MutexGuard<'_, Store> {
        self.store.lock().expect("store lock")
    }

    pub fn vetter(&self) -> &dyn Vetter {
        self.vetter.get().map(|v| v.as_ref()).expect("vetter set")
    }

    pub fn master(&self) -> Result<keys::Key> {
        let mut master = self.master.lock().expect("master lock");
        if master.is_none() {
            *master = Some(self.platform.master.load().context("the master key")?);
        }
        Ok(master.clone().expect("loaded"))
    }

    pub fn has_master(&self) -> bool {
        self.master().is_ok()
    }

    pub fn secret(&self, name: &str) -> Result<Zeroizing<String>> {
        let sealed = self
            .store()
            .key(name)
            .map(|key| key.sealed())
            .ok_or_else(|| anyhow!("there is no vault key {name}"))?;
        crypto::open(&*self.master()?, name, &sealed)
    }

    pub fn present(&self, request: &Request, caller: &Caller, action: &str) -> bool {
        let passphrase = request.header("x-vault-passphrase");
        matches!(
            self.platform.presence.confirm(action, caller, passphrase),
            Ok(true)
        )
    }

    pub fn presence_kind(&self) -> &'static str {
        self.platform.presence.kind()
    }

    fn refused(&self, sandbox: &str, uid: Option<u32>) {
        let mut at = self.refused_at.lock().expect("refusal lock");
        if at.is_some_and(|at| at.elapsed() < Duration::from_secs(60)) {
            return;
        }
        *at = Some(Instant::now());
        let socket = if sandbox == ADMIN {
            "the admin socket".to_string()
        } else {
            format!("the {sandbox} sandbox socket")
        };
        let who = uid.map_or("an unknown user".to_string(), |uid| format!("uid {uid}"));
        eprintln!("ai2 vault: refused {who} on {socket}; only {:?} may call it (said at most once a minute)", self.config.owners);
    }

    pub fn open_endpoint(self: &Arc<Self>, name: &str, endpoint: &Endpoint) -> Result<Endpoint> {
        let (bound, actual) = match endpoint {
            Endpoint::Unix { path } => {
                let listener = bind_unix(path, self.config.socket_mode)?;
                (Bound::Unix(listener, path.clone()), endpoint.clone())
            }
            Endpoint::Tcp { listen, peers } => {
                let listener =
                    TcpListener::bind(listen).with_context(|| format!("listening on {listen}"))?;
                let actual = Endpoint::Tcp {
                    listen: listener.local_addr()?.to_string(),
                    peers: peers.clone(),
                };
                (Bound::Tcp(listener, peers.clone()), actual)
            }
        };
        self.listen(name, bound);
        Ok(actual)
    }

    pub fn close_endpoint(&self, name: &str) {
        let Some(mut listener) = self.listeners.lock().expect("listeners lock").remove(name) else {
            return;
        };
        listener.stop.store(true, Ordering::SeqCst);
        match &listener.wake {
            Wake::Unix(path) => {
                let _ = UnixStream::connect(path);
            }
            Wake::Tcp(addr) => {
                let _ = TcpStream::connect_timeout(addr, Duration::from_secs(1));
            }
        }
        if let Some(thread) = listener.thread.take() {
            let _ = thread.join();
        }
        if let Wake::Unix(path) = &listener.wake {
            let _ = std::fs::remove_file(path);
        }
    }

    fn listen(self: &Arc<Self>, name: &str, bound: Bound) {
        let stop = Arc::new(AtomicBool::new(false));
        let wake = match &bound {
            Bound::Unix(_, path) => Wake::Unix(path.clone()),
            Bound::Tcp(listener, _) => Wake::Tcp(
                listener
                    .local_addr()
                    .unwrap_or_else(|_| SocketAddr::from(([127, 0, 0, 1], 0))),
            ),
        };
        let vault = Arc::clone(self);
        let sandbox = name.to_string();
        let stopping = Arc::clone(&stop);
        let thread = std::thread::spawn(move || accept(vault, sandbox, bound, stopping));
        self.listeners.lock().expect("listeners lock").insert(
            name.to_string(),
            Listener {
                stop,
                wake,
                thread: Some(thread),
            },
        );
    }
}

fn accept(vault: Arc<Vault>, sandbox: String, bound: Bound, stop: Arc<AtomicBool>) {
    loop {
        match &bound {
            Bound::Unix(listener, _) => {
                let accepted = listener.accept();
                if stop.load(Ordering::SeqCst) {
                    return;
                }
                let Ok((conn, _)) = accepted else {
                    std::thread::sleep(Duration::from_millis(50));
                    continue;
                };
                let caller = peer(&conn);
                let user_ok = caller
                    .uid
                    .is_some_and(|uid| vault.config.owners.contains(&uid));
                if !user_ok {
                    vault.refused(&sandbox, caller.uid);
                }
                let _ = conn.set_read_timeout(Some(READ_TIMEOUT));
                spawn(&vault, &sandbox, conn, (true, user_ok), caller);
            }
            Bound::Tcp(listener, peers) => {
                let accepted = listener.accept();
                if stop.load(Ordering::SeqCst) {
                    return;
                }
                let Ok((conn, peer)) = accepted else {
                    std::thread::sleep(Duration::from_millis(50));
                    continue;
                };
                let _ = conn.set_read_timeout(Some(READ_TIMEOUT));
                let allowed = peers.contains(&canonical(peer.ip()));
                spawn(&vault, &sandbox, conn, (allowed, true), Caller::default());
            }
        }
    }
}

fn spawn(
    vault: &Arc<Vault>,
    sandbox: &str,
    conn: impl Read + Write + Send + 'static,
    (allowed, user_ok): (bool, bool),
    caller: Caller,
) {
    let vault = Arc::clone(vault);
    let sandbox = sandbox.to_string();
    std::thread::spawn(move || handle(&vault, &sandbox, conn, (allowed, user_ok), &caller));
}

#[cfg(target_os = "linux")]
fn peer(stream: &UnixStream) -> Caller {
    let mut cred = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let read = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut cred as *mut libc::ucred).cast(),
            &mut len,
        )
    };
    if read != 0 {
        return Caller::default();
    }
    Caller {
        pid: u32::try_from(cred.pid).ok().filter(|pid| *pid > 0),
        uid: Some(cred.uid),
    }
}

#[cfg(not(target_os = "linux"))]
fn peer(stream: &UnixStream) -> Caller {
    let fd = stream.as_raw_fd();
    let (mut uid, mut gid) = (0, 0);
    let read = unsafe { libc::getpeereid(fd, &mut uid, &mut gid) };
    let mut pid: libc::pid_t = 0;
    let mut len = std::mem::size_of::<libc::pid_t>() as libc::socklen_t;
    let got_pid = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_LOCAL,
            libc::LOCAL_PEERPID,
            (&mut pid as *mut libc::pid_t).cast(),
            &mut len,
        )
    };
    Caller {
        pid: (got_pid == 0)
            .then(|| u32::try_from(pid).ok())
            .flatten()
            .filter(|pid| *pid > 0),
        uid: (read == 0).then_some(uid),
    }
}

fn canonical(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(ip, IpAddr::V4),
        v4 => v4,
    }
}

fn handle(
    vault: &Arc<Vault>,
    sandbox: &str,
    mut conn: impl Read + Write,
    (allowed, user_ok): (bool, bool),
    caller: &Caller,
) {
    let request = match http::read_request(&mut conn) {
        Ok(request) => request,
        Err(error) => {
            let _ = http::write_error(&mut conn, 400, "bad_request", &error.to_string());
            return;
        }
    };
    if !user_ok {
        let _ = http::write_json(
            &mut conn,
            403,
            &json!({ "error": "denied", "reason": "wrong_user", "intent": null }),
        );
        return;
    }
    if sandbox == ADMIN {
        let (status, body) = admin::route(vault, &request, caller);
        let _ = http::write_json(&mut conn, status, &body);
    } else {
        gateway::handle(vault, sandbox, allowed, request, &mut conn);
    }
}

pub fn dir_mode(socket_mode: u32) -> u32 {
    if socket_mode & 0o070 == 0 {
        0o700
    } else {
        0o710
    }
}

fn make_dirs(dir: &Path, mode: u32) -> Result<()> {
    if dir.exists() {
        return Ok(());
    }
    if let Some(parent) = dir.parent() {
        make_dirs(parent, mode)?;
    }
    std::fs::DirBuilder::new()
        .mode(mode)
        .create(dir)
        .with_context(|| format!("making {}", dir.display()))?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(mode))
        .with_context(|| format!("securing {}", dir.display()))
}

pub fn bind_unix(path: &Path, mode: u32) -> Result<UnixListener> {
    if let Some(parent) = path.parent() {
        make_dirs(parent, dir_mode(mode))?;
    }
    if let Ok(meta) = std::fs::symlink_metadata(path) {
        if !meta.file_type().is_socket() {
            bail!("{} is there and is not a socket", path.display());
        }
        std::fs::remove_file(path)?;
    }
    let listener =
        UnixListener::bind(path).with_context(|| format!("listening on {}", path.display()))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
    Ok(listener)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(path: &Path) -> u32 {
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn a_group_socket_gets_folders_the_group_can_pass_through() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir
            .path()
            .join("sandboxes")
            .join("box")
            .join("gateway.sock");
        let _listener = bind_unix(&path, 0o660).unwrap();
        assert_eq!(mode(&path), 0o660);
        assert_eq!(mode(&dir.path().join("sandboxes")), 0o710);
        assert_eq!(mode(&dir.path().join("sandboxes").join("box")), 0o710);
    }

    #[test]
    fn a_private_socket_gets_private_folders() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a").join("b").join("gateway.sock");
        let _listener = bind_unix(&path, 0o600).unwrap();
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(&dir.path().join("a")), 0o700);
        assert_eq!(mode(&dir.path().join("a").join("b")), 0o700);
    }

    #[test]
    fn sockets_are_shared_with_the_admin_group_only_on_linux() {
        let config = Config::new("/s".into(), "/s/admin.sock".into());
        let expected = if cfg!(target_os = "linux") {
            0o660
        } else {
            0o600
        };
        assert_eq!(
            (config.admin_mode, config.socket_mode),
            (expected, expected)
        );
    }
}
