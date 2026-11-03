use crate::cli::{default_out, exit_code, providers_file, Build, Subcommand};
use crate::doctor::{Row, State};
use crate::model::Model;
use anyhow::{anyhow, bail, Context, Result};
use clap::ArgMatches;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::Arc;
use std::time::Duration;
use vault::{KeyInfo, NoVetter, Outcome, Proposal, Resolver, Serve, Verdict, Vetter, Vetting};

pub struct ModelVetter(pub Model);

impl Vetter for ModelVetter {
    fn propose(&self, key: &KeyInfo) -> Result<Vec<String>> {
        Ok(self
            .0
            .ask::<Proposal>(vault::PROPOSE_SYSTEM, &key.input())?
            .prefixes)
    }
    fn vet(&self, call: &Vetting) -> Result<Verdict> {
        self.0.ask::<Verdict>(vault::VET_SYSTEM, &call.input())
    }
}

pub struct VaultSubcommand {
    pub name: &'static str,
}

impl VaultSubcommand {
    pub fn new(name: &'static str) -> Self {
        Self { name }
    }

    fn options(&self) -> vault::Options {
        let name = self.name;
        vault::Options::new(
            &crate::cli::state_dir(name),
            Box::new(move |serve| run_serve(name, serve)),
        )
    }
}

impl Subcommand for VaultSubcommand {
    fn command(&self) -> clap::Command {
        vault::command_with(vec![vault_ui::command()])
    }
    fn run(&self, matches: &ArgMatches) -> Result<ExitCode> {
        let options = self.options();
        match vault::outcome(matches, &options)? {
            Outcome::Code(code) => Ok(ExitCode::from(code as u8)),
            Outcome::Extra(name, sub) if name == "ui" => {
                let report = sub
                    .get_one::<PathBuf>("report")
                    .map(|path| report_file(path))
                    .transpose()?;
                vault_ui::run(report, &options.admin).map(exit_code)
            }
            Outcome::Extra(name, _) => Err(anyhow!("unknown vault subcommand {name}")),
        }
    }
}

fn report_file(path: &Path) -> Result<PathBuf> {
    if path.is_file() {
        return Ok(path.to_path_buf());
    }
    let folder = path
        .canonicalize()
        .with_context(|| format!("{} is not a folder or a report", path.display()))?;
    let reviewed = default_out(Build::ai2(), &folder).join("report.html");
    if reviewed.is_file() {
        return Ok(reviewed);
    }
    let inside = folder.join("report.html");
    if inside.is_file() {
        return Ok(inside);
    }
    bail!(
        "There is no report.html for {}: run ai2 on it first.",
        folder.display()
    )
}

pub struct SetupSubcommand {
    pub name: &'static str,
}

impl Subcommand for SetupSubcommand {
    fn command(&self) -> clap::Command {
        setup::command()
    }
    fn run(&self, matches: &ArgMatches) -> Result<ExitCode> {
        let bin = std::env::current_exe()?;
        setup::run(
            matches,
            &setup::Options::new(bin, crate::cli::state_dir(self.name)),
        )
        .map(exit_code)
    }
}

fn run_serve(name: &str, serve: Serve) -> Result<()> {
    let platform = keys::platform()?;
    let mut config = serve.config;
    config
        .owners
        .extend(owners(std::env::var("AI2_VAULT_OWNERS").ok().as_deref()));
    let file = providers_file(name, serve.providers);
    let model = serve.model;
    vault::start(config, platform, move |resolver: Resolver| {
        let Some(file) = file else {
            eprintln!("{name} vault: no providers file, so calls that need vetting are refused");
            return Box::new(NoVetter);
        };
        match Model::load(&file, model.as_deref()) {
            Ok(m) => Box::new(ModelVetter(m.with_vault(Arc::new(move |key: &str| {
                resolver(key).map(|secret| secret.as_str().to_string())
            })))),
            Err(error) => {
                eprintln!("{name} vault: no vetting model: {error:#}");
                Box::new(NoVetter)
            }
        }
    })?
    .wait();
    Ok(())
}

