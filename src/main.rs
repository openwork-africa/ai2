fn main() -> anyhow::Result<()> {
    ai2::run_with(
        ai2::builtin(),
        vec![ai2::cli::harness_command()],
        |_, matches| {
            ai2::cli::run_harness(
                harness::Registry::public(),
                ai2::cli::state_dir("ai2"),
                matches,
            )
        },
    )
}
