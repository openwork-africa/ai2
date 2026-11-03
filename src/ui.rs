use crate::cli::{exit_code, Build, Subcommand};
use crate::vault::VaultSubcommand;
use anyhow::Result;
use clap::ArgMatches;
use std::process::ExitCode;

pub struct UiSubcommand {
    pub build: Build,
}

pub fn options(name: &'static str) -> ::ui::Options {
    ::ui::Options {
        home: crate::cli::state_dir(name),
        admin: VaultSubcommand::new(name).options().admin,
    }
}

impl Subcommand for UiSubcommand {
    fn command(&self) -> clap::Command {
        ::ui::command()
    }
    fn run(&self, matches: &ArgMatches) -> Result<ExitCode> {
        ::ui::run(matches, &options(self.build.name)).map(exit_code)
    }
}
