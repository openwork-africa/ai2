use crate::cli::{exit_code, Build, Subcommand};
use anyhow::Result;
use clap::ArgMatches;
use std::process::ExitCode;

pub struct McpSubcommand {
    pub build: Build,
}

pub fn pieces(build: Build) -> Result<::mcp::Pieces> {
    Ok(::mcp::Pieces::local(
        build.name,
        build.version,
        std::env::current_exe()?,
        crate::cli::state_dir(build.name),
    ))
}

impl Subcommand for McpSubcommand {
    fn command(&self) -> clap::Command {
        ::mcp::command()
    }
    fn run(&self, matches: &ArgMatches) -> Result<ExitCode> {
        ::mcp::run(matches, pieces(self.build)?).map(exit_code)
    }
}
