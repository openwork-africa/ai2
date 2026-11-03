use crate::admin::HOST_SANDBOX;
use crate::files::{self, ADMIN_GROUP, SERVICE_BIN, UNIT, UNIT_PATH, USER};
use crate::host::Os;
use crate::runner::{Cmd, Ran};
use crate::{Context, State, Status, Step, NOT_SAFE};
use anyhow::{bail, Result};
use keys::{LINUX_CRED, LINUX_PASSPHRASE, LINUX_STATE, SYSTEMD_CREDS};
use serde_json::Value;
use std::path::Path;

pub const RELEASES: &str = "https://github.com/apple/container/releases";
pub const CONTAINER_SIGNER: &str =
    "Developer ID Installer: Apple Inc. - Containerization (UPBK2H6LZM)";
pub const MACOS_CONTAINERS: u32 = 26;

fn path_str(path: &Path) -> String {
    path.display().to_string()
}

fn found(ctx: &Context, program: &str) -> Result<Option<String>> {
    let ran = ctx.runner.run(&Cmd::new(
        "/bin/sh",
        &["-c", &format!("command -v {program}")],
    ))?;
    let path = ran.out.trim();
    Ok((ran.ok && !path.is_empty()).then(|| path.to_string()))
}

fn must(ran: Ran, what: &str) -> Result<Ran> {
    if !ran.ok {
        bail!("{what} failed");
    }
    Ok(ran)
}

