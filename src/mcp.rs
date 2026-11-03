use crate::cli::{exit_code, Build, Subcommand};
use anyhow::Result;
use clap::ArgMatches;
use std::process::ExitCode;

pub struct McpSubcommand {
    pub build: Build,
}

pub fn pieces(build: Build) -> Result<::mcp::Pieces> {
    #[cfg_attr(not(feature = "agents"), allow(unused_mut))]
    let mut pieces = ::mcp::Pieces::local(
        build.name,
        build.version,
        std::env::current_exe()?,
        crate::cli::state_dir(build.name),
    );
    #[cfg(feature = "agents")]
    {
        pieces.agents = crate::agents::local(build);
    }
    Ok(pieces)
}

impl Subcommand for McpSubcommand {
    fn command(&self) -> clap::Command {
        ::mcp::command()
    }
    fn run(&self, matches: &ArgMatches) -> Result<ExitCode> {
        ::mcp::run(matches, pieces(self.build)?).map(exit_code)
    }
}
