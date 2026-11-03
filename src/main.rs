use std::process::ExitCode;
fn main() -> ExitCode {
    let build = ai2::Build::ai2();
    ai2::run_with(
        ai2::builtin(),
        vec![
            Box::new(ai2::HarnessSubcommand::new(
                harness::Registry::public(),
                ai2::cli::state_dir("ai2"),
            )),
            Box::new(ai2::VaultSubcommand::new("ai2")),
            Box::new(ai2::SetupSubcommand { name: "ai2" }),
        ],
        build,
    )
}
