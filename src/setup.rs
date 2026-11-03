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
            setup::Outcome::Ui { check } => {
                in_the_browser(self.name, options, check).map(exit_code)
            }
        }
    }
}

#[cfg(feature = "ui")]
fn in_the_browser(name: &'static str, options: setup::Options, check: bool) -> Result<bool> {
    let port = std::env::var("AI2_UI_PORT")
        .ok()
        .and_then(|port| port.parse().ok())
        .unwrap_or(ui::DEFAULT_PORT);
    crate::ui::inject(crate::cli::Build {
        name,
        ..crate::cli::Build::ai2()
    });
    ui::setup::run(
        &crate::ui::options(name),
        options,
        ui::setup::Launch {
            port,
            open: true,
            check,
        },
    )
}

#[cfg(not(feature = "ui"))]
fn in_the_browser(_name: &'static str, options: setup::Options, check: bool) -> Result<bool> {
    eprintln!("{}", setup::NO_UI);
    setup::headless(&options, check)
}

pub fn rows(name: &'static str) -> Vec<Row> {
    let report = std::env::current_exe()
        .map_err(anyhow::Error::from)
        .and_then(|bin| {
            setup::setup(&setup::Options::new(bin, crate::cli::state_dir(name)), true)
                .map(|(_, report)| report)
        });
    let mut rows = steps_rows(name, &report);
    rows.push(admin_socket(name));
    rows
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
}
