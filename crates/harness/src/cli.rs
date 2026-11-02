use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, Result};
use clap::Subcommand;
use serde::Serialize;
use serde_json::json;

use crate::job::{self, Event};
use crate::line::{takes_personality, task_line};
use crate::setup::{Registry, Setup};
use crate::state::{is_connected, on_path, workspace_path, State, States};

const INSTALL: Duration = Duration::from_secs(20 * 60);
const SIGN_IN: Duration = Duration::from_secs(15 * 60);

#[derive(Subcommand, Debug)]
pub enum HarnessCommand {
    List,
    Install {
        id: String,
    },
    SignIn {
        id: String,
    },
    Command {
        id: String,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        personality: Option<String>,
        #[arg(long, default_value = "")]
        prompt: String,
        #[arg(long)]
        folder: Option<String>,
    },
}

pub struct Context {
    pub registry: Registry,
    pub home: PathBuf,
    pub state: PathBuf,
}

pub fn print(value: &impl Serialize) {
    let mut out = std::io::stdout().lock();
    let _ = serde_json::to_writer(&mut out, value)
        .map_err(std::io::Error::from)
        .and_then(|_| {
            out.write_all(b"\n")?;
            out.flush()
        });
}

impl Context {
    fn path(&self) -> String {
        workspace_path(&self.home, &std::env::var("PATH").unwrap_or_default())
    }

    fn setup(&self, id: &str) -> Result<&Setup> {
        self.registry
            .get(id)
            .ok_or_else(|| anyhow!("There is no harness called {id}."))
    }

    fn listed(&self, setup: &Setup, path: &str) -> serde_json::Value {
        let known = States::at(&self.state).of(&setup.id);
        let state = State {
            installed: known.installed || on_path(&setup.command, path),
            ..known
        };
        json!({
            "id": setup.id,
            "label": setup.label,
            "kind": if setup.is_agent() { "agent" } else { "tool" },
            "command": setup.command,
            "logo": setup.logo,
            "canSignIn": setup.sign_in.is_some(),
            "signInHint": setup.sign_in_hint,
            "models": setup.models(),
            "use": setup.use_text,
            "takesPersonality": takes_personality(setup),
            "headless": setup.headless().is_some(),
            "installed": state.installed,
            "signedIn": state.signed_in,
            "connected": is_connected(setup, state),
        })
    }

    fn job(&self, setup: &Setup, install: bool) -> Result<bool> {
        let line = if install {
            Some(&setup.install)
        } else {
            setup.sign_in.as_ref()
        };
        let line =
            line.ok_or_else(|| anyhow!("{} signs in the first time it runs.", setup.label))?;
        let input = std::iter::from_fn(|| {
            let mut line = String::new();
            std::io::stdin()
                .read_line(&mut line)
                .ok()
                .filter(|&read| read > 0)
                .map(|_| line)
        });
        let ended = job::run(
            line,
            &self.path(),
            if install { INSTALL } else { SIGN_IN },
            input,
            print,
        )?;
        if ended.ok {
            let states = States::at(&self.state);
            let before = states.of(&setup.id);
            let _ = states.set(
                &setup.id,
                State {
                    installed: true,
                    signed_in: !install || before.signed_in,
                },
            );
            if install {
                self.registry.after_install(setup, &self.home);
            }
        }
        print(&Event::Done {
            ok: ended.ok,
            message: ended.message,
        });
        Ok(ended.ok)
    }

    pub fn run(&self, command: HarnessCommand) -> Result<bool> {
        match command {
            HarnessCommand::List => {
                let path = self.path();
                let harnesses: Vec<_> = self
                    .registry
                    .all()
                    .into_iter()
                    .map(|setup| self.listed(setup, &path))
                    .collect();
                print(&json!({ "harnesses": harnesses }));
                Ok(true)
            }
            HarnessCommand::Install { id } => self.job(self.setup(&id)?, true),
            HarnessCommand::SignIn { id } => self.job(self.setup(&id)?, false),
            HarnessCommand::Command {
                id,
                model,
                personality,
                prompt,
                folder,
            } => {
                let setup = self
                    .registry
                    .agent(&id)
                    .ok_or_else(|| anyhow!("There is no harness called {id}."))?;
                let folder = match folder {
                    Some(folder) => folder,
                    None => std::env::current_dir()?.display().to_string(),
                };
                let line = task_line(
                    setup,
                    &prompt,
                    &folder,
                    model.as_deref(),
                    personality.as_deref(),
                );
                print(&json!({ "id": setup.id, "line": line }));
                Ok(true)
            }
        }
    }
}
