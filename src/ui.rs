use crate::cli::{default_out, exit_code, Build, Subcommand};
use crate::vault::VaultSubcommand;
use anyhow::{Context, Result};
use clap::ArgMatches;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::Arc;
use ui::reviews::{Launched, NewReview};

pub struct UiSubcommand {
    pub build: Build,
}

pub fn options(build: Build) -> ::ui::Options {
    ::ui::Options {
        home: crate::cli::state_dir(build.name),
        admin: VaultSubcommand::new(build.name).options().admin,
        agents: crate::agents::local(build),
    }
}

pub fn port() -> u16 {
    ::ui::service::port()
}

pub fn inject(build: Build) {
    let Ok(bin) = std::env::current_exe() else {
        return;
    };
    ::ui::reviews::inject(Arc::new(move |review: &NewReview| {
        launch(build, &bin, review)
    }));
}

pub fn launch(build: Build, bin: &Path, review: &NewReview) -> Result<Launched> {
    let folder = PathBuf::from(review.folder.trim())
        .canonicalize()
        .with_context(|| format!("{} is not a folder", review.folder))?;
    if !folder.is_dir() {
        anyhow::bail!("{} is not a folder", folder.display());
    }
    let hash = default_out(build, &folder)
        .file_name()
        .map(|hash| hash.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut command = Command::new(bin);
    command.args(arguments(&folder, review));
    Ok(Launched { command, hash })
}

pub fn arguments(folder: &Path, review: &NewReview) -> Vec<String> {
    let mut args = vec![
        "--run-path".to_string(),
        folder.display().to_string(),
        "--json".into(),
    ];
    let given = |value: &Option<String>| value.clone().filter(|text| !text.trim().is_empty());
    if let Some(objective) = given(&review.objective) {
        args.extend(["--objective".into(), objective]);
    }
    if !review.changed.is_empty() {
        args.extend(["--changed".into(), review.changed.join(",")]);
    }
    if let Some(providers) = given(&review.providers) {
        args.extend(["--providers".into(), providers]);
    }
    if let Some(model) = given(&review.model) {
        args.extend(["--model".into(), model]);
    }
    args
}

impl Subcommand for UiSubcommand {
    fn command(&self) -> clap::Command {
        ::ui::command()
    }
    fn run(&self, matches: &ArgMatches) -> Result<ExitCode> {
        inject(self.build);
        ::ui::run(matches, &options(self.build)).map(exit_code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_review_from_the_page_runs_this_binary_on_the_folder_and_names_its_hash() {
        let folder = tempfile::tempdir().unwrap();
        let review = NewReview {
            folder: folder.path().display().to_string(),
            objective: Some("fix the login".into()),
            changed: vec!["src/a.rs".into(), "src/b.rs".into()],
            model: Some(" ".into()),
            ..NewReview::default()
        };
        let launched = launch(Build::ai2(), Path::new("/bin/ai2"), &review).unwrap();
        let real = folder.path().canonicalize().unwrap();
        assert_eq!(launched.command.get_program(), "/bin/ai2");
        let args: Vec<String> = launched
            .command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "--run-path",
                &real.display().to_string(),
                "--json",
                "--objective",
                "fix the login",
                "--changed",
                "src/a.rs,src/b.rs"
            ]
        );
        assert_eq!(
            launched.hash,
            default_out(Build::ai2(), &real)
                .file_name()
                .unwrap()
                .to_string_lossy()
        );
        assert!(::ui::reviews::valid_hash(&launched.hash));
        let missing = NewReview {
            folder: folder.path().join("nope").display().to_string(),
            ..NewReview::default()
        };
        assert!(launch(Build::ai2(), Path::new("/bin/ai2"), &missing).is_err());
    }
}
