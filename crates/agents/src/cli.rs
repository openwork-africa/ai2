use std::io::Write;
use std::process::ExitCode;

use clap::{ArgMatches, FromArgMatches, Parser, Subcommand};
use serde::Serialize;

use crate::definition::NewAgent;
use crate::error::{Problem, Result};
use crate::model::{AgentFound, AgentList, Removed, RunAsk};
use crate::service::Agents;

const USAGE: u8 = 64;

#[derive(Subcommand, Debug)]
enum AgentsCommand {
    #[command(about = "Keep an agent, or replace the one with the same name")]
    Create {
        name: String,
        #[arg(long, help = "The harness it runs on, e.g. claude or pi")]
        harness: String,
        #[arg(long)]
        model: Option<String>,
        #[arg(long, help = "Its system prompt; a short one is expanded")]
        personality: Option<String>,
        #[arg(long, help = "The folder it works in when a run names none")]
        folder: Option<String>,
        #[arg(long, help = "<provider>/<model-id> that expands a short personality")]
        enhancer: Option<String>,
    },
    #[command(about = "Every agent, by name")]
    List,
    #[command(about = "One agent")]
    Show { name: String },
    #[command(about = "Expand an agent's short personality into a full system prompt")]
    Enhance {
        name: Option<String>,
        #[arg(long, help = "Expand these words instead of a kept agent's")]
        personality: Option<String>,
        #[arg(long, help = "<provider>/<model-id> that expands it")]
        enhancer: Option<String>,
    },
    #[command(about = "Forget an agent")]
    Remove { name: String },
}

#[derive(Parser, Debug)]
#[command(
    name = "run",
    about = "Runs an agent, or a harness, on a prompt in a folder, then reviews what it did"
)]
struct RunCommand {
    #[arg(long, conflicts_with = "harness", required_unless_present = "harness")]
    agent: Option<String>,
    #[arg(long)]
    harness: Option<String>,
    #[arg(long)]
    model: Option<String>,
    #[arg(long)]
    personality: Option<String>,
    #[arg(long, help = "The folder it works in [default: the agent's]")]
    folder: Option<String>,
    prompt: String,
}

pub fn command() -> clap::Command {
    AgentsCommand::augment_subcommands(
        clap::Command::new("agents")
            .about("Agents: a harness, a model and a personality kept by name")
            .subcommand_required(true),
    )
}

pub fn run_command() -> clap::Command {
    <RunCommand as clap::CommandFactory>::command()
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

fn ended(done: Result<bool>) -> ExitCode {
    match done {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(problem) => {
            print(&problem);
            if problem.status == 400 {
                ExitCode::from(USAGE)
            } else {
                ExitCode::FAILURE
            }
        }
    }
}

pub fn run(matches: &ArgMatches, agents: &Agents) -> anyhow::Result<ExitCode> {
    let command = AgentsCommand::from_arg_matches(matches)?;
    Ok(ended(execute(command, agents)))
}

pub fn start(matches: &ArgMatches, agents: &Agents) -> anyhow::Result<ExitCode> {
    let command = RunCommand::from_arg_matches(matches)?;
    let ask = RunAsk {
        agent: command.agent,
        harness: command.harness,
        model: command.model,
        personality: command.personality,
        folder: command.folder,
        prompt: command.prompt,
    };
    Ok(ended(
        agents
            .run(&ask, &mut |event| print(event))
            .map(|finished| finished.run.ok()),
    ))
}

fn execute(command: AgentsCommand, agents: &Agents) -> Result<bool> {
    match command {
        AgentsCommand::Create {
            name,
            harness,
            model,
            personality,
            folder,
            enhancer,
        } => {
            let new = NewAgent {
                name,
                harness,
                model,
                personality,
                folder,
            };
            let agent = agents.create(&new, enhancer.as_deref())?;
            print(&AgentFound { agent });
        }
        AgentsCommand::List => print(&AgentList {
            agents: agents.list()?,
        }),
        AgentsCommand::Show { name } => print(&AgentFound {
            agent: agents.get(&name)?,
        }),
        AgentsCommand::Enhance {
            name,
            personality,
            enhancer,
        } => match (name, personality) {
            (Some(name), None) => print(&agents.enhance(&name, enhancer.as_deref())?),
            (None, Some(words)) => print(&agents.enhance_words(&words, "", enhancer.as_deref())),
            (Some(_), Some(_)) => {
                return Err(Problem::invalid(
                    "usage",
                    "Give an agent's name or --personality, not both.",
                ))
            }
            (None, None) => {
                return Err(Problem::invalid(
                    "usage",
                    "Give the agent's name, or --personality with the words to expand.",
                ))
            }
        },
        AgentsCommand::Remove { name } => {
            agents.remove(&name)?;
            print(&Removed { removed: name });
        }
    }
    Ok(true)
}
