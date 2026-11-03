use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, Result};
use clap::Subcommand;
use serde::Serialize;
use serde_json::json;

use crate::job::{self, Ended, Event};
use crate::line::{takes_personality, task_line};
use crate::setup::{ModelChoice, Registry, Setup};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Agent,
    Tool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct Listed {
    pub id: String,
    pub label: String,
    pub kind: Kind,
    pub command: String,
    pub logo: Option<String>,
    pub can_sign_in: bool,
    pub sign_in_hint: Option<String>,
    pub models: Vec<ModelChoice>,
    #[serde(rename = "use")]
    pub use_text: Option<String>,
    pub takes_personality: bool,
    pub headless: bool,
    pub installed: bool,
    pub signed_in: bool,
    pub connected: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CommandLine {
    pub id: String,
    pub line: String,
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

    fn listed(&self, setup: &Setup, path: &str) -> Listed {
        let known = States::at(&self.state).of(&setup.id);
        let state = State {
            installed: known.installed || on_path(&setup.command, path),
            ..known
        };
        Listed {
            id: setup.id.clone(),
            label: setup.label.clone(),
            kind: if setup.is_agent() {
                Kind::Agent
            } else {
                Kind::Tool
            },
            command: setup.command.clone(),
            logo: setup.logo.clone(),
            can_sign_in: setup.sign_in.is_some(),
            sign_in_hint: setup.sign_in_hint.clone(),
            models: setup.models().to_vec(),
            use_text: setup.use_text.clone(),
            takes_personality: takes_personality(setup),
            headless: setup.headless().is_some(),
            installed: state.installed,
            signed_in: state.signed_in,
            connected: is_connected(setup, state),
        }
    }

    pub fn list(&self) -> Vec<Listed> {
        let path = self.path();
        self.registry
            .all()
            .into_iter()
            .map(|setup| self.listed(setup, &path))
            .collect()
    }

    pub fn job(
        &self,
        id: &str,
        install: bool,
        input: impl Iterator<Item = String> + Send + 'static,
        emit: impl FnMut(&Event),
    ) -> Result<Ended> {
        let setup = self.setup(id)?;
        let line = if install {
            Some(&setup.install)
        } else {
            setup.sign_in.as_ref()
        };
        let line =
            line.ok_or_else(|| anyhow!("{} signs in the first time it runs.", setup.label))?;
        let ended = job::run(
            line,
            &self.path(),
            if install { INSTALL } else { SIGN_IN },
            input,
            emit,
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
        Ok(ended)
    }

    fn printed_job(&self, id: &str, install: bool) -> Result<bool> {
        let input = std::iter::from_fn(|| {
            let mut line = String::new();
            std::io::stdin()
                .read_line(&mut line)
                .ok()
                .filter(|&read| read > 0)
                .map(|_| line)
        });
        let ended = self.job(id, install, input, print)?;
        print(&Event::Done {
            ok: ended.ok,
            message: ended.message,
        });
        Ok(ended.ok)
    }

    pub fn command_line(
        &self,
        id: &str,
        prompt: &str,
        folder: Option<String>,
        model: Option<&str>,
        personality: Option<&str>,
    ) -> Result<CommandLine> {
        let setup = self
            .registry
            .agent(id)
            .ok_or_else(|| anyhow!("There is no harness called {id}."))?;
        let folder = match folder {
            Some(folder) => folder,
            None => std::env::current_dir()?.display().to_string(),
        };
        Ok(CommandLine {
            id: setup.id.clone(),
            line: task_line(setup, prompt, &folder, model, personality),
        })
    }

    pub fn run(&self, command: HarnessCommand) -> Result<bool> {
        match command {
            HarnessCommand::List => {
                print(&json!({ "harnesses": self.list() }));
                Ok(true)
            }
            HarnessCommand::Install { id } => self.printed_job(&id, true),
            HarnessCommand::SignIn { id } => self.printed_job(&id, false),
            HarnessCommand::Command {
                id,
                model,
                personality,
                prompt,
                folder,
            } => {
                print(&self.command_line(
                    &id,
                    &prompt,
                    folder,
                    model.as_deref(),
                    personality.as_deref(),
                )?);
                Ok(true)
            }
        }
    }
}
