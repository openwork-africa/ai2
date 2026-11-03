use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let home = std::env::var_os("AI2_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("ai2-ui-example"));
    let admin = std::env::var_os("AI2_VAULT_ADMIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join("vault").join("admin.sock"));
    let matches = ui::command().get_matches();
    let agents = agents::Agents::local(&home, harness::Registry::public);
    ui::run(
        &matches,
        &ui::Options {
            home,
            admin,
            agents,
        },
    )?;
    Ok(())
}