fn owners(listed: Option<&str>) -> Vec<u32> {
    listed
        .unwrap_or_default()
        .split(',')
        .filter_map(|uid| uid.trim().parse().ok())
        .collect()
}

fn admin_get(name: &'static str, path: &str) -> Result<Value> {
    let options = VaultSubcommand::new(name).options();
    let (status, value) = vault::cli::Admin::new(&options).call("GET", path, None, false)?;
    if status != 200 {
        bail!("{path} answered {status}: {value}");
    }
    Ok(value)
}

pub fn daemon_version(name: &'static str) -> Result<String> {
    admin_get(name, "/status")?["version"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("/status gave no version"))
}

pub fn sandboxes(name: &'static str) -> Row {
    sandboxes_row(name, admin_get(name, "/sandboxes"))
}

pub fn sandboxes_row(name: &str, listed: Result<Value>) -> Row {
    const CHECK: &str = "sandboxes";
    let listed = match listed {
        Ok(listed) => listed,
        Err(error) => {
            return Row::fail(CHECK, format!("{error:#}")).fix(format!("run {name} setup"))
        }
    };
    let names: Vec<&str> = listed["sandboxes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|sandbox| sandbox["name"].as_str())
        .collect();
    if names.is_empty() {
        return Row::ok(CHECK, "none");
    }
    let detail = names.join(", ");
    if names.contains(&"host") {
        return Row::warn(
            CHECK,
            format!("{detail}; host is not safe: its tokens work for any process on this machine, outside a container"),
        )
        .fix(format!(
            "give agents a container sandbox, and remove host with {name} vault sandbox remove --name host"
        ));
    }
    Row::ok(CHECK, detail)
}

pub fn setup_rows(name: &'static str) -> Vec<Row> {
    let report = std::env::current_exe()
        .map_err(anyhow::Error::from)
        .and_then(|bin| {
            setup::setup(&setup::Options::new(bin, crate::cli::state_dir(name)), true)
                .map(|(_, report)| report)
        });
    steps_rows(name, &report)
}

pub fn steps_rows(name: &str, report: &Result<Value>) -> Vec<Row> {
    setup::all()
        .iter()
        .map(|step| match report {
            Ok(report) => step_row(name, step.id(), &report["steps"][step.id()]),
            Err(error) => {
                Row::fail(step.id(), format!("{error:#}")).fix(format!("run {name} setup"))
            }
        })
        .collect()
}

fn step_row(name: &str, id: &str, step: &Value) -> Row {
    let status = step["status"].as_str().unwrap_or("unknown");
    let state = match status {
        "ready" => State::Ok,
        "missing" | "failed" => State::Fail,
        _ => State::Warn,
    };
    let why = step["reason"].as_str().or(step["error"].as_str());
    let mut detail = match (status, why) {
        ("skipped", _) => "skipped after an earlier check failed".to_string(),
        (_, Some(why)) => why.to_string(),
        _ => step
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(key, _)| match key.as_str() {
                "warnings" => false,
                "status" => status != "ready",
                _ => true,
            })
            .map(|(key, value)| match value {
                Value::String(text) => format!("{key}: {text}"),
                other => format!("{key}: {other}"),
            })
            .collect::<Vec<_>>()
            .join(", "),
    };
    for warning in step["warnings"].as_array().into_iter().flatten() {
        if let Some(warning) = warning.as_str() {
            detail = format!("{detail}; {warning}");
        }
    }
    let row = Row::new(id, state, detail);
    match status {
        "ready" | "unsupported" => row,
        _ => row.fix(format!("run {name} setup")),
    }
}

pub struct Socket {
    pub owner: String,
    pub group: String,
    pub mode: u32,
}

pub struct Expected {
    pub owner: String,
    pub group: Option<String>,
    pub mode: u32,
}

