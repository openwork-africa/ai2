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
    use anyhow::{anyhow, Result};
    use clap::{ArgMatches, FromArgMatches, Subcommand as _};
    use harness::cli::{Context as HarnessContext, HarnessCommand};
    use harness::Registry;
    use std::path::PathBuf;
    use std::process::ExitCode;
    use std::sync::Mutex;

    pub struct HarnessSubcommand {
        registry: Mutex<Option<Registry>>,
        pub state: PathBuf,
    }
    impl HarnessSubcommand {
        pub fn new(registry: Registry, state: PathBuf) -> Self {
            Self {
                registry: Mutex::new(Some(registry)),
                state,
            }
        }
    }
    impl Subcommand for HarnessSubcommand {
        fn command(&self) -> clap::Command {
            harness_command()
        }
        fn run(&self, matches: &ArgMatches) -> Result<ExitCode> {
            let registry = self
                .registry
                .lock()
                .map_err(|_| anyhow!("the harness registry is poisoned"))?
                .take()
                .ok_or_else(|| anyhow!("the harness subcommand runs once"))?;
            run_harness(registry, self.state.clone(), matches).map(exit_code)
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

#[cfg(all(test, feature = "harness"))]
mod tests {
    use super::*;
    use crate::cli::Subcommand;

    #[test]
    fn an_install_runs_the_registrys_hooks() {
        let state = tempfile::tempdir().unwrap();
        let marker = state.path().join("hook-ran");
        let mut registry = Registry::default();
        registry.put(
            Setup {
                id: "noop".into(),
                label: "Noop".into(),
                command: "true".into(),
                install: "true".into(),
                ..Setup::default()
            },
            None,
        );
        let seen = marker.clone();
        registry.on_installed(Box::new(move |setup, _| {
            std::fs::write(&seen, &setup.id)?;
            Ok(())
        }));
        let sub = HarnessSubcommand::new(registry, state.path().to_path_buf());
        let matches = sub
            .command()
            .try_get_matches_from(["harness", "install", "noop"])
            .unwrap();
        let _ = sub.run(&matches).unwrap();
        assert_eq!(std::fs::read_to_string(&marker).unwrap(), "noop");
    }

    fn direct_registry(marker: &std::path::Path) -> Registry {
        let mut registry = Registry::default();
        registry.put(
            Setup {
                id: "noop".into(),
                label: "Noop".into(),
                command: "true".into(),
                install: "true".into(),
                sign_in: Some("true".into()),
                ..Setup::default()
            },
            None,
        );
        let seen = marker.to_path_buf();
        registry.on_installed(Box::new(move |setup, _| {
            std::fs::write(&seen, &setup.id)?;
            Ok(())
        }));
        registry
    }

    #[test]
    fn an_install_runs_directly_in_this_terminal_by_default() {
        let state = tempfile::tempdir().unwrap();
        let marker = state.path().join("hook-ran");
        let sub = HarnessSubcommand::new(direct_registry(&marker), state.path().to_path_buf());
        let matches = sub
            .command()
            .try_get_matches_from(["harness", "install", "noop"])
            .unwrap();
        let _ = sub.run(&matches).unwrap();
        assert_eq!(std::fs::read_to_string(&marker).unwrap(), "noop");
    }

    #[test]
    fn a_direct_sign_in_reports_a_failing_command() {
        let state = tempfile::tempdir().unwrap();
        let marker = state.path().join("hook-ran");
        let mut registry = direct_registry(&marker);
        registry.put(
            Setup {
                id: "broke".into(),
                label: "Broke".into(),
                command: "false".into(),
                install: "true".into(),
                sign_in: Some("false".into()),
                ..Setup::default()
            },
            None,
        );
        let sub = HarnessSubcommand::new(registry, state.path().to_path_buf());
        let matches = sub
            .command()
            .try_get_matches_from(["harness", "sign-in", "broke"])
            .unwrap();
        let failed = sub.run(&matches).unwrap_err();
        assert!(failed.to_string().contains("error (1)"), "{failed:?}");
    }

    #[test]
    fn web_keeps_the_json_event_stream() {
        let state = tempfile::tempdir().unwrap();
        let marker = state.path().join("hook-ran");
        let sub = HarnessSubcommand::new(direct_registry(&marker), state.path().to_path_buf());
        let matches = sub
            .command()
            .try_get_matches_from(["harness", "sign-in", "noop", "--web"])
            .unwrap();
        let _ = sub.run(&matches).unwrap();
    }
}
