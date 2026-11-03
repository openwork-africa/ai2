pub use crate::rrf::*;
#[cfg(feature = "harness")]
pub use ::harness::*;
#[cfg(feature = "harness")]
pub use command::{harness_command, run_harness, HarnessSubcommand};
#[cfg(feature = "harness")]
pub use skill_command::{install_skill, SkillSubcommand};

#[cfg(feature = "harness")]
mod skill_command {
    use crate::cli::Subcommand;
    use anyhow::{anyhow, Result};
    use clap::{Arg, ArgMatches};
    use harness::Registry;
    use std::path::{Path, PathBuf};
    use std::process::ExitCode;

    pub struct SkillSubcommand {
        pub registry: Registry,
    }

    pub fn install_skill(registry: &Registry, id: &str, home: &Path) -> Result<PathBuf> {
        let setup = registry
            .get(id)
            .ok_or_else(|| anyhow!("There is no harness called {id}."))?;
        harness::skill::install(setup, home)
    }

    impl Subcommand for SkillSubcommand {
        fn command(&self) -> clap::Command {
            clap::Command::new("skill")
                .about("Adds the ai2 skill to a coding agent's skills folder")
                .subcommand_required(true)
                .subcommand(
                    clap::Command::new("install")
                        .about("Writes <skills folder>/ai2/SKILL.md for a harness, such as claude or codex")
                        .arg(Arg::new("harness").required(true).help("The harness id, as ai2 harness list names it")),
                )
        }

        fn run(&self, matches: &ArgMatches) -> Result<ExitCode> {
            let Some(("install", sub)) = matches.subcommand() else {
                return Ok(ExitCode::from(64));
            };
            let id = sub
                .get_one::<String>("harness")
                .cloned()
                .unwrap_or_default();
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .ok_or_else(|| anyhow!("HOME is not set"))?;
            let path = install_skill(&self.registry, &id, &home)?;
            println!(
                "{}",
                serde_json::json!({ "harness": id, "path": path.display().to_string() })
            );
            Ok(ExitCode::SUCCESS)
        }
    }
}

#[cfg(feature = "harness")]
mod command {
    use crate::cli::{exit_code, Subcommand};
    use anyhow::Result;
    use clap::{ArgMatches, FromArgMatches, Subcommand as _};
    use harness::cli::{Context as HarnessContext, HarnessCommand};
    use harness::Registry;
    use std::path::PathBuf;
    use std::process::ExitCode;

    pub struct HarnessSubcommand {
        pub registry: Registry,
        pub state: PathBuf,
    }
    impl HarnessSubcommand {
        pub fn new(registry: Registry, state: PathBuf) -> Self {
            Self { registry, state }
        }
    }
    impl Subcommand for HarnessSubcommand {
        fn command(&self) -> clap::Command {
            harness_command()
        }
        fn run(&self, matches: &ArgMatches) -> Result<ExitCode> {
            run_harness(self.registry.clone_for_run(), self.state.clone(), matches).map(exit_code)
        }
    }

    trait RegistryRun {
        fn clone_for_run(&self) -> Registry;
    }
    impl RegistryRun for Registry {
        fn clone_for_run(&self) -> Registry {
            let mut registry = Registry::default();
            for setup in self.all() {
                registry.put(setup.clone(), None);
            }
            registry
        }
    }

    pub fn harness_command() -> clap::Command {
        HarnessCommand::augment_subcommands(
            clap::Command::new("harness")
                .about(
                    "Installs coding agents, signs in to them and prints their headless command lines",
                )
                .subcommand_required(true),
        )
    }

    pub fn run_harness(registry: Registry, state: PathBuf, matches: &ArgMatches) -> Result<bool> {
        let command = HarnessCommand::from_arg_matches(matches)?;
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        (HarnessContext {
            registry,
            home,
            state,
        })
        .run(command)
    }
}