pub fn admin_socket(name: &'static str) -> Row {
    let options = VaultSubcommand::new(name).options();
    let mode = vault::Config::new(options.state.clone(), options.admin.clone()).admin_mode;
    let expected = if cfg!(target_os = "linux") {
        Expected {
            owner: setup::files::USER.into(),
            group: Some(setup::files::ADMIN_GROUP.into()),
            mode,
        }
    } else {
        Expected {
            owner: output("id", &["-un"]).unwrap_or_default(),
            group: None,
            mode,
        }
    };
    socket_row(name, &options.admin, stat(&options.admin), &expected)
}

fn output(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn stat(path: &Path) -> Option<Socket> {
    let format = if cfg!(target_os = "linux") {
        ["-c", "%U %G %a"]
    } else {
        ["-f", "%Su %Sg %Lp"]
    };
    let text = output("stat", &[format[0], format[1], path.to_str()?])?;
    let mut fields = text.split_whitespace();
    Some(Socket {
        owner: fields.next()?.into(),
        group: fields.next()?.into(),
        mode: u32::from_str_radix(fields.next()?, 8).ok()?,
    })
}

pub fn socket_row(name: &str, path: &Path, found: Option<Socket>, expected: &Expected) -> Row {
    const CHECK: &str = "admin-socket";
    let Some(found) = found else {
        return Row::fail(CHECK, format!("no admin socket at {}", path.display()))
            .fix(format!("run {name} setup"));
    };
    let detail = format!(
        "{}: owner {}, group {}, mode {:04o}",
        path.display(),
        found.owner,
        found.group,
        found.mode
    );
    let right = found.owner == expected.owner
        && expected
            .group
            .as_ref()
            .is_none_or(|group| *group == found.group)
        && found.mode == expected.mode;
    if right {
        return Row::ok(CHECK, detail);
    }
    let group = expected
        .group
        .as_ref()
        .map_or(String::new(), |group| format!(", group {group}"));
    Row::fail(
        CHECK,
        format!(
            "{detail}; it should be owner {}{group}, mode {:04o}",
            expected.owner, expected.mode
        ),
    )
    .fix(format!(
        "run {name} setup so the daemon makes the socket again"
    ))
}

pub fn vetting_model(name: &str) -> Row {
    let Some(file) = providers_file(name, None) else {
        return Row::warn(
            VETTING,
            "no providers file, so the vault refuses calls that need vetting",
        )
        .fix(format!(
            "write ~/.{name}/providers.json or set {}_PROVIDERS",
            name.to_uppercase().replace('-', "_")
        ));
    };
    match Model::load(&file, None) {
        Ok(model) => vetting_row(&model.name(), model.base_url(), reach(model.base_url())),
        Err(error) => {
            Row::fail(VETTING, format!("{error:#}")).fix(format!("fix {}", file.display()))
        }
    }
}

const VETTING: &str = "vetting-model";

fn reach(url: &str) -> Result<u16> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .http_status_as_error(false)
        .build()
        .new_agent();
    Ok(agent
        .get(url)
        .call()
        .with_context(|| format!("calling {url}"))?
        .status()
        .as_u16())
}