fn root_write(ctx: &Context, path: &str, contents: &str, umask: &str) -> Result<()> {
    let script = format!(r#"umask {umask} && cat > "$1.new" && mv "$1.new" "$1""#);
    let ran = ctx.runner.run(
        &Cmd::new("sh", &["-c", &script, "sh", path])
            .root()
            .input(contents.as_bytes()),
    )?;
    must(ran, &format!("writing {path}")).map(|_| ())
}

pub fn supports_containers(os: &Os) -> bool {
    match os {
        Os::Macos { version, arm64 } => *arm64 && *version >= MACOS_CONTAINERS,
        Os::Linux { systemd } => systemd.is_some_and(|version| version >= SYSTEMD_CREDS),
        Os::Other { .. } => false,
    }
}

pub fn latest_version(effective_url: &str) -> Option<String> {
    let version = effective_url
        .trim()
        .rsplit_once("/tag/")?
        .1
        .trim_matches('/');
    (!version.is_empty()
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-'))
    .then(|| version.to_string())
}

pub fn pkg_url(version: &str) -> String {
    format!("{RELEASES}/download/{version}/container-{version}-installer-signed.pkg")
}

pub fn signed_by_apple(check_signature: &str) -> bool {
    check_signature.contains(CONTAINER_SIGNER)
        && check_signature.contains("Notarization: trusted by the Apple notary service")
}

pub struct Containers;

impl Containers {
    fn install(&self, ctx: &Context) -> Result<()> {
        if found(ctx, "brew")?.is_some() {
            ctx.runner.say("installing Apple container with Homebrew");
            must(
                ctx.runner
                    .run(&Cmd::new("brew", &["install", "container"]).shown())?,
                "brew install container",
            )?;
            return Ok(());
        }
        let latest = ctx.runner.run(&Cmd::new(
            "curl",
            &[
                "-fsSLI",
                "-o",
                "/dev/null",
                "-w",
                "%{url_effective}",
                &format!("{RELEASES}/latest"),
            ],
        ))?;
        let Some(version) = latest_version(&latest.out) else {
            bail!("could not find the latest Apple container release at {RELEASES}");
        };
        let pkg = path_str(&std::env::temp_dir().join(format!("ai2-container-{version}.pkg")));
        ctx.runner
            .say(&format!("downloading Apple container {version}"));
        must(
            ctx.runner
                .run(&Cmd::new("curl", &["-fsSL", "-o", &pkg, &pkg_url(&version)]).shown())?,
            "downloading the container installer",
        )?;
        let signature = ctx
            .runner
            .run(&Cmd::new("pkgutil", &["--check-signature", &pkg]))?;
        if !signature.ok || !signed_by_apple(&signature.out) {
            let _ = ctx.runner.run(&Cmd::new("rm", &["-f", &pkg]));
            bail!("the container installer is not signed by {CONTAINER_SIGNER} and notarized");
        }
        let installed = ctx.runner.run(
            &Cmd::new("installer", &["-pkg", &pkg, "-target", "/"])
                .root()
                .shown(),
        )?;
        let _ = ctx.runner.run(&Cmd::new("rm", &["-f", &pkg]));
        must(installed, "installing Apple container").map(|_| ())
    }
}

impl Step for Containers {
    fn id(&self) -> &'static str {
        "containers"
    }

    fn check(&self, ctx: &Context) -> Result<State> {
        match &ctx.host.os {
            Os::Macos { .. } if supports_containers(&ctx.host.os) => {
                let path = found(ctx, "container")?;
                let running = match &path {
                    Some(path) => {
                        let ran = ctx
                            .runner
                            .run(&Cmd::new(path, &["system", "status"]))?;
                        ran.ok && ran.out.contains("running")
                    }
                    None => false,
                };
                Ok(State::ready(path.is_some() && running)
                    .with("runtime", "apple-container")
                    .with("path", path.map_or(Value::Null, Value::from))
                    .with("running", running))
            }
            Os::Linux {
                systemd: Some(version),
            } if supports_containers(&ctx.host.os) => Ok(State::new(Status::Ready)
                .with("runtime", "systemd")
                .with("systemd", *version)),
            Os::Macos { version, arm64 } => Ok(State::new(Status::Unsupported)
                .with("runtime", Value::Null)
                .with(
                    "reason",
                    format!(
                        "Apple container needs macOS {MACOS_CONTAINERS} on Apple silicon; this is macOS {version}{}",
                        if *arm64 { "" } else { " on Intel" }
                    ),
                )
                .warn(NOT_SAFE)),
            Os::Linux { systemd } => Ok(State::new(Status::Unsupported)
                .with("runtime", Value::Null)
                .with(
                    "reason",
                    match systemd {
                        Some(version) => format!(
                            "sandboxes need systemd {SYSTEMD_CREDS} or later; this is systemd {version}"
                        ),
                        None => "systemd is not running".to_string(),
                    },
                )
                .warn(NOT_SAFE)),
            Os::Other { name } => Ok(State::new(Status::Unsupported)
                .with("runtime", Value::Null)
                .with("reason", format!("{name} is not supported"))
                .warn(NOT_SAFE)),
        }
    }

    fn apply(&self, ctx: &Context, checked: &State) -> Result<State> {
        if checked.info.get("path") == Some(&Value::Null) {
            self.install(ctx)?;
        }
        let Some(path) = found(ctx, "container")? else {
            bail!("Apple container was installed but `container` is not on the PATH");
        };
        must(
            ctx.runner
                .run(&Cmd::new(&path, &["system", "start", "--enable-kernel-install"]).shown())?,
            "container system start",
        )?;
        let state = self.check(ctx)?;
        if !state.is_ready() {
            bail!("Apple container is installed but its system service is not running");
        }
        Ok(state)
    }
}

pub struct MasterKeyStep;

const CRED_DIR: &str = "/etc/ai2-vault";

fn credential(ctx: &Context) -> Result<State> {
    let ran = ctx
        .runner
        .run(&Cmd::new("stat", &["-c", "%a %U", LINUX_CRED]))?;
    let mut fields = ran.out.split_whitespace();
    let (Some(mode), Some(owner), true) = (fields.next(), fields.next(), ran.ok) else {
        return Ok(State::new(Status::Missing));
    };
    let private = u32::from_str_radix(mode, 8).is_ok_and(|bits| bits & 0o177 == 0);
    let state = State::ready(private && owner == "root")
        .with("mode", mode)
        .with("owner", owner);
    Ok(if state.is_ready() {
        state
    } else {
        state.with("reason", "the credential must be mode 0600 owned by root")
    })
}

fn store(os: &Os) -> &'static str {
    match os {
        Os::Macos { .. } => "keychain",
        Os::Linux { .. } => "systemd-creds",
        Os::Other { .. } => "none",
    }
}

