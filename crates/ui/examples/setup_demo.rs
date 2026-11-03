use anyhow::{bail, Result};
use serde_json::{json, Map, Value};
use setup::host::Host;
use setup::runner::fake::Fake;
use setup::runner::{Cmd, Ran, Runner};
use setup::{State, Status, NOT_SAFE};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use ui::setup::Engine;

struct Script<'a> {
    runner: &'a dyn Runner,
    report: Map<String, Value>,
    ok: bool,
    stopped: bool,
}

fn pause(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

fn shell(lines: &[&str], gap: f32) -> Cmd {
    let script = lines
        .iter()
        .map(|line| format!("echo '{line}'; sleep {gap}"))
        .collect::<Vec<_>>()
        .join("; ");
    Cmd::new("sh", &["-c", &script]).shown()
}

impl<'a> Script<'a> {
    fn new(runner: &'a dyn Runner) -> Self {
        Self {
            runner,
            report: Map::new(),
            ok: true,
            stopped: false,
        }
    }

    fn step(&mut self, id: &str, work: impl FnOnce(&dyn Runner) -> Result<State>) {
        let state = if self.stopped {
            State::new(Status::Skipped)
        } else {
            self.runner.step_started(id);
            match work(self.runner) {
                Ok(state) => state,
                Err(error) => {
                    self.stopped = true;
                    State::new(Status::Failed).with("error", format!("{error:#}"))
                }
            }
        };
        self.runner.step_finished(id, &state);
        for warning in &state.warnings {
            self.runner.say(warning);
        }
        self.ok &= matches!(state.status, Status::Ready | Status::Unsupported);
        self.report.insert(id.into(), state.to_json());
    }

    fn end(self) -> Result<(bool, Value)> {
        Ok((
            self.ok,
            json!({ "ok": self.ok, "check": false, "steps": self.report }),
        ))
    }
}

fn brew(runner: &dyn Runner) -> Result<State> {
    runner.say("installing Apple container with Homebrew");
    runner.run(&shell(
        &[
            "==> Fetching downloads for: container",
            "==> Downloading https://ghcr.io/v2/homebrew/core/container/manifests/0.6.0",
            "==> Pouring container--0.6.0.arm64_tahoe.bottle.tar.gz",
            "/opt/homebrew/Cellar/container/0.6.0: 42 files, 61.2MB",
        ],
        0.35,
    ))?;
    runner.run(&shell(
        &[
            "Verifying apiserver is running...",
            "Starting kernel install",
        ],
        0.3,
    ))?;
    Ok(State::new(Status::Ready).with("runtime", "apple-container"))
}

fn keychain(runner: &dyn Runner) -> Result<State> {
    runner.say("making the vault master key");
    pause(400);
    runner.run(&shell(
        &["$ security add-generic-password -s ai2-vault -a master -T /Users/ada/.local/bin/ai2 …"],
        0.2,
    ))?;
    pause(700);
    Ok(State::new(Status::Ready).with("store", "keychain"))
}

fn touch_id(runner: &dyn Runner) -> Result<State> {
    runner.say("Touch ID is available; the vault asks for it on every change");
    pause(500);
    Ok(State::new(Status::Ready).with("presence", "touch-id"))
}

fn launchd(runner: &dyn Runner) -> Result<State> {
    runner.say("writing ~/Library/LaunchAgents/africa.openwork.ai2-vault.plist");
    pause(400);
    runner.run(&shell(
        &["$ launchctl bootstrap gui/501 ~/Library/LaunchAgents/africa.openwork.ai2-vault.plist"],
        0.2,
    ))?;
    runner.say("adding the vault sandbox host, for apps that run on this machine; the vault asks for Touch ID or your password for this one change");
    pause(600);
    Ok(State::new(Status::Ready)
        .with("service", "launchd")
        .with("host_sandbox", "created"))
}

fn systemd(runner: &dyn Runner) -> Result<State> {
    runner.say("installing the vault service as ai2-vault.service");
    runner.run(&Cmd::new("systemctl", &["daemon-reload"]).root())?;
    runner.run(&shell(
        &["Created symlink /etc/systemd/system/multi-user.target.wants/ai2-vault.service → /etc/systemd/system/ai2-vault.service."],
        0.3,
    ))?;
    Ok(State::new(Status::Ready)
        .with("service", "systemd")
        .warn("Log out and back in so ada can reach the vault as a member of ai2-vault-admin."))
}

fn sandboxes(runner: &dyn Runner) -> Result<State> {
    runner.say("checking systemd-run and the socket folder");
    pause(600);
    Ok(State::new(Status::Ready).with("runtime", "systemd"))
}

fn creds(runner: &dyn Runner) -> Result<State> {
    runner.say("making the vault master key");
    runner.say("encrypting the vault master key to /etc/ai2-vault/master.cred with systemd-creds, through sudo");
    runner.run(&Cmd::new("install", &["-d", "-m", "0755", "/etc/ai2-vault"]).root())?;
    pause(700);
    Ok(State::new(Status::Ready).with("store", "systemd-creds"))
}

fn passphrase(runner: &dyn Runner) -> Result<State> {
    runner.say("choose the vault passphrase; it is asked for every change to the vault");
    runner.ask_passphrase("Vault passphrase: ", "The same again: ")?;
    runner.say("hashing the passphrase with Argon2id");
    pause(900);
    Ok(State::new(Status::Ready).with("presence", "passphrase"))
}

fn forever(runner: &dyn Runner, line: &str) -> Result<State> {
    runner.say(line);
    loop {
        pause(60_000);
    }
}

fn scenario(name: &str, runner: &dyn Runner) -> Result<(bool, Value)> {
    let mut script = Script::new(runner);
    match name {
        "running" => {
            script.step("containers", brew);
            script.step("master-key", |runner| {
                runner.say("making the vault master key");
                runner.run(&shell(
                    &["$ security add-generic-password -s ai2-vault -a master …"],
                    0.1,
                ))?;
                forever(runner, "waiting for the login keychain")
            });
        }
        "input" | "sudo" => {
            script.step("containers", sandboxes);
            script.step("master-key", creds);
            script.step("presence", passphrase);
            script.step("daemon", systemd);
        }
        "failed" => {
            script.step("containers", |runner| {
                runner.say("systemd 249 has no socket folders for agent units");
                pause(300);
                Ok(State::new(Status::Unsupported).warn(NOT_SAFE))
            });
            script.step("master-key", |runner| {
                runner.say("making the vault master key");
                pause(800);
                bail!("the vault keeps its master key with systemd-creds, which needs systemd 250 or later; this machine has systemd 249. Upgrade systemd, then run ai2 setup again.")
            });
            script.step("presence", passphrase);
            script.step("daemon", systemd);
        }
        "unsupported" => {
            script.step("containers", |runner| {
                runner.say("Apple container needs macOS 26 on Apple silicon");
                pause(300);
                Ok(State::new(Status::Unsupported).warn(NOT_SAFE))
            });
            script.step("master-key", keychain);
            script.step("presence", touch_id);
            script.step("daemon", launchd);
        }
        "warned" => {
            script.step("containers", sandboxes);
            script.step("master-key", creds);
            script.step("presence", |runner| {
                pause(300);
                runner.say("the passphrase is already set");
                Ok(State::new(Status::Ready).with("presence", "passphrase"))
            });
            script.step("daemon", systemd);
        }
        _ => {
            script.step("containers", brew);
            script.step("master-key", keychain);
            script.step("presence", touch_id);
            script.step("daemon", launchd);
        }
    }
    script.end()
}

fn host(name: &str) -> Host {
    match name {
        "input" | "sudo" | "failed" | "warned" => {
            Host::linux(Some(if name == "failed" { 249 } else { 255 }))
        }
        "unsupported" => Host::mac(15, false),
        _ => Host::mac(26, true),
    }
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let name = args.next().unwrap_or_else(|| "live".into());
    let port: u16 = args.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let home = std::env::temp_dir().join(format!("ai2-setup-demo-{}", std::process::id()));
    let options = ui::Options {
        admin: PathBuf::from(&home).join("admin.sock"),
        home,
    };
    let sudo = name == "sudo";
    let shown = host(&name);
    let engine = Engine {
        host: Arc::new(move || shown.clone()),
        runner: Arc::new(move || {
            let fake = Fake::default();
            if sudo {
                fake.once("sudo -n -v", Ran::failed());
            }
            Box::new(fake)
        }),
        live: true,
        drive: Arc::new(move |runner, _| scenario(&name, runner)),
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let server = ui::Server::bind(port, &options).await?;
        ui::setup::serve(server, engine, false, false).await
    })?;
    Ok(())
}
