pub mod admin;
pub mod files;
pub mod host;
pub mod info;
pub mod runner;
pub mod steps;
#[cfg(test)]
mod tests;

use admin::Admin;
use anyhow::Result;
use host::Host;
pub use info::{inputs_info, steps_info, Field, InputInfo, StepInfo};
use keys::MasterKey;
use runner::{Runner, System};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

pub use vault::{sandbox, NOT_SAFE};

#[derive(Clone, Debug)]
pub struct Options {
    pub bin: PathBuf,
    pub home: PathBuf,
    pub state: PathBuf,
    pub label: String,
    pub keychain: Option<PathBuf>,
    pub admin: PathBuf,
}

impl Options {
    pub fn new(bin: impl Into<PathBuf>, home: impl Into<PathBuf>) -> Self {
        let home = home.into();
        let state = default_state(&home);
        Self {
            bin: bin.into(),
            admin: admin::default_admin(&state),
            state,
            home,
            label: files::label(std::env::var(files::LABEL_ENV).ok()),
            keychain: keys::keychain_file(std::env::var_os(keys::KEYCHAIN_ENV)),
        }
    }
}

#[cfg(target_os = "linux")]
pub fn default_state(_ai2_home: &Path) -> PathBuf {
    PathBuf::from(keys::LINUX_STATE)
}

#[cfg(not(target_os = "linux"))]
pub fn default_state(ai2_home: &Path) -> PathBuf {
    ai2_home.join("vault")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ready,
    Missing,
    Unsupported,
    Unknown,
    Failed,
    Skipped,
}

#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub status: Status,
    pub warnings: Vec<String>,
    pub info: Map<String, Value>,
}

impl State {
    pub fn new(status: Status) -> Self {
        Self {
            status,
            warnings: Vec::new(),
            info: Map::new(),
        }
    }

    pub fn ready(ready: bool) -> Self {
        Self::new(if ready {
            Status::Ready
        } else {
            Status::Missing
        })
    }

    pub fn with(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.info.insert(key.into(), value.into());
        self
    }

    pub fn warn(mut self, warning: &str) -> Self {
        self.warnings.push(warning.into());
        self
    }

    pub fn is_ready(&self) -> bool {
        self.status == Status::Ready
    }

    pub fn to_json(&self) -> Value {
        let mut out = Map::new();
        out.insert("status".into(), json!(self.status));
        out.extend(self.info.clone());
        if !self.warnings.is_empty() {
            out.insert("warnings".into(), json!(self.warnings));
        }
        Value::Object(out)
    }
}

pub struct Context<'a> {
    pub host: &'a Host,
    pub options: &'a Options,
    pub runner: &'a dyn Runner,
    pub master: &'a dyn MasterKey,
    pub admin: &'a dyn Admin,
    pub presence: &'a dyn Fn() -> Result<bool>,
    pub check_only: bool,
}

pub trait Step {
    fn id(&self) -> &'static str;
    fn check(&self, ctx: &Context) -> Result<State>;
    fn apply(&self, ctx: &Context, checked: &State) -> Result<State>;

    fn settled(&self, _ctx: &Context, state: State) -> Result<State> {
        Ok(state)
    }
}

pub fn all() -> Vec<Box<dyn Step>> {
    vec![
        Box::new(steps::Containers),
        Box::new(steps::MasterKeyStep),
        Box::new(steps::PresenceStep),
        Box::new(steps::Daemon),
    ]
}

fn settles(step: &dyn Step, ctx: &Context) -> Result<State> {
    let checked = step.check(ctx)?;
    if ctx.check_only || checked.status == Status::Unsupported {
        return Ok(checked);
    }
    let state = if checked.is_ready() {
        checked
    } else {
        step.apply(ctx, &checked)?
    };
    step.settled(ctx, state)
}

pub fn run_steps(steps: &[Box<dyn Step>], ctx: &Context) -> (bool, Value) {
    let mut out = Map::new();
    let mut ok = true;
    let mut stopped = false;
    for step in steps {
        let state = if stopped {
            State::new(Status::Skipped)
        } else {
            ctx.runner.step_started(step.id());
            match settles(step.as_ref(), ctx) {
                Ok(state) => state,
                Err(error) => {
                    stopped = true;
                    State::new(Status::Failed).with("error", format!("{error:#}"))
                }
            }
        };
        ctx.runner.step_finished(step.id(), &state);
        for warning in &state.warnings {
            ctx.runner.say(warning);
        }
        ok &= matches!(state.status, Status::Ready | Status::Unsupported);
        out.insert(step.id().into(), state.to_json());
    }
    (
        ok,
        json!({ "ok": ok, "check": ctx.check_only, "steps": out }),
    )
}

pub fn command() -> clap::Command {
    clap::Command::new("setup")
        .about("Sets this machine up for the vault: containers, the master key, a human check and the daemon")
        .arg(
            clap::Arg::new("headless")
                .long("headless")
                .action(clap::ArgAction::SetTrue)
                .help("Run in this terminal and print one JSON object, without the web page"),
        )
}

pub const NO_UI: &str =
    "ai2 setup: this build has no web page for setup, so it runs here in the terminal, as with --headless";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Done(bool),
    Ui,
}

pub fn outcome(matches: &clap::ArgMatches, opts: &Options) -> Result<Outcome> {
    if !matches.get_flag("headless") {
        return Ok(Outcome::Ui);
    }
    headless(opts, false).map(Outcome::Done)
}

pub fn headless(opts: &Options, check_only: bool) -> Result<bool> {
    let (ok, report) = setup(opts, check_only)?;
    println!("{}", serde_json::to_string(&report)?);
    Ok(ok)
}

pub fn run(matches: &clap::ArgMatches, opts: &Options) -> Result<bool> {
    match outcome(matches, opts)? {
        Outcome::Done(ok) => Ok(ok),
        Outcome::Ui => {
            eprintln!("{NO_UI}");
            headless(opts, false)
        }
    }
}

pub fn setup(opts: &Options, check_only: bool) -> Result<(bool, Value)> {
    setup_with(opts, check_only, &System)
}

pub fn setup_with(opts: &Options, check_only: bool, runner: &dyn Runner) -> Result<(bool, Value)> {
    let host = Host::detect(runner);
    let platform = keys::platform()?;
    let admin = admin::Socket::new(&opts.admin);
    let ctx = Context {
        host: &host,
        options: opts,
        runner,
        master: platform.master.as_ref(),
        admin: &admin,
        presence: &touch_id,
        check_only,
    };
    Ok(run_steps(&all(), &ctx))
}

#[cfg(target_os = "macos")]
fn touch_id() -> Result<bool> {
    keys::macos::available()
}

#[cfg(not(target_os = "macos"))]
fn touch_id() -> Result<bool> {
    Ok(false)
}