impl Step for MasterKeyStep {
    fn id(&self) -> &'static str {
        "master-key"
    }

    fn check(&self, ctx: &Context) -> Result<State> {
        let state = match ctx.host.os {
            Os::Linux { .. } => credential(ctx)?,
            _ => State::ready(ctx.master.exists()?),
        };
        Ok(state.with("store", store(&ctx.host.os)))
    }

    fn apply(&self, ctx: &Context, checked: &State) -> Result<State> {
        if let Os::Linux { systemd } = &ctx.host.os {
            if systemd.is_none_or(|version| version < SYSTEMD_CREDS) {
                let has = systemd.map_or("no systemd".to_string(), |v| format!("systemd {v}"));
                bail!(
                    "the vault keeps its master key with systemd-creds, which needs systemd {SYSTEMD_CREDS} or later; this machine has {has}. Upgrade systemd, then run ai2 setup again."
                );
            }
        }
        if let Os::Linux { .. } = ctx.host.os {
            if checked.info.contains_key("reason") {
                ctx.runner
                    .say("tightening the vault master key file to mode 0600");
            } else {
                ctx.runner.say("making the vault master key");
                ctx.runner.say(&format!(
                    "encrypting the vault master key to {LINUX_CRED} with systemd-creds, through sudo"
                ));
                must(
                    ctx.runner
                        .run(&Cmd::new("install", &["-d", "-m", "0755", CRED_DIR]).root())?,
                    &format!("making {CRED_DIR}"),
                )?;
                must(
                    ctx.runner.run(
                        &Cmd::new(
                            "systemd-creds",
                            &["encrypt", "--name=master", "-", LINUX_CRED],
                        )
                        .root()
                        .input(&*keys::random_key()?),
                    )?,
                    "systemd-creds could not encrypt the master key; it needs systemd 250 or later",
                )?;
            }
            for command in [
                Cmd::new("chmod", &["0600", LINUX_CRED]),
                Cmd::new("chown", &["root:root", LINUX_CRED]),
            ] {
                must(
                    ctx.runner.run(&command.root())?,
                    &format!("securing {LINUX_CRED}"),
                )?;
            }
        } else {
            ctx.runner.say("making the vault master key");
            ctx.master.create()?;
        }
        self.check(ctx)
    }
}

pub struct PresenceStep;

const HAS_FILE: &str = r#"if test -s "$1"; then echo yes; else echo no; fi"#;

impl Step for PresenceStep {
    fn id(&self) -> &'static str {
        "presence"
    }

    fn check(&self, ctx: &Context) -> Result<State> {
        match &ctx.host.os {
            Os::Macos { .. } => {
                Ok(State::ready((ctx.presence)()?).with("method", "touch-id-or-password"))
            }
            Os::Linux { .. } => {
                let ran = ctx.runner.run(
                    &Cmd::new("sh", &["-c", HAS_FILE, "sh", LINUX_PASSPHRASE])
                        .root_if(!ctx.check_only),
                )?;
                let state = match ran.out.trim() {
                    "yes" => State::new(Status::Ready),
                    "no" => State::new(Status::Missing),
                    _ => State::new(Status::Unknown)
                        .with("reason", "reading the passphrase hash needs sudo"),
                };
                Ok(state
                    .with("method", "passphrase")
                    .with("hash", LINUX_PASSPHRASE))
            }
            Os::Other { .. } => Ok(State::new(Status::Unsupported)),
        }
    }

    fn apply(&self, ctx: &Context, checked: &State) -> Result<State> {
        match ctx.host.os {
            Os::Macos { .. } => bail!("this Mac cannot confirm that a human is there: set a login password and Touch ID in System Settings, then run ai2 setup again"),
            Os::Other { .. } => return Ok(checked.clone()),
            Os::Linux { .. } => {}
        }
        ctx.runner
            .say("choose the vault passphrase; it is asked for every change to the vault");
        let first = ctx
            .runner
            .ask_passphrase("Vault passphrase: ", "The same again: ")?;
        let hashed = keys::passphrase::hash(&first)?;
        must(
            ctx.runner
                .run(&Cmd::new("install", &["-d", "-m", "0700", LINUX_STATE]).root())?,
            &format!("making {LINUX_STATE}"),
        )?;
        root_write(ctx, LINUX_PASSPHRASE, &format!("{hashed}\n"), "077")?;
        self.check(ctx)
    }
}

pub struct Daemon;

impl Daemon {
    fn launchd(&self, ctx: &Context) -> (std::path::PathBuf, String, String) {
        (
            files::plist_path(&ctx.host.home, &ctx.options.label),
            files::plist(
                &ctx.options.bin,
                &ctx.options.home,
                &ctx.options.state,
                &ctx.options.label,
                ctx.options.keychain.as_deref(),
            ),
            format!("gui/{}", ctx.host.uid),
        )
    }

