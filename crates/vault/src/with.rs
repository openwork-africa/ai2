use crate::cli::{code, exit, warn, Admin};
use crate::policy::encode_percent;
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;

pub const IMAGE: &str = "debian:trixie-slim";
pub const WORK: &str = "/work";
pub const HOST: &str = "host";
pub const TOKEN_ENV: &str = "AI2_VAULT_TOKEN";

const FORWARDED: [libc::c_int; 3] = [libc::SIGINT, libc::SIGTERM, libc::SIGHUP];

static PENDING: AtomicI32 = AtomicI32::new(0);

extern "C" fn on_signal(signal: libc::c_int) {
    PENDING.store(signal, Ordering::SeqCst);
}

pub struct With {
    pub keys: Vec<String>,
    pub sandbox: Option<String>,
    pub image: String,
    pub intent: Option<String>,
    pub mode: String,
    pub methods: Option<String>,
    pub url_prefixes: Vec<String>,
    pub ttl: Option<String>,
    pub command: Vec<String>,
}

pub trait Machine {
    fn containers(&self) -> bool;
    fn terminal(&self) -> bool;
    fn folder(&self) -> Result<PathBuf>;
    fn run(&self, command: &mut Command, stop: Option<&mut Command>) -> Result<i32>;
    fn quiet(&self, command: &mut Command);
}

pub struct System;

impl Machine for System {
    fn containers(&self) -> bool {
        cfg!(target_os = "macos") && on_path("container")
    }

    fn terminal(&self) -> bool {
        unsafe { libc::isatty(libc::STDIN_FILENO) == 1 }
    }

    fn folder(&self) -> Result<PathBuf> {
        Ok(std::env::current_dir()?)
    }

    fn run(&self, command: &mut Command, stop: Option<&mut Command>) -> Result<i32> {
        let program = command.get_program().to_string_lossy().into_owned();
        let mut child = command
            .spawn()
            .map_err(|error| anyhow!("could not run {program}: {error}"))?;
        wait(&mut child, stop)
    }

    fn quiet(&self, command: &mut Command) {
        quiet(command);
    }
}

fn quiet(command: &mut Command) {
    let _ = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn on_path(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|folder| {
            std::fs::metadata(folder.join(program))
                .is_ok_and(|meta| meta.is_file() && meta.mode() & 0o111 != 0)
        })
    })
}

enum Place {
    Named(String),
    Throwaway { name: String, socket: PathBuf },
}

impl Place {
    fn name(&self) -> &str {
        match self {
            Place::Named(name) | Place::Throwaway { name, .. } => name,
        }
    }
}

struct Minted {
    id: String,
    token: String,
    revoke: String,
}

fn say(what: &str) {
    eprintln!("ai2 vault: {what}");
}

fn failed(error: anyhow::Error) -> i32 {
    say(&format!("{error:#}"));
    exit::ERROR
}

pub fn run(admin: &Admin, with: With) -> i32 {
    run_on(admin, with, &System)
}

pub fn run_on(admin: &Admin, with: With, machine: &dyn Machine) -> i32 {
    if with.mode == "agent" && with.intent.as_deref().is_none_or(|i| i.trim().is_empty()) {
        say("an agent token needs --intent: what the command is for");
        return exit::USAGE;
    }
    if with.command.is_empty() {
        say("give the command to run after --");
        return exit::USAGE;
    }
    let before = catch();
    let code = placed(admin, &with, machine);
    restore(&before);
    code
}

fn placed(admin: &Admin, with: &With, machine: &dyn Machine) -> i32 {
    let held = admin.hold();
    let place = match place(admin, with, machine) {
        Ok(Ok(place)) => place,
        Ok(Err(code)) => return code,
        Err(error) => return failed(error),
    };
    let minted = mint(admin, with, place.name());
    drop(held);
    let code = match minted {
        Ok(Ok(minted)) => {
            let ran =
                interrupted().map_or_else(|| inside(with, machine, &place, &minted, admin), Ok);
            revoke(admin, &minted);
            ran.unwrap_or_else(failed)
        }
        Ok(Err(code)) => code,
        Err(error) => failed(error),
    };
    if let Place::Throwaway { name, .. } = &place {
        machine.quiet(Command::new("container").args(["delete", "--force", name]));
        remove(admin, name);
    }
    code
}