pub fn vetting_row(model: &str, url: &str, reached: Result<u16>) -> Row {
    match reached {
        Ok(status) => Row::ok(VETTING, format!("{model} at {url} answered {status}")),
        Err(error) => Row::fail(VETTING, format!("{model}: {error:#}"))
            .fix(format!("check the network and {url}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_host_sandbox_is_a_warning_and_an_unreachable_daemon_a_failure() {
        assert_eq!(
            sandboxes_row("ai2", Ok(json!({ "sandboxes": [] }))).state,
            State::Ok
        );
        let boxed = sandboxes_row("ai2", Ok(json!({ "sandboxes": [{ "name": "box" }] })));
        assert_eq!((boxed.state, boxed.detail.as_str()), (State::Ok, "box"));
        let host = sandboxes_row(
            "ai2",
            Ok(json!({ "sandboxes": [{ "name": "box" }, { "name": "host" }] })),
        );
        assert_eq!(host.state, State::Warn);
        assert!(host.detail.starts_with("box, host; host is not safe"));
        assert_eq!(
            sandboxes_row("ai2", Err(anyhow!("not running"))).state,
            State::Fail
        );
    }

    #[test]
    fn setup_steps_become_rows_in_order() {
        let report = Ok(json!({ "ok": false, "check": true, "steps": {
            "daemon": { "status": "missing", "written": false, "loaded": false },
            "containers": { "status": "unsupported", "runtime": null, "reason": "needs macOS 26", "warnings": ["Not safe"] },
            "master-key": { "status": "ready", "store": "keychain" },
            "presence": { "status": "unknown", "reason": "needs sudo" },
        }}));
        let rows = steps_rows("ai2", &report);
        let shape: Vec<_> = rows
            .iter()
            .map(|r| {
                (
                    r.check.as_str(),
                    r.state,
                    r.detail.as_str(),
                    r.fix.is_some(),
                )
            })
            .collect();
        assert_eq!(
            shape,
            [
                ("containers", State::Warn, "needs macOS 26; Not safe", false),
                ("master-key", State::Ok, "store: keychain", false),
                ("presence", State::Warn, "needs sudo", true),
                (
                    "daemon",
                    State::Fail,
                    "status: missing, written: false, loaded: false",
                    true
                ),
            ]
        );
        let failed = steps_rows("ai2", &Err(anyhow!("no keychain")));
        assert!(failed.iter().all(|r| r.state == State::Fail));
    }

    #[test]
    fn the_admin_socket_needs_its_owner_and_mode() {
        let path = Path::new("/run/ai2-vault/admin.sock");
        let expected = Expected {
            owner: "ai2-vault".into(),
            group: Some("ai2-vault-admin".into()),
            mode: 0o660,
        };
        let socket = |owner: &str, mode| Socket {
            owner: owner.into(),
            group: "ai2-vault-admin".into(),
            mode,
        };
        let right = socket_row("ai2", path, Some(socket("ai2-vault", 0o660)), &expected);
        assert_eq!(right.state, State::Ok);
        let open = socket_row("ai2", path, Some(socket("ai2-vault", 0o666)), &expected);
        assert_eq!(open.state, State::Fail);
        assert!(open.detail.ends_with(
            "mode 0666; it should be owner ai2-vault, group ai2-vault-admin, mode 0660"
        ));
        let rooted = socket_row("ai2", path, Some(socket("root", 0o660)), &expected);
        assert_eq!(rooted.state, State::Fail);
        assert_eq!(socket_row("ai2", path, None, &expected).state, State::Fail);
    }

    #[test]
    fn the_vetting_model_is_reached_without_a_key() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let answer = std::thread::spawn(move || {
            let (mut conn, _) = listener.accept().unwrap();
            let mut asked = [0u8; 1024];
            let read = conn.read(&mut asked).unwrap();
            conn.write_all(
                b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
            )
            .unwrap();
            String::from_utf8_lossy(&asked[..read]).to_lowercase()
        });
        let row = vetting_row("a/one", &url, reach(&url));
        assert_eq!(row.state, State::Ok);
        assert!(row.detail.ends_with("answered 404"));
        let asked = answer.join().unwrap();
        assert!(!asked.contains("authorization") && !asked.contains("x-api-key"));
        assert_eq!(
            vetting_row("a/one", &url, Err(anyhow!("refused"))).state,
            State::Fail
        );
    }

    #[test]
    fn owners_come_from_a_comma_list_of_uids() {
        assert_eq!(super::owners(Some("501, 1000,x")), vec![501, 1000]);
        assert!(super::owners(None).is_empty());
    }
}
