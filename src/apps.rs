use crate::cli::Subcommand;
use anyhow::Result;
use clap::ArgMatches;
use std::process::ExitCode;

pub struct AppsSubcommand {
    pub name: &'static str,
}

impl Subcommand for AppsSubcommand {
    fn command(&self) -> clap::Command {
        ::apps::command()
    }
    fn run(&self, matches: &ArgMatches) -> Result<ExitCode> {
        ::apps::run(
            matches,
            &::apps::Options::new(crate::cli::state_dir(self.name)),
        )
    }
}
