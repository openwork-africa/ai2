use crate::cli::{code, exit, warn, Admin};
use crate::policy::encode_percent;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::os::unix::process::ExitStatusExt;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;

const FORWARDED: [libc::c_int; 3] = [libc::SIGINT, libc::SIGTERM, libc::SIGHUP];

static PENDING: AtomicI32 = AtomicI32::new(0);

extern "C" fn on_signal(signal: libc::c_int) {
    PENDING.store(signal, Ordering::SeqCst);
}

pub struct With {
    pub keys: Vec<String>,
    pub sandbox: String,
    pub intent: Option<String>,
    pub mode: String,
    pub methods: Option<String>,
    pub url_prefixes: Vec<String>,
    pub ttl: Option<String>,
    pub command: Vec<String>,
}

struct Minted {
    id: String,
    token: String,
    revoke: String,
}

fn say(what: &str) {
    eprintln!("ai2 vault: {what}");
}

pub fn run(admin: &Admin, with: With) -> i32 {
    if with.mode == "agent" && with.intent.as_deref().is_none_or(|i| i.trim().is_empty()) {
        say("an agent token needs --intent: what the command is for");
        return exit::USAGE;
    }
    if with.command.is_empty() {
        say("give the command to run after --");
        return exit::USAGE;
    }
    let minted = match mint(admin, &with) {
        Ok(Ok(minted)) => minted,
        Ok(Err(code)) => return code,
        Err(error) => {
            say(&format!("{error:#}"));
            return exit::ERROR;
        }
    };
    let ran = endpoint(admin, &with.sandbox).and_then(|endpoint| spawn(&with, &minted, &endpoint));
    revoke(admin, &minted);
    match ran {
        Ok(code) => code,
        Err(error) => {
            say(&format!("{error:#}"));
            exit::ERROR
        }
    }
}

fn mint(admin: &Admin, with: &With) -> Result<Result<Minted, i32>> {
    let body = json!({
        "keys": with.keys,
        "sandbox": with.sandbox,
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
        "token {} for {} in the sandbox {}, revoked when the command exits",
        minted.id,
        with.keys.join(", "),
        with.sandbox
    ));
    Ok(Ok(minted))
}

fn endpoint(admin: &Admin, sandbox: &str) -> Result<String> {
    let (_, value) = admin.call("GET", "/sandboxes", None, false)?;
    let found = value["sandboxes"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|s| s["name"] == sandbox)
        .ok_or_else(|| anyhow!("there is no sandbox {sandbox}"))?;
    endpoint_of(found).ok_or_else(|| anyhow!("the sandbox {sandbox} has no endpoint"))
}

fn endpoint_of(sandbox: &Value) -> Option<String> {
    match sandbox["transport"].as_str()? {
        "unix" => Some(format!("unix:{}", sandbox["path"].as_str()?)),
        "tcp" => Some(format!("http://{}", sandbox["listen"].as_str()?)),
        _ => None,
    }
}

fn spawn(with: &With, minted: &Minted, endpoint: &str) -> Result<i32> {
    let before = catch();
    let waited = Command::new(&with.command[0])
        .args(&with.command[1..])
        .env("AI2_VAULT_TOKEN", &minted.token)
        .env("AI2_VAULT_ENDPOINT", endpoint)
        .spawn()
        .map_err(|error| anyhow!("could not run {}: {error}", with.command[0]))
        .and_then(|mut child| wait(&mut child));
    restore(&before);
    waited
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

fn wait(child: &mut Child) -> Result<i32> {
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status
                .code()
                .or_else(|| status.signal().map(|signal| 128 + signal))
                .unwrap_or(exit::ERROR));
        }
        let signal = PENDING.swap(0, Ordering::SeqCst);
        if signal != 0 {
            unsafe { libc::kill(child.id() as libc::pid_t, signal) };
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
