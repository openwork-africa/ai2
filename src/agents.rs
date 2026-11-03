use crate::cli::{default_out, providers_file, state_dir, Build, Subcommand};
use crate::model::{Asking, Model};
use ::agents::{AgentStore, Agents, Ask, Enhancer, LocalStore, Review, Reviewed};
use anyhow::{anyhow, bail, Context, Result};
use clap::ArgMatches;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::Arc;
use std::time::Duration;

const ENHANCING: Asking = Asking {
    max_tokens: Some(1200),
    temperature: Some(0.6),
    timeout: Duration::from_secs(45),
};

pub struct AgentsSubcommand {
    pub agents: Agents,
}

pub struct RunSubcommand {
    pub agents: Agents,
}

impl Subcommand for AgentsSubcommand {
    fn command(&self) -> clap::Command {
        ::agents::cli::command()
    }
    fn run(&self, matches: &ArgMatches) -> Result<ExitCode> {
        ::agents::cli::run(matches, &self.agents)
    }
}

impl Subcommand for RunSubcommand {
    fn command(&self) -> clap::Command {
        ::agents::cli::run_command()
    }
    fn run(&self, matches: &ArgMatches) -> Result<ExitCode> {
        ::agents::cli::start(matches, &self.agents)
    }
}

pub fn local(build: Build) -> Agents {
    let store = LocalStore::new(&state_dir(build.name));
    with_store(build, Arc::new(store), ::harness::Registry::public)
}

pub fn with_store(
    build: Build,
    store: Arc<dyn AgentStore>,
    registry: impl Fn() -> ::harness::Registry + Send + Sync + 'static,
) -> Agents {
    let user = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let path = ::harness::state::workspace_path(&user, &std::env::var("PATH").unwrap_or_default());
    let agents = Agents::new(store, registry, &state_dir(build.name))
        .with_path(path)
        .with_enhancer(enhancer(build));
    match std::env::current_exe() {
        Ok(bin) => agents.with_review(Arc::new(Launcher { build, bin })),
        Err(_) => agents,
    }
}

pub fn enhancer(build: Build) -> Enhancer {
    Arc::new(move |name: Option<&str>| {
        let file = providers_file(build.name, None)
            .ok_or_else(|| anyhow!("no providers file to expand a personality with"))?;
        let model = Model::load(&file, name)?;
        let ask: Ask =
            Arc::new(move |system: &str, input: &str| model.text(system, input, &ENHANCING));
        Ok(ask)
    })
}

pub struct Launcher {
    pub build: Build,
    pub bin: PathBuf,
}

pub fn arguments(folder: &Path, objective: &str) -> Vec<String> {
    vec![
        "--run-path".into(),
        folder.display().to_string(),
        "--objective".into(),
        objective.into(),
        "--json".into(),
    ]
}

impl Review for Launcher {
    fn review(
        &self,
        folder: &Path,
        objective: &str,
        say: &mut dyn FnMut(String),
    ) -> Result<Reviewed> {
        let mut child = Command::new(&self.bin)
            .args(arguments(folder, objective))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("starting {}", self.bin.display()))?;
        let mut out = child.stdout.take().context("no output")?;
        let printed = std::thread::spawn(move || {
            let mut text = String::new();
            out.read_to_string(&mut text).map(|_| text)
        });
        let mut last = String::new();
        if let Some(errors) = child.stderr.take() {
            for line in BufReader::new(errors).lines().map_while(Result::ok) {
                say(line.clone());
                last = line;
            }
        }
        let status = child.wait()?;
        let text = printed
            .join()
            .map_err(|_| anyhow!("reading the review failed"))??;
        if !status.success() {
            bail!("the review ended with an error: {last}");
        }
        let review = serde_json::from_str(&text).context("the review printed no JSON")?;
        let hash = default_out(self.build.name, folder)
            .file_name()
            .map(|hash| hash.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(Reviewed { hash, review })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn the_review_after_a_run_is_this_binary_on_the_folder_named_by_its_hash() {
        let bin = tempfile::tempdir().unwrap();
        let folder = tempfile::tempdir().unwrap();
        let script = bin.path().join("ai2");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\necho 'ai2: rating the Run Risk Factor' >&2\necho '{{\"objective\":\"fix it\"}}'\n",
                bin.path().join("args").display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let launcher = Launcher {
            build: Build::ai2(),
            bin: script.clone(),
        };
        let mut said = Vec::new();
        let reviewed = launcher
            .review(folder.path(), "fix it", &mut |line| said.push(line))
            .unwrap();
        assert_eq!(said, ["ai2: rating the Run Risk Factor"]);
        assert_eq!(reviewed.review["objective"], "fix it");
        assert_eq!(
            reviewed.hash,
            default_out("ai2", folder.path())
                .file_name()
                .unwrap()
                .to_string_lossy()
        );
        let args = std::fs::read_to_string(bin.path().join("args")).unwrap();
        assert_eq!(
            args.lines().collect::<Vec<_>>(),
            [
                "--run-path",
                &folder.path().display().to_string(),
                "--objective",
                "fix it",
                "--json"
            ]
        );
        std::fs::write(&script, "#!/bin/sh\necho 'ai2: no' >&2\nexit 1\n").unwrap();
        let failed = launcher
            .review(folder.path(), "x", &mut |_| {})
            .unwrap_err();
        assert_eq!(
            failed.to_string(),
            "the review ended with an error: ai2: no"
        );
    }
}
