#[cfg(feature = "agents")]
pub mod agents;
#[cfg(feature = "apps")]
pub mod apps;
pub mod cli;
pub mod doctor;
pub mod harness;
#[cfg(feature = "mcp")]
pub mod mcp;
pub mod model;
pub mod prompts;
pub mod report;
pub mod rrf;
pub mod scan;
#[cfg(feature = "setup")]
pub mod setup;
pub mod sources;
pub mod step;
pub mod steps;
pub mod time;
#[cfg(feature = "ui")]
pub mod ui;
#[cfg(feature = "vault")]
pub mod vault;

#[cfg(feature = "agents")]
pub use agents::{AgentsSubcommand, RunSubcommand};
#[cfg(feature = "apps")]
pub use apps::AppsSubcommand;
pub use cli::{builtin, run, run_with, Build, Subcommand};
pub use doctor::DoctorSubcommand;
#[cfg(feature = "harness")]
pub use harness::{HarnessSubcommand, SkillSubcommand};
#[cfg(feature = "mcp")]
pub use mcp::McpSubcommand;
#[cfg(feature = "setup")]
pub use setup::SetupSubcommand;
pub use step::{Context, Found, Item, Rated, Step};
#[cfg(feature = "ui")]
pub use ui::UiSubcommand;
#[cfg(feature = "vault")]
pub use vault::VaultSubcommand;

pub fn subcommands(build: Build) -> Vec<Box<dyn Subcommand>> {
    vec![
        #[cfg(feature = "harness")]
        Box::new(HarnessSubcommand::new(
            crate::harness::Registry::public(),
            cli::state_dir(build.name),
        )),
        #[cfg(feature = "harness")]
        Box::new(SkillSubcommand {
            registry: crate::harness::Registry::public(),
        }),
        #[cfg(feature = "vault")]
        Box::new(VaultSubcommand::new(build.name)),
        #[cfg(feature = "setup")]
        Box::new(SetupSubcommand { name: build.name }),
        #[cfg(feature = "apps")]
        Box::new(AppsSubcommand { name: build.name }),
        #[cfg(feature = "agents")]
        Box::new(AgentsSubcommand {
            agents: crate::agents::local(build),
        }),
        #[cfg(feature = "agents")]
        Box::new(RunSubcommand {
            agents: crate::agents::local(build),
        }),
        #[cfg(feature = "ui")]
        Box::new(UiSubcommand { build }),
        #[cfg(feature = "mcp")]
        Box::new(McpSubcommand { build }),
        Box::new(DoctorSubcommand { build }),
    ]
}

#[cfg(test)]
mod tests {
    #[test]
    fn one_subcommand_per_enabled_feature_then_the_doctor() {
        let names: Vec<String> = super::subcommands(super::Build::ai2())
            .iter()
            .map(|sub| sub.command().get_name().to_string())
            .collect();
        let enabled = [
            ("harness", cfg!(feature = "harness")),
            ("skill", cfg!(feature = "harness")),
            ("vault", cfg!(feature = "vault")),
            ("setup", cfg!(feature = "setup")),
            ("apps", cfg!(feature = "apps")),
            ("agents", cfg!(feature = "agents")),
            ("run", cfg!(feature = "agents")),
            ("ui", cfg!(feature = "ui")),
            ("mcp", cfg!(feature = "mcp")),
        ];
        let expected: Vec<&str> = enabled
            .iter()
            .filter(|(_, on)| *on)
            .map(|(name, _)| *name)
            .chain(["doctor"])
            .collect();
        assert_eq!(names, expected);
    }

    #[test]
    fn features_bring_the_pieces_they_need() {
        if cfg!(feature = "setup") {
            const {
                assert!(cfg!(feature = "vault"));
            }
        }
        if cfg!(feature = "agents") {
            const {
                assert!(cfg!(feature = "harness"));
            }
        }
        if cfg!(feature = "ui") {
            const {
                assert!(cfg!(all(
                    feature = "vault",
                    feature = "setup",
                    feature = "apps",
                    feature = "agents"
                )));
            }
        }
    }
}
