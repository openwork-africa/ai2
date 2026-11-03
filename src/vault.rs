use crate::cli::{default_out, exit_code, providers_file, Build, Subcommand};
use crate::model::Model;
use anyhow::{anyhow, bail, Context, Result};
use clap::ArgMatches;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use vault::{KeyInfo, NoVetter, Outcome, Proposal, Resolver, Serve, Verdict, Vetter, Vetting};

pub struct ModelVetter(pub Model);

impl Vetter for ModelVetter {
    fn propose(&self, key: &KeyInfo) -> Result<Vec<String>> {
        Ok(self
            .0
            .ask::<Proposal>(vault::PROPOSE_SYSTEM, &key.input())?
            .prefixes)
    }
    fn vet(&self, call: &Vetting) -> Result<Verdict> {
        self.0.ask::<Verdict>(vault::VET_SYSTEM, &call.input())
    }
}

pub struct VaultSubcommand {
    pub name: &'static str,
}

impl VaultSubcommand {
    pub fn new(name: &'static str) -> Self {
        Self { name }
    }

    fn options(&self) -> vault::Options {
        let name = self.name;
        vault::Options::new(
            &crate::cli::state_dir(name),
            Box::new(move |serve| run_serve(name, serve)),
        )
    }
}

impl Subcommand for VaultSubcommand {
    fn command(&self) -> clap::Command {
        vault::command_with(vec![vault_ui::command()])
    }
    fn run(&self, matches: &ArgMatches) -> Result<ExitCode> {
        let options = self.options();
        match vault::outcome(matches, &options)? {
            Outcome::Code(code) => Ok(ExitCode::from(code as u8)),
            Outcome::Extra(name, sub) if name == "ui" => {
                let report = sub
                    .get_one::<PathBuf>("report")
                    .map(|path| report_file(path))
                    .transpose()?;
                vault_ui::run(report, &options.admin).map(exit_code)
            }
            Outcome::Extra(name, _) => Err(anyhow!("unknown vault subcommand {name}")),
        }
    }
}

fn report_file(path: &Path) -> Result<PathBuf> {
    if path.is_file() {
        return Ok(path.to_path_buf());
    }
    let folder = path
        .canonicalize()
        .with_context(|| format!("{} is not a folder or a report", path.display()))?;
    let reviewed = default_out(Build::ai2(), &folder).join("report.html");
    if reviewed.is_file() {
        return Ok(reviewed);
    }
    let inside = folder.join("report.html");
    if inside.is_file() {
        return Ok(inside);
    }
    bail!(
        "There is no report.html for {}: run ai2 on it first.",
        folder.display()
    )
}

pub struct SetupSubcommand {
    pub name: &'static str,
}

impl Subcommand for SetupSubcommand {
    fn command(&self) -> clap::Command {
        setup::command()
    }
    fn run(&self, matches: &ArgMatches) -> Result<ExitCode> {
        let bin = std::env::current_exe()?;
        setup::run(
            matches,
            &setup::Options::new(bin, crate::cli::state_dir(self.name)),
        )
        .map(exit_code)
    }
}

fn run_serve(name: &str, serve: Serve) -> Result<()> {
    let platform = keys::platform()?;
    let mut config = serve.config;
    config
        .owners
        .extend(owners(std::env::var("AI2_VAULT_OWNERS").ok().as_deref()));
    let file = providers_file(name, serve.providers);
    let model = serve.model;
    vault::start(config, platform, move |resolver: Resolver| {
        let Some(file) = file else {
            eprintln!("{name} vault: no providers file, so calls that need vetting are refused");
            return Box::new(NoVetter);
        };
        match Model::load(&file, model.as_deref()) {
            Ok(m) => Box::new(ModelVetter(m.with_vault(Arc::new(move |key: &str| {
                resolver(key).map(|secret| secret.as_str().to_string())
            })))),
            Err(error) => {
                eprintln!("{name} vault: no vetting model: {error:#}");
                Box::new(NoVetter)
            }
        }
    })?
    .wait();
    Ok(())
}

fn owners(listed: Option<&str>) -> Vec<u32> {
    listed
        .unwrap_or_default()
        .split(',')
        .filter_map(|uid| uid.trim().parse().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn owners_come_from_a_comma_list_of_uids() {
        assert_eq!(super::owners(Some("501, 1000,x")), vec![501, 1000]);
        assert!(super::owners(None).is_empty());
    }
}