fn interrupted() -> Option<i32> {
    match PENDING.load(Ordering::SeqCst) {
        0 => None,
        signal => {
            say("interrupted before the command started");
            Some(128 + signal)
        }
    }
}

fn place(admin: &Admin, with: &With, machine: &dyn Machine) -> Result<Result<Place, i32>> {
    if let Some(name) = &with.sandbox {
        return Ok(Ok(Place::Named(name.clone())));
    }
    if !machine.containers() {
        say(setup::NOT_SAFE);
        return Ok(host(admin)?.map(|()| Place::Named(HOST.into())));
    }
    let name = format!("with-{}", random_id()?);
    let (status, value) = admin.call("POST", "/sandboxes", Some(json!({ "name": name })), true)?;
    if status != 200 {
        say(value["message"]
            .as_str()
            .unwrap_or("the vault made no sandbox"));
        return Ok(Err(code(status)));
    }
    let socket = value["sandbox"]["path"]
        .as_str()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("the sandbox {name} has no socket"));
    match socket {
        Ok(socket) => Ok(Ok(Place::Throwaway { name, socket })),
        Err(error) => {
            remove(admin, &name);
            Err(error)
        }
    }
}

fn host(admin: &Admin) -> Result<Result<(), i32>> {
    if sandboxes(admin)?.iter().any(|s| s["name"] == HOST) {
        return Ok(Ok(()));
    }
    say("making the host sandbox");
    let (status, value) = admin.call("POST", "/sandboxes", Some(json!({ "name": HOST })), true)?;
    if status == 200 || status == 409 {
        return Ok(Ok(()));
    }
    say(value["message"]
        .as_str()
        .unwrap_or("the vault made no host sandbox"));
    Ok(Err(code(status)))
}

