#[cfg(not(target_os = "linux"))]
compile_error!("ai2-muse builds and runs on Linux only");

use std::process::ExitCode;

fn main() -> ExitCode {
    let build = ai2::Build {
        name: "ai2-muse",
        version: env!("CARGO_PKG_VERSION"),
    };
    ai2::run_with(ai2::builtin(), ai2::subcommands(build), build)
}
