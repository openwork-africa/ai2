use crate::cli::{exit_code, Build, Subcommand};
use anyhow::Result;
use clap::{Arg, ArgAction, ArgMatches};
use serde::Serialize;
use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Ok,
    Warn,
    Fail,
}

impl State {
    fn word(self) -> &'static str {
        match self {
            State::Ok => "ok",
            State::Warn => "warn",
            State::Fail => "fail",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Row {
    pub check: String,
    pub state: State,
    pub detail: String,
    pub fix: Option<String>,
}

impl Row {
    pub fn new(check: &str, state: State, detail: impl Into<String>) -> Row {
        Row {
            check: check.into(),
            state,
            detail: detail.into(),
            fix: None,
        }
    }

    pub fn ok(check: &str, detail: impl Into<String>) -> Row {
        Row::new(check, State::Ok, detail)
    }

    pub fn warn(check: &str, detail: impl Into<String>) -> Row {
        Row::new(check, State::Warn, detail)
    }

    pub fn fail(check: &str, detail: impl Into<String>) -> Row {
        Row::new(check, State::Fail, detail)
    }

    pub fn fix(mut self, fix: impl Into<String>) -> Row {
        self.fix = Some(fix.into());
        self
    }
}

pub struct DoctorSubcommand {
    pub build: Build,
}

impl Subcommand for DoctorSubcommand {
    fn command(&self) -> clap::Command {
        clap::Command::new("doctor")
            .about("Checks everything in one table: the binary, versions, the vault's setup, its sockets and sandboxes, and the vetting model")
            .arg(
                Arg::new("json")
                    .long("json")
                    .action(ArgAction::SetTrue)
                    .help("Print the checks as JSON: [{check, state, detail, fix}]"),
            )
    }

    fn run(&self, matches: &ArgMatches) -> Result<ExitCode> {
        let rows = rows(self.build);
        if matches.get_flag("json") {
            println!("{}", serde_json::to_string(&rows)?);
        } else {
            print!("{}", table(&rows));
        }
        Ok(exit_code(passes(&rows)))
    }
}

pub fn rows(build: Build) -> Vec<Row> {
    let this = std::env::current_exe().ok();
    let first = on_path(build.name, std::env::var_os("PATH").as_deref());
    #[cfg(feature = "vault")]
    let daemon = Some(crate::vault::daemon_version(build.name));
    #[cfg(not(feature = "vault"))]
    let daemon = None;
    [
        vec![
            binary(build.name, first.as_deref(), this.as_deref()),
            version(build.name, build.version, daemon),
        ],
        #[cfg(feature = "setup")]
        crate::setup::rows(build.name, build.version),
        #[cfg(feature = "vault")]
        crate::vault::rows(build.name),
    ]
    .concat()
}

pub fn passes(rows: &[Row]) -> bool {
    rows.iter().all(|row| row.state != State::Fail)
}

pub fn table(rows: &[Row]) -> String {
    let cells: Vec<[&str; 4]> = std::iter::once(["check", "state", "detail", "fix"])
        .chain(rows.iter().map(|row| {
            [
                row.check.as_str(),
                row.state.word(),
                row.detail.as_str(),
                row.fix.as_deref().unwrap_or(""),
            ]
        }))
        .collect();
    let width = |at: usize| {
        cells
            .iter()
            .map(|c| c[at].chars().count())
            .max()
            .unwrap_or(0)
    };
    let widths = [width(0), width(1), width(2)];
    cells
        .iter()
        .map(|c| {
            let line = format!(
                "{:w0$}  {:w1$}  {:w2$}  {}",
                c[0],
                c[1],
                c[2],
                c[3],
                w0 = widths[0],
                w1 = widths[1],
                w2 = widths[2]
            );
            format!("{}\n", line.trim_end())
        })
        .collect()
}

pub fn on_path(name: &str, path: Option<&OsStr>) -> Option<PathBuf> {
    std::env::split_paths(path?)
        .map(|dir| dir.join(name))
        .find(|file| {
            std::fs::metadata(file)
                .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        })
}

fn same(a: &Path, b: &Path) -> bool {
    let real = |path: &Path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    real(a) == real(b)
}

pub fn binary(name: &str, first: Option<&Path>, this: Option<&Path>) -> Row {
    const CHECK: &str = "binary";
    let Some(this) = this else {
        return Row::warn(CHECK, "cannot tell where this binary is");
    };
    let folder = this.parent().unwrap_or(this).display();
    match first {
        None => Row::warn(
            CHECK,
            format!("{} is not on PATH; this is {}", name, this.display()),
        )
        .fix(format!("add {folder} to PATH")),
        Some(first) if same(first, this) => Row::ok(CHECK, this.display().to_string()),
        Some(first) => Row::fail(
            CHECK,
            format!(
                "the first {name} on PATH is {}, not this one ({})",
                first.display(),
                this.display()
            ),
        )
        .fix(format!(
            "remove {} or put {folder} before it on PATH",
            first.display()
        )),
    }
}

pub fn version(name: &str, client: &str, daemon: Option<Result<String>>) -> Row {
    const CHECK: &str = "version";
    match daemon {
        None => Row::ok(CHECK, format!("client {client}")),
        Some(Ok(daemon)) if daemon == client => {
            Row::ok(CHECK, format!("client {client}, daemon {daemon}"))
        }
        Some(Ok(daemon)) => Row::warn(CHECK, format!("client {client}, daemon {daemon}")).fix(
            format!("run {name} setup so the daemon restarts on {client}"),
        ),
        Some(Err(error)) => Row::warn(
            CHECK,
            format!("client {client}; the daemon did not answer: {error:#}"),
        )
        .fix(format!("run {name} setup")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;

    fn executable(dir: &Path, name: &str) -> PathBuf {
        let file = dir.join(name);
        std::fs::write(&file, "").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
        file
    }

    #[test]
    fn the_first_executable_on_path_is_found() {
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        std::fs::write(a.path().join("ai2"), "").unwrap();
        let found = executable(b.path(), "ai2");
        let path = std::env::join_paths([a.path(), b.path()]).unwrap();
        assert_eq!(on_path("ai2", Some(&path)), Some(found));
        assert_eq!(on_path("ai2", None), None);
    }

    #[test]
    fn the_binary_check_fails_when_another_ai2_comes_first() {
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let (first, this) = (executable(a.path(), "ai2"), executable(b.path(), "ai2"));
        assert_eq!(binary("ai2", Some(&this), Some(&this)).state, State::Ok);
        let other = binary("ai2", Some(&first), Some(&this));
        assert_eq!(other.state, State::Fail);
        assert!(other.fix.unwrap().contains(&b.path().display().to_string()));
        assert_eq!(binary("ai2", None, Some(&this)).state, State::Warn);
    }

    #[test]
    fn a_daemon_on_another_version_is_a_warning() {
        assert_eq!(version("ai2", "0.1.2", None).detail, "client 0.1.2");
        assert_eq!(
            version("ai2", "0.1.2", Some(Ok("0.1.2".into()))).state,
            State::Ok
        );
        let older = version("ai2", "0.1.2", Some(Ok("0.1.1".into())));
        assert_eq!(
            (older.state, older.detail.as_str()),
            (State::Warn, "client 0.1.2, daemon 0.1.1")
        );
        assert_eq!(
            version("ai2", "0.1.2", Some(Err(anyhow!("down")))).state,
            State::Warn
        );
    }

    #[test]
    fn only_a_failure_fails_the_doctor() {
        let warned = vec![Row::ok("a", "fine"), Row::warn("b", "hm").fix("do it")];
        assert!(passes(&warned));
        assert!(!passes(&[Row::fail("c", "broken")]));
    }

    #[test]
    fn the_table_lines_up_and_the_json_has_four_fields() {
        let rows = vec![
            Row::ok("binary", "/bin/ai2"),
            Row::warn("sandboxes", "host").fix("remove host"),
        ];
        assert_eq!(
            table(&rows),
            "check      state  detail    fix\nbinary     ok     /bin/ai2\nsandboxes  warn   host      remove host\n"
        );
        assert_eq!(
            serde_json::to_value(&rows[1]).unwrap(),
            serde_json::json!({ "check": "sandboxes", "state": "warn", "detail": "host", "fix": "remove host" })
        );
    }
}
