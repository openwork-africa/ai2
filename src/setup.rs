use crate::cli::{exit_code, Subcommand};
use crate::doctor::{Row, State};
use crate::vault::VaultSubcommand;
use anyhow::Result;
use clap::ArgMatches;
use serde_json::Value;
use std::path::Path;
use std::process::{Command, ExitCode};

pub struct SetupSubcommand {
    pub name: &'static str,
}

impl Subcommand for SetupSubcommand {
    fn command(&self) -> clap::Command {
        setup::command()
    }
    fn run(&self, matches: &ArgMatches) -> Result<ExitCode> {
        let bin = std::env::current_exe()?;
        let options = setup::Options::new(bin, crate::cli::state_dir(self.name));
        match setup::outcome(matches, &options)? {
            setup::Outcome::Done(ok) => Ok(exit_code(ok)),
            setup::Outcome::Ui => in_the_browser(self.name, options).map(exit_code),
        }
    }
}

#[cfg(feature = "ui")]
fn in_the_browser(name: &'static str, options: setup::Options) -> Result<bool> {
    let port = crate::ui::port();
    let build = crate::cli::Build {
        name,
        ..crate::cli::Build::ai2()
    };
    crate::ui::inject(build);
    ui::setup::run(
        &crate::ui::options(build),
        options,
        ui::setup::Launch { port, open: true },
    )
}

#[cfg(not(feature = "ui"))]
fn in_the_browser(_name: &'static str, options: setup::Options) -> Result<bool> {
    eprintln!("{}", setup::NO_UI);
    setup::headless(&options, false)
}

pub fn rows(name: &'static str, version: &str) -> Vec<Row> {
    let report = std::env::current_exe()
        .map_err(anyhow::Error::from)
        .and_then(|bin| {
            setup::setup(&setup::Options::new(bin, crate::cli::state_dir(name)), true)
                .map(|(_, report)| report)
        });
    let mut rows = steps_rows(name, &report);
    if let (Ok(report), Some(row)) = (&report, rows.iter_mut().find(|row| row.check == DASHBOARD)) {
        let step = &report["steps"][DASHBOARD];
        let port = step["port"]
            .as_u64()
            .and_then(|port| u16::try_from(port).ok());
        let answer = port.map(|port| answer(name, port)).unwrap_or_default();
        *row = dashboard_row(name, step, &answer, version);
    }
    rows.push(admin_socket(name));
    rows
}

pub const DASHBOARD: &str = "dashboard";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Answer {
    pub listening: bool,
    pub version: Option<String>,
}

#[cfg(feature = "apps")]
fn answer(name: &str, port: u16) -> Answer {
    let home = crate::cli::state_dir(name);
    Answer {
        listening: apps::port::listening(port),
        version: apps::Ui::find(&home, port)
            .and_then(|ui| ui.call("GET", "/api/status").ok())
            .and_then(|status| status["version"].as_str().map(String::from)),
    }
}

#[cfg(not(feature = "apps"))]
fn answer(_name: &str, _port: u16) -> Answer {
    Answer::default()
}

pub fn dashboard_row(name: &str, step: &Value, answer: &Answer, client: &str) -> Row {
    if step["status"].as_str() != Some("ready") {
        return step_row(name, DASHBOARD, step);
    }
    let port = step["port"]
        .as_u64()
        .unwrap_or(u64::from(setup::files::UI_PORT));
    let address = format!("http://127.0.0.1:{port}");
    let restart = step["restart"]
        .as_str()
        .map_or_else(|| format!("run {name} setup"), |line| format!("run {line}"));
    if !answer.listening {
        return Row::fail(
            DASHBOARD,
            format!("the service is loaded, but nothing answers on {address}"),
        )
        .fix(restart);
    }
    let Some(running) = &answer.version else {
        return Row::fail(
            DASHBOARD,
            format!("something answers on {address}, but it is not the ai2 dashboard"),
        )
        .fix(format!(
            "stop what holds port {port}, or set AI2_UI_PORT to another port and run {name} setup"
        ));
    };
    if running != client {
        return Row::warn(
            DASHBOARD,
            format!("listening on {address}, but it runs {running} and this binary is {client}"),
        )
        .fix(format!("{restart} so the dashboard starts on {client}"));
    }
    Row::ok(
        DASHBOARD,
        format!("loaded and listening on {address}, version {running}"),
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use serde_json::json;

    #[test]
    fn setup_steps_become_rows_in_order() {
        let report = Ok(json!({ "ok": false, "check": true, "steps": {
            "daemon": { "status": "missing", "written": false, "loaded": false },
            "containers": { "status": "unsupported", "runtime": null, "reason": "needs macOS 26", "warnings": ["Not safe"] },
            "master-key": { "status": "ready", "store": "keychain" },
            "presence": { "status": "unknown", "reason": "needs sudo" },
            "dashboard": { "status": "ready", "loaded": true, "port": 7797 },
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
                ("dashboard", State::Ok, "loaded: true, port: 7797", false),
            ]
        );
        let failed = steps_rows("ai2", &Err(anyhow!("no keychain")));
        assert!(failed.iter().all(|r| r.state == State::Fail));
    }

    #[test]
    fn the_dashboard_row_needs_the_service_loaded_listening_and_on_this_version() {
        let ready = json!({
            "status": "ready", "written": true, "loaded": true, "port": 7797,
            "restart": "launchctl kickstart -k gui/501/africa.openwork.ai2-ui",
        });
        let up = |version: Option<&str>| Answer {
            listening: true,
            version: version.map(String::from),
        };
        let row = dashboard_row("ai2", &ready, &up(Some("0.1.2")), "0.1.2");
        assert_eq!(row.check, "dashboard");
        assert_eq!(row.state, State::Ok);
        assert_eq!(
            row.detail,
            "loaded and listening on http://127.0.0.1:7797, version 0.1.2"
        );
        assert!(row.fix.is_none());

        let old = dashboard_row("ai2", &ready, &up(Some("0.1.1")), "0.1.2");
        assert_eq!(old.state, State::Warn);
        assert_eq!(
            old.detail,
            "listening on http://127.0.0.1:7797, but it runs 0.1.1 and this binary is 0.1.2"
        );
        assert_eq!(
            old.fix.as_deref(),
            Some("run launchctl kickstart -k gui/501/africa.openwork.ai2-ui so the dashboard starts on 0.1.2")
        );

        let silent = dashboard_row("ai2", &ready, &Answer::default(), "0.1.2");
        assert_eq!(silent.state, State::Fail);
        assert!(silent
            .detail
            .contains("nothing answers on http://127.0.0.1:7797"));
        assert_eq!(
            silent.fix.as_deref(),
            Some("run launchctl kickstart -k gui/501/africa.openwork.ai2-ui")
        );

        let stranger = dashboard_row("ai2", &ready, &up(None), "0.1.2");
        assert_eq!(stranger.state, State::Fail);
        assert!(stranger.detail.contains("not the ai2 dashboard"));
        assert!(stranger.fix.unwrap().contains("AI2_UI_PORT"));

        let missing =
            json!({ "status": "missing", "written": false, "loaded": false, "port": 7797 });
        let row = dashboard_row("ai2", &missing, &up(Some("0.1.2")), "0.1.2");
        assert_eq!(row.state, State::Fail);
        assert_eq!(row.fix.as_deref(), Some("run ai2 setup"));
        assert!(row.detail.contains("loaded: false"));

        let unsupported = json!({ "status": "unsupported" });
        let row = dashboard_row("ai2", &unsupported, &Answer::default(), "0.1.2");
        assert_eq!(row.state, State::Warn);
        assert!(row.fix.is_none());
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
}