fn random_id() -> Result<String> {
    let mut bytes = [0u8; 4];
    getrandom::fill(&mut bytes).map_err(|error| anyhow!("no randomness: {error}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn mint(admin: &Admin, with: &With, sandbox: &str) -> Result<Result<Minted, i32>> {
    let body = json!({
        "keys": with.keys,
        "sandbox": sandbox,
        "mode": with.mode,
        "methods": with.methods,
        "url_prefixes": with.url_prefixes,
        "intent": with.intent,
        "ttl": with.ttl,
    });
    let (status, value) = admin.call("POST", "/tokens", Some(body), true)?;
    warn(&value);
    if status != 200 {
        say(value["message"]
            .as_str()
            .unwrap_or("the vault made no token"));
        return Ok(Err(code(status)));
    }
    let text = |field: &str| {
        value[field]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| anyhow!("the vault's answer has no {field}"))
    };
    let minted = Minted {
        id: text("id")?,
        token: text("token")?,
        revoke: text("revoke")?,
    };
    say(&format!(
        "token {} for {} in the sandbox {sandbox}, revoked when the command exits",
        minted.id,
        with.keys.join(", "),
    ));
    Ok(Ok(minted))
}

fn sandboxes(admin: &Admin) -> Result<Vec<Value>> {
    let (_, value) = admin.call("GET", "/sandboxes", None, false)?;
    Ok(value["sandboxes"].as_array().cloned().unwrap_or_default())
}

fn endpoint(admin: &Admin, sandbox: &str) -> Result<String> {
    let found = sandboxes(admin)?
        .into_iter()
        .find(|s| s["name"] == sandbox)
        .ok_or_else(|| anyhow!("there is no sandbox {sandbox}"))?;
    endpoint_of(&found).ok_or_else(|| anyhow!("the sandbox {sandbox} has no endpoint"))
}

fn endpoint_of(sandbox: &Value) -> Option<String> {
    match sandbox["transport"].as_str()? {
        "unix" => Some(format!("unix:{}", sandbox["path"].as_str()?)),
        "tcp" => Some(format!("http://{}", sandbox["listen"].as_str()?)),
        _ => None,
    }
}

fn inside(
    with: &With,
    machine: &dyn Machine,
    place: &Place,
    minted: &Minted,
    admin: &Admin,
) -> Result<i32> {
    let (mut command, mut stop) = match place {
        Place::Named(name) => (on_host(with, &endpoint(admin, name)?), None),
        Place::Throwaway { name, socket } => {
            let folder = machine.folder()?;
            say(&format!(
                "running in a fresh {} container {name}, with {} at {WORK}",
                with.image,
                folder.display()
            ));
            let mut stop = Command::new("container");
            stop.args(["stop", name]);
            (
                in_container(with, name, socket, &folder, machine.terminal())?,
                Some(stop),
            )
        }
    };
    command.env(TOKEN_ENV, &minted.token);
    machine.run(&mut command, stop.as_mut())
}

fn on_host(with: &With, endpoint: &str) -> Command {
    let mut command = Command::new(&with.command[0]);
    command
        .args(&with.command[1..])
        .env(setup::sandbox::ENDPOINT_ENV, endpoint);
    command
}

pub fn in_container(
    with: &With,
    name: &str,
    socket: &Path,
    folder: &Path,
    terminal: bool,
) -> Result<Command> {
    let shown = folder.display().to_string();
    if !folder.is_absolute() || shown.contains(':') || shown.contains(',') {
        bail!(
            "the current folder {shown} cannot be mounted: it must be absolute, without ':' or ','"
        );
    }
    let mut command = Command::new("container");
    command
        .args(["run", "--rm", "--name", name, "--interactive"])
        .args(terminal.then_some("--tty"))
        .args(setup::sandbox::attach(socket)?)
        .args(["--volume", &format!("{shown}:{WORK}")])
        .args(["--workdir", WORK, "--env", TOKEN_ENV, &with.image])
        .args(&with.command);
    Ok(command)
}

fn catch() -> Vec<libc::sighandler_t> {
    PENDING.store(0, Ordering::SeqCst);
    let handler = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
    FORWARDED
        .iter()
        .map(|&signal| unsafe { libc::signal(signal, handler) })
        .collect()
}

fn restore(before: &[libc::sighandler_t]) {
    for (&signal, &handler) in FORWARDED.iter().zip(before) {
        unsafe { libc::signal(signal, handler) };
    }
}

fn wait(child: &mut Child, mut stop: Option<&mut Command>) -> Result<i32> {
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status
                .code()
                .or_else(|| status.signal().map(|signal| 128 + signal))
                .unwrap_or(exit::ERROR));
        }
        let signal = PENDING.swap(0, Ordering::SeqCst);
        match (signal, stop.as_deref_mut()) {
            (0, _) => {}
            (_, Some(stop)) => quiet(stop),
            (signal, None) => unsafe {
                libc::kill(child.id() as libc::pid_t, signal);
            },
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn revoke(admin: &Admin, minted: &Minted) {
    let revoked = admin.call_with(
        "DELETE",
        &format!("/tokens/{}", encode_percent(&minted.id)),
        None,
        false,
        &[("X-Vault-Revoke", &minted.revoke)],
    );
    match revoked {
        Ok((200, _)) => say(&format!("revoked token {}", minted.id)),
        Ok((_, value)) => say(&format!(
            "could not revoke token {}: {}; revoke it with ai2 vault token revoke --id {}",
            minted.id,
            value["message"].as_str().unwrap_or("no answer"),
            minted.id
        )),
        Err(error) => say(&format!(
            "could not revoke token {}: {error:#}; it ends when its ttl does",
            minted.id
        )),
    }
}

fn remove(admin: &Admin, name: &str) {
    let removed = admin.call(
        "DELETE",
        &format!("/sandboxes/{}", encode_percent(name)),
        None,
        true,
    );
    let why = match removed {
        Ok((200, _)) => return say(&format!("removed the sandbox {name}")),
        Ok((_, value)) => value["message"].as_str().unwrap_or("no answer").to_string(),
        Err(error) => format!("{error:#}"),
    };
    say(&format!(
        "could not remove the sandbox {name}: {why}; its token is revoked, remove it with ai2 vault sandbox remove --name {name}"
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::Daemon;
    use std::cell::RefCell;

    #[derive(Default)]
    struct Fake {
        containers: bool,
        ran: RefCell<Vec<Ran>>,
        quiet: RefCell<Vec<Vec<String>>>,
        answer: i32,
    }

    #[derive(Debug)]
    struct Ran {
        program: String,
        args: Vec<String>,
        stop: Option<Vec<String>>,
        token: Option<String>,
        endpoint: Option<String>,
    }

    fn strings(command: &Command) -> Vec<String> {
        command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    fn env(command: &Command, name: &str) -> Option<String> {
        command
            .get_envs()
            .find(|(key, _)| *key == name)
            .and_then(|(_, value)| value)
            .map(|value| value.to_string_lossy().into_owned())
    }

    impl Machine for Fake {
        fn containers(&self) -> bool {
            self.containers
        }
        fn terminal(&self) -> bool {
            true
        }
        fn folder(&self) -> Result<PathBuf> {
            Ok("/Users/ada/project".into())
        }
        fn run(&self, command: &mut Command, stop: Option<&mut Command>) -> Result<i32> {
            self.ran.borrow_mut().push(Ran {
                program: command.get_program().to_string_lossy().into_owned(),
                args: strings(command),
                stop: stop.map(|stop| strings(stop)),
                token: env(command, TOKEN_ENV),
                endpoint: env(command, "AI2_VAULT_ENDPOINT"),
            });
            Ok(self.answer)
        }
        fn quiet(&self, command: &mut Command) {
            let mut all = vec![command.get_program().to_string_lossy().into_owned()];
            all.extend(strings(command));
            self.quiet.borrow_mut().push(all);
        }
    }

    fn with(sandbox: Option<&str>, command: &[&str]) -> With {
        With {
            keys: vec!["svc".into()],
            sandbox: sandbox.map(str::to_string),
            image: IMAGE.into(),
            intent: Some("read the test data".into()),
            mode: "agent".into(),
            methods: None,
            url_prefixes: Vec::new(),
            ttl: None,
            command: command.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn sandboxes(daemon: &Daemon) -> Vec<String> {
        daemon.names("/sandboxes", "sandboxes", "name")
    }

    fn tokens(daemon: &Daemon) -> usize {
        daemon.names("/tokens", "tokens", "id").len()
    }

    #[test]
    fn the_container_command_mounts_the_socket_and_the_folder_and_passes_the_token_by_name() {
        let command = in_container(
            &with(None, &["sh", "-c", "echo hi"]),
            "with-0a1b2c3d",
            Path::new("/Users/ada/.ai2/vault/sandboxes/with-0a1b2c3d/gateway.sock"),
            Path::new("/Users/ada/project"),
            true,
        )
        .unwrap();
        assert_eq!(command.get_program(), "container");
        assert_eq!(
            strings(&command),
            [
                "run",
                "--rm",
                "--name",
                "with-0a1b2c3d",
                "--interactive",
                "--tty",
                "--volume",
                "/Users/ada/.ai2/vault/sandboxes/with-0a1b2c3d/gateway.sock:/run/ai2-vault.sock",
                "--env",
                "AI2_VAULT_ENDPOINT=unix:/run/ai2-vault.sock",
                "--volume",
                "/Users/ada/project:/work",
                "--workdir",
                "/work",
                "--env",
                "AI2_VAULT_TOKEN",
                "debian:trixie-slim",
                "sh",
                "-c",
                "echo hi"
            ]
        );
        let piped = in_container(
            &with(None, &["true"]),
            "w",
            Path::new("/s.sock"),
            Path::new("/p"),
            false,
        )
        .unwrap();
        assert!(!strings(&piped).contains(&"--tty".to_string()));
        assert!(in_container(
            &with(None, &["true"]),
            "w",
            Path::new("/s.sock"),
            Path::new("/a:b"),
            false
        )
        .is_err());
    }

    #[test]
    fn without_a_sandbox_the_command_runs_in_a_throwaway_container_that_is_cleaned_up() {
        let daemon = Daemon::start();
        daemon.put("svc");
        let opts = daemon.options();
        let admin = Admin::new(&opts);
        let fake = Fake {
            containers: true,
            answer: 5,
            ..Fake::default()
        };
        assert_eq!(run_on(&admin, with(None, &["sh", "-c", "x"]), &fake), 5);
        let ran = fake.ran.borrow();
        assert_eq!(ran.len(), 1);
        assert_eq!(ran[0].program, "container");
        let name = ran[0].args[3].clone();
        assert!(name.starts_with("with-") && name.len() == 13, "{name}");
        assert!(ran[0]
            .token
            .as_deref()
            .is_some_and(|t| t.starts_with("vt_")));
        assert_eq!(ran[0].stop, Some(vec!["stop".to_string(), name.clone()]));
        assert!(ran[0]
            .args
            .iter()
            .any(|a| a.ends_with(&format!("{name}/gateway.sock:/run/ai2-vault.sock"))));
        assert_eq!(
            *fake.quiet.borrow(),
            [vec![
                "container".to_string(),
                "delete".into(),
                "--force".into(),
                name
            ]]
        );
        assert!(sandboxes(&daemon).is_empty());
        assert_eq!(tokens(&daemon), 0);
    }

    #[test]
    fn without_containers_it_runs_on_the_host_and_makes_the_host_sandbox_when_missing() {
        let daemon = Daemon::start();
        daemon.put("svc");
        let opts = daemon.options();
        let admin = Admin::new(&opts);
        let fake = Fake::default();
        assert_eq!(run_on(&admin, with(None, &["true", "a"]), &fake), 0);
        assert_eq!(run_on(&admin, with(None, &["true"]), &fake), 0);
        let ran = fake.ran.borrow();
        assert_eq!(ran[0].program, "true");
        assert_eq!(ran[0].args, ["a"]);
        assert!(ran[0]
            .endpoint
            .as_deref()
            .is_some_and(|e| e.starts_with("unix:") && e.ends_with("host/gateway.sock")));
        assert!(ran[0].token.is_some());
        assert_eq!(ran[0].stop, None);
        assert_eq!(sandboxes(&daemon), ["host"]);
        assert!(fake.quiet.borrow().is_empty());
        assert_eq!(tokens(&daemon), 0);
    }

    #[test]
    fn a_named_sandbox_runs_on_the_host_even_where_containers_work() {
        let daemon = Daemon::start();
        daemon.put("svc");
        let opts = daemon.options();
        let admin = Admin::new(&opts);
        let fake = Fake {
            containers: true,
            ..Fake::default()
        };
        assert_eq!(
            run_on(&admin, with(Some("nope"), &["true"]), &fake),
            exit::ERROR
        );
        assert!(fake.ran.borrow().is_empty());
        assert_eq!(
            daemon
                .call("POST", "/sandboxes", Some(json!({ "name": "box" })))
                .0,
            200
        );
        assert_eq!(run_on(&admin, with(Some("box"), &["true"]), &fake), 0);
        assert_eq!(fake.ran.borrow()[0].program, "true");
        assert_eq!(sandboxes(&daemon), ["box"]);
        assert_eq!(tokens(&daemon), 0);
    }

    #[test]
    #[ignore]
    fn a_command_runs_in_a_fresh_apple_container_with_the_gateway_and_the_folder() {
        let daemon = Daemon::start();
        daemon.put("svc");
        let opts = daemon.options();
        let admin = Admin::new(&opts);
        let folder = tempfile::Builder::new()
            .prefix("ai2w")
            .tempdir_in("/tmp")
            .unwrap();
        std::fs::write(folder.path().join("seen"), "here").unwrap();
        let script = r#"test -S /run/ai2-vault.sock && test "$AI2_VAULT_ENDPOINT" = unix:/run/ai2-vault.sock && case "$AI2_VAULT_TOKEN" in vt_*) ;; *) exit 1;; esac && test "$(pwd)" = /work && test "$(cat seen)" = here && exit 5"#;
        struct In(PathBuf);
        impl Machine for In {
            fn containers(&self) -> bool {
                System.containers()
            }
            fn terminal(&self) -> bool {
                false
            }
            fn folder(&self) -> Result<PathBuf> {
                Ok(self.0.clone())
            }
            fn run(&self, command: &mut Command, stop: Option<&mut Command>) -> Result<i32> {
                System.run(command.stdin(Stdio::null()), stop)
            }
            fn quiet(&self, command: &mut Command) {
                System.quiet(command)
            }
        }
        let machine = In(std::fs::canonicalize(folder.path()).unwrap());
        assert!(machine.containers(), "this test needs Apple container");
        assert_eq!(
            run_on(&admin, with(None, &["sh", "-c", script]), &machine),
            5
        );
        assert!(sandboxes(&daemon).is_empty());
        assert_eq!(tokens(&daemon), 0);
        let listed = Command::new("container")
            .args(["list", "--all"])
            .output()
            .unwrap();
        assert!(!String::from_utf8_lossy(&listed.stdout).contains("with-"));
    }
}
