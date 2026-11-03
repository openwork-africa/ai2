use std::process::ExitCode;
fn main() -> ExitCode {
    let build = ai2::Build::ai2();
    ai2::run_with(ai2::builtin(), ai2::subcommands(build), build)
}