    fn mac_check(&self, ctx: &Context) -> Result<State> {
        let (plist, expected, domain) = self.launchd(ctx);
        let written = ctx.runner.read(&plist).as_deref() == Some(expected.as_str());
        let loaded = ctx
            .runner
            .run(&Cmd::new(
                "launchctl",
                &["print", &format!("{domain}/{}", ctx.options.label)],
            ))?
            .ok;
        Ok(State::ready(written && loaded)
            .with("plist", path_str(&plist))
            .with("written", written)
            .with("loaded", loaded))
    }

    fn mac_apply(&self, ctx: &Context, before: &State) -> Result<State> {
        let (plist, expected, domain) = self.launchd(ctx);
        ctx.runner.mkdir(&ctx.options.state, 0o700)?;
        ctx.runner.write(&plist, &expected, 0o644)?;
        if before.info.get("loaded") == Some(&Value::Bool(true)) {
            let _ = ctx.runner.run(&Cmd::new(
                "launchctl",
                &["bootout", &format!("{domain}/{}", ctx.options.label)],
            ))?;
        }
        let bootstrap = Cmd::new("launchctl", &["bootstrap", &domain, &path_str(&plist)]);
        let mut ran = ctx.runner.run(&bootstrap)?;
        if !ran.ok {
            ran = ctx.runner.run(&bootstrap)?;
        }
        must(ran, "launchctl bootstrap")?;
        self.mac_check(ctx)
    }

    fn linux_check(&self, ctx: &Context) -> Result<State> {
        let expected = files::unit(Path::new(SERVICE_BIN), LINUX_CRED, ctx.host.uid);
        let written = ctx.runner.read(Path::new(UNIT_PATH)).as_deref() == Some(expected.as_str());
        let user = ctx.runner.run(&Cmd::new("getent", &["passwd", USER]))?.ok;
        let groups = ctx.runner.run(&Cmd::new("id", &["-nG", &ctx.host.user]))?;
        let admin = groups.out.split_whitespace().any(|g| g == ADMIN_GROUP);
        let bin = ctx
            .runner
            .run(&Cmd::new(
                "cmp",
                &["-s", &path_str(&ctx.options.bin), SERVICE_BIN],
            ))?
            .ok;
        let active = ctx
            .runner
            .run(&Cmd::new("systemctl", &["is-active", "--quiet", UNIT]))?
            .ok;
        Ok(State::ready(written && user && admin && bin && active)
            .with("unit", UNIT_PATH)
            .with("written", written)
            .with("user", user)
            .with("admin", admin)
            .with("binary", bin)
            .with("active", active))
    }

    fn linux_apply(&self, ctx: &Context, before: &State) -> Result<State> {
        let was = |key: &str| before.info.get(key) == Some(&Value::Bool(true));
        let mut warnings = Vec::new();
        if !was("user") {
            must(
                ctx.runner.run(
                    &Cmd::new(
                        "useradd",
                        &[
                            "--system",
                            "--user-group",
                            "--no-create-home",
                            "--home-dir",
                            LINUX_STATE,
                            "--shell",
                            "/usr/sbin/nologin",
                            USER,
                        ],
                    )
                    .root(),
                )?,
                &format!("making the {USER} user"),
            )?;
        }
        if !ctx
            .runner
            .run(&Cmd::new("getent", &["group", ADMIN_GROUP]))?
            .ok
        {
            must(
                ctx.runner
                    .run(&Cmd::new("groupadd", &["--system", ADMIN_GROUP]).root())?,
                &format!("making the {ADMIN_GROUP} group"),
            )?;
        }
        if !was("admin") {
            must(
                ctx.runner
                    .run(&Cmd::new("usermod", &["-aG", ADMIN_GROUP, &ctx.host.user]).root())?,
                &format!("adding {} to {ADMIN_GROUP}", ctx.host.user),
            )?;
            warnings.push(format!(
                "Log out and back in so {} can reach the vault as a member of {ADMIN_GROUP}.",
                ctx.host.user
            ));
        }
        if ctx.options.bin != Path::new(SERVICE_BIN) {
            must(
                ctx.runner.run(
                    &Cmd::new(
                        "install",
                        &[
                            "-D",
                            "-m",
                            "0755",
                            "-o",
                            "root",
                            "-g",
                            "root",
                            &path_str(&ctx.options.bin),
                            SERVICE_BIN,
                        ],
                    )
                    .root(),
                )?,
                &format!("copying ai2 to {SERVICE_BIN}"),
            )?;
        }
        root_write(
            ctx,
            UNIT_PATH,
            &files::unit(Path::new(SERVICE_BIN), LINUX_CRED, ctx.host.uid),
            "022",
        )?;
        must(
            ctx.runner.run(
                &Cmd::new(
                    "install",
                    &["-d", "-o", USER, "-g", USER, "-m", "0700", LINUX_STATE],
                )
                .root(),
            )?,
            &format!("making {LINUX_STATE}"),
        )?;
        must(
            ctx.runner
                .run(&Cmd::new("chown", &["-R", &format!("{USER}:{USER}"), LINUX_STATE]).root())?,
            &format!("handing {LINUX_STATE} to {USER}"),
        )?;
        must(
            ctx.runner
                .run(&Cmd::new("systemctl", &["daemon-reload"]).root())?,
            "systemctl daemon-reload",
        )?;
        must(
            ctx.runner
                .run(&Cmd::new("systemctl", &["enable", "--now", UNIT]).root())?,
            &format!("starting {UNIT}"),
        )?;
        if was("active") {
            must(
                ctx.runner
                    .run(&Cmd::new("systemctl", &["restart", UNIT]).root())?,
                &format!("restarting {UNIT}"),
            )?;
        }
        let mut state = self.linux_check(ctx)?;
        state.warnings.extend(warnings);
        Ok(state)
    }
}

impl Step for Daemon {
    fn id(&self) -> &'static str {
        "daemon"
    }

    fn check(&self, ctx: &Context) -> Result<State> {
        match &ctx.host.os {
            Os::Macos { .. } => self.mac_check(ctx),
            Os::Linux { .. } => self.linux_check(ctx),
            Os::Other { .. } => Ok(State::new(Status::Unsupported)),
        }
    }

    fn apply(&self, ctx: &Context, checked: &State) -> Result<State> {
        match &ctx.host.os {
            Os::Macos { .. } => self.mac_apply(ctx, checked),
            Os::Linux { .. } => self.linux_apply(ctx, checked),
            Os::Other { .. } => Ok(checked.clone()),
        }
    }

    fn settled(&self, ctx: &Context, state: State) -> Result<State> {
        if !state.is_ready() {
            return Ok(state);
        }
        Ok(host_sandbox(ctx, state))
    }
}

pub const HOST_PROMPT: &str = "Vault passphrase, to add the sandbox host and nothing else: ";

fn host_sandbox(ctx: &Context, state: State) -> State {
    let skipped = |state: State, why: String| {
        state.with("host_sandbox", "skipped").warn(&format!(
            "The vault sandbox {HOST_SANDBOX} was not made: {why}. Run ai2 setup again later, or ai2 vault sandbox add --name {HOST_SANDBOX}."
        ))
    };
    match ctx.admin.sandboxes() {
        Ok(names) if names.iter().any(|name| name == HOST_SANDBOX) => {
            return state.with("host_sandbox", "present")
        }
        Ok(_) => {}
        Err(error) => {
            return skipped(
                state,
                format!("the vault's admin API is not reachable ({error:#})"),
            )
        }
    }
    let passphrase = match ctx.host.os {
        Os::Linux { .. } => {
            ctx.runner.say(&format!(
                "adding the vault sandbox {HOST_SANDBOX}, for apps that run on this machine; the vault asks for its passphrase for this one change"
            ));
            match ctx.runner.ask_hidden(HOST_PROMPT) {
                Ok(passphrase) => Some(zeroize::Zeroizing::new(passphrase)),
                Err(error) => return skipped(state, format!("{error:#}")),
            }
        }
        _ => {
            ctx.runner.say(&format!(
                "adding the vault sandbox {HOST_SANDBOX}, for apps that run on this machine; the vault asks for Touch ID or your password for this one change"
            ));
            None
        }
    };
    match ctx
        .admin
        .add_sandbox(HOST_SANDBOX, passphrase.as_ref().map(|p| p.as_str()))
    {
        Ok((200..=299, _)) => state.with("host_sandbox", "created"),
        Ok((status, value)) => skipped(
            state,
            format!(
                "the vault answered {status}: {}",
                value["message"].as_str().unwrap_or("no message")
            ),
        ),
        Err(error) => skipped(state, format!("{error:#}")),
    }
}
