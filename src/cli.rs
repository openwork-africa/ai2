use crate::harness::Rrf;
use crate::report::{self, Card, Review};
use crate::step::{Context, Found, Step};
use crate::steps::{self, Msr};
use crate::{model, scan, sources};
use anyhow::{bail, Context as _, Result};
use clap::{ArgMatches, CommandFactory, FromArgMatches, Parser, Subcommand as _};
use harness::cli::{print, Context as HarnessContext, HarnessCommand};
use harness::Registry;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

static BIN_NAME: OnceLock<String> = OnceLock::new();

#[derive(Clone, Copy)]
pub struct Build {
    pub name: &'static str,
    pub version: &'static str,
}
impl Build {
    pub fn ai2() -> Self {
        Self {
            name: "ai2",
            version: env!("CARGO_PKG_VERSION"),
        }
    }
}

pub trait Subcommand {
    fn command(&self) -> clap::Command;
    fn run(&self, matches: &ArgMatches) -> Result<bool>;
}

pub struct HarnessSubcommand {
    pub registry: Registry,
    pub state: PathBuf,
}
impl HarnessSubcommand {
    pub fn new(registry: Registry, state: PathBuf) -> Self {
        Self { registry, state }
    }
}
impl Subcommand for HarnessSubcommand {
    fn command(&self) -> clap::Command {
        harness_command()
    }
    fn run(&self, matches: &ArgMatches) -> Result<bool> {
        run_harness(self.registry.clone_for_run(), self.state.clone(), matches)
    }
}

trait RegistryRun {
    fn clone_for_run(&self) -> Registry;
}
impl RegistryRun for Registry {
    fn clone_for_run(&self) -> Registry {
        let mut registry = Registry::default();
        for setup in self.all() {
            registry.put(setup.clone(), None);
        }
        registry
    }
}

#[derive(Parser)]
#[command(version)]
struct Cli {
    #[arg(long, default_value = ".", help = "The folder the run worked in")]
    run_path: PathBuf,
    #[arg(long, default_value = "", help = "What the run was asked to do")]
    objective: String,
    #[arg(
        long,
        value_delimiter = ',',
        help = "The files the run changed, relative to the run path"
    )]
    changed: Vec<String>,
    #[arg(long, help = "A providers file, as dm and pi take them")]
    providers: Option<PathBuf>,
    #[arg(
        long,
        help = "<provider>/<model-id> [default: the providers file's default]"
    )]
    model: Option<String>,
    #[arg(
        long,
        default_value_t = 7,
        help = "How many days back an exploit counts as recent"
    )]
    days: u64,
    #[arg(
        long,
        default_value_t = 35,
        help = "The whole review's time budget, in minutes"
    )]
    budget: u64,
    #[arg(long, help = "Where review.json and report.html go")]
    out: Option<PathBuf>,
    #[arg(long, help = "Print the review as JSON")]
    json: bool,
    #[arg(long, help = "Write review.json alone, without report.html")]
    disable_dashboard: bool,
}

pub fn builtin() -> Vec<Box<dyn Step>> {
    vec![Box::new(Msr), Box::new(Rrf::default())]
}
pub fn builtin_with(reviewers: Vec<crate::harness::Reviewer>) -> Vec<Box<dyn Step>> {
    vec![Box::new(Msr), Box::new(Rrf::new(reviewers))]
}

pub fn run(steps: Vec<Box<dyn Step>>) -> ExitCode {
    run_with(steps, Vec::new(), Build::ai2())
}

pub fn run_with(
    steps: Vec<Box<dyn Step>>,
    subcommands: Vec<Box<dyn Subcommand>>,
    build: Build,
) -> ExitCode {
    let _ = BIN_NAME.set(build.name.to_string());
    match execute(steps, subcommands, build) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("Error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn check_unique(steps: &[Box<dyn Step>]) -> Result<()> {
    let mut seen = Vec::new();
    for step in steps {
        if seen.contains(&step.id()) {
            bail!("duplicate step id {}", step.id());
        }
        seen.push(step.id());
    }
    Ok(())
}

fn ordered(mut steps: Vec<Box<dyn Step>>) -> Result<Vec<Box<dyn Step>>> {
    check_unique(&steps)?;
    let mut done: Vec<Box<dyn Step>> = Vec::new();
    let mut done_ids: Vec<&'static str> = Vec::new();
    while !steps.is_empty() {
        let Some(at) = steps.iter().position(|s| {
            s.after()
                .iter()
                .all(|dep| !steps.iter().any(|o| o.id() == *dep) || done_ids.contains(dep))
        }) else {
            bail!("step ordering has a cycle or a missing dependency");
        };
        let step = steps.remove(at);
        done_ids.push(step.id());
        done.push(step);
    }
    Ok(done)
}

fn dispatch(subcommands: &[Box<dyn Subcommand>], name: &str, matches: &ArgMatches) -> ExitCode {
    let Some(sub) = subcommands.iter().find(|s| s.command().get_name() == name) else {
        print(&serde_json::json!({ "error": format!("unknown subcommand {name}") }));
        return ExitCode::FAILURE;
    };
    match sub.run(matches) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            print(&serde_json::json!({ "error": error.to_string() }));
            ExitCode::FAILURE
        }
    }
}

fn execute(
    mut steps: Vec<Box<dyn Step>>,
    subcommands: Vec<Box<dyn Subcommand>>,
    build: Build,
) -> Result<ExitCode> {
    steps = ordered(steps)?;
    let names: Vec<_> = steps.iter().map(|s| s.scale().name).collect();
    let mut command = Cli::command()
        .name(build.name)
        .version(build.version)
        .about(format!("Rates a coding agent's run: {}", names.join(", ")));
    for step in &steps {
        for arg in step.args() {
            let prefixed = format!("{}.{}", step.id(), arg.get_id());
            command = command.arg(arg.id(prefixed));
        }
    }
    for sub in &subcommands {
        command = command.subcommand(sub.command());
    }
    let args = command.args_conflicts_with_subcommands(true).get_matches();
    if let Some((name, matches)) = args.subcommand() {
        let name = name.to_string();
        return Ok(dispatch(&subcommands, &name, matches));
    }
    let cli = Cli::from_arg_matches(&args)?;
    let folder = cli
        .run_path
        .canonicalize()
        .with_context(|| format!("{} is not a folder", cli.run_path.display()))?;
    let mut notes = Vec::new();
    let env_name = format!("{}_PROVIDERS", build.name.to_uppercase().replace('-', "_"));
    let providers = cli
        .providers
        .clone()
        .or_else(|| std::env::var_os(&env_name).map(PathBuf::from))
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join(format!(".{}/providers.json", build.name)))
                .filter(|p| p.is_file())
        });
    let model = providers
        .as_deref()
        .map(|file| model::Model::load(file, cli.model.as_deref()))
        .transpose()
        .unwrap_or_else(|error| {
            notes.push(format!("No model: {error:#}"));
            None
        });
    if providers.is_none() {
        notes.push("No providers file: the steps that ask a model were left out.".into());
    }
    let out = cli
        .out
        .clone()
        .unwrap_or_else(|| default_out(build, &folder));
    progress("reading the folder and the environment");
    let spec = scan::scan_skipping(&folder, model.as_ref(), &mut notes, Some(&out));
    let deadline = Instant::now() + Duration::from_secs(cli.budget.saturating_mul(60));
    let mut ctx = Context {
        folder: folder.clone(),
        objective: cli.objective.clone(),
        changed: cli.changed.clone(),
        days: cli.days,
        spec,
        model,
        args,
        deadline: Some(deadline),
    };
    let mut provision_failed: Vec<Option<String>> = Vec::new();
    for step in &mut steps {
        match step.provision(&ctx) {
            Ok(()) => provision_failed.push(None),
            Err(error) => {
                let why = format!("{} could not be prepared: {error:#}", step.scale().name);
                notes.push(why.clone());
                provision_failed.push(Some(why));
            }
        }
    }
    let mut ratings: Vec<Card> = Vec::new();
    let mut found = Found::default();
    let mut chosen_harness = None;
    let mut write_error: Option<anyhow::Error> = None;
    for (at, step) in steps.iter().enumerate() {
        let scale = step.scale();
        progress(&format!("rating the {}", scale.name));
        let mut card = Card {
            key: scale.key,
            name: scale.name,
            short: scale.short,
            riskier: scale.riskier,
            rating: None,
            missing: None,
            items: Vec::new(),
        };
        if let Some(why) = &provision_failed[at] {
            card.missing = Some(why.clone());
        } else {
            match step.rate(&ctx, &found) {
                Ok(rated) => {
                    found.exploits.extend(rated.exploits);
                    found.findings.extend(rated.findings);
                    chosen_harness = chosen_harness.or(rated.harness);
                    if rated.rating.is_none() {
                        card.missing = Some(
                            rated
                                .notes
                                .first()
                                .cloned()
                                .unwrap_or_else(|| scale.missing.into()),
                        );
                    }
                    notes.extend(rated.notes);
                    card.rating = rated.rating;
                    card.items = rated.items;
                }
                Err(error) => {
                    let why = format!("{} could not be rated: {error:#}", scale.name);
                    card.missing = Some(why.clone());
                    notes.push(why);
                }
            }
        }
        ratings.push(card);
        let review = make_review(
            build,
            &folder,
            &ctx,
            &ratings,
            &found,
            chosen_harness,
            &notes,
        );
        if let Err(error) = write(&review, &out, !cli.disable_dashboard) {
            write_error = Some(error);
            break;
        }
    }
    for step in &mut steps {
        step.cleanup();
    }
    if let Some(error) = write_error {
        return Err(error);
    }
    ctx.objective.clone_from(&cli.objective);
    let review = make_review(
        build,
        &folder,
        &ctx,
        &ratings,
        &found,
        chosen_harness,
        &notes,
    );
    let written = write(&review, &out, !cli.disable_dashboard)?;
    if cli.json {
        println!("{}", serde_json::to_string_pretty(&review)?);
    } else {
        println!("{}", written.display());
    }
    Ok(ExitCode::SUCCESS)
}

fn make_review(
    build: Build,
    folder: &Path,
    ctx: &Context,
    ratings: &[Card],
    found: &Found,
    harness: Option<&'static str>,
    notes: &[String],
) -> Review {
    let risks: Vec<(String, f64)> = found
        .exploits
        .iter()
        .map(|e| (e.technology.clone(), steps::weight(&e.severity)))
        .chain(
            found
                .findings
                .iter()
                .map(|f| (f.technology.clone(), steps::weight(&f.severity))),
        )
        .collect();
    Review {
        version: build.version,
        folder: folder.display().to_string(),
        generated_at: sources::now(),
        harness,
        technologies: steps::technologies(&ctx.spec, &risks),
        ratings: ratings.to_vec(),
        objective: ctx.objective.clone(),
        exploits: found.exploits.clone(),
        findings: found.findings.clone(),
        spec: ctx.spec.clone(),
        notes: notes.to_vec(),
    }
}

pub fn default_out(build: Build, folder: &Path) -> PathBuf {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in folder.display().to_string().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    state_dir(build.name)
        .join("reviews")
        .join(format!("{hash:016x}"))
}

pub fn write(review: &Review, out: &Path, dashboard: bool) -> Result<PathBuf> {
    std::fs::create_dir_all(out).with_context(|| format!("making {}", out.display()))?;
    std::fs::write(out.join("review.json"), serde_json::to_vec_pretty(review)?)?;
    if !dashboard {
        return Ok(out.join("review.json"));
    }
    std::fs::write(out.join("report.html"), report::html(review))?;
    Ok(out.join("report.html"))
}

pub fn harness_command() -> clap::Command {
    HarnessCommand::augment_subcommands(
        clap::Command::new("harness")
            .about(
                "Installs coding agents, signs in to them and prints their headless command lines",
            )
            .subcommand_required(true),
    )
}

pub fn run_harness(registry: Registry, state: PathBuf, matches: &ArgMatches) -> Result<bool> {
    let command = HarnessCommand::from_arg_matches(matches)?;
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    (HarnessContext {
        registry,
        home,
        state,
    })
    .run(command)
}

pub fn state_dir(name: &str) -> PathBuf {
    std::env::var_os(format!("{}_HOME", name.to_uppercase()))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join(format!(".{name}"))
        })
}

pub fn progress(what: &str) {
    let name = BIN_NAME.get().map(String::as_str).unwrap_or("ai2");
    eprintln!("{name}: {what}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::step::Rated;
    use crate::steps::MSR;
    struct Dup;
    impl Step for Dup {
        fn id(&self) -> &'static str {
            "rating.msr"
        }
        fn scale(&self) -> &'static crate::steps::Scale {
            &MSR
        }
        fn rate(&self, _ctx: &Context, _found: &Found) -> Result<Rated> {
            Ok(Rated::default())
        }
    }
    struct Fail;
    impl Subcommand for Fail {
        fn command(&self) -> clap::Command {
            clap::Command::new("fail")
        }
        fn run(&self, _matches: &ArgMatches) -> Result<bool> {
            anyhow::bail!("nope")
        }
    }
    #[test]
    fn duplicate_step_ids_are_refused() {
        let steps: Vec<Box<dyn Step>> = vec![Box::new(Dup), Box::new(Dup)];
        assert!(check_unique(&steps).is_err());
    }
    #[test]
    fn steps_are_ordered_by_their_after_list() {
        let steps: Vec<Box<dyn Step>> = vec![Box::new(Rrf::default()), Box::new(Msr)];
        let ordered_steps = ordered(steps).unwrap();
        assert_eq!(
            ordered_steps.iter().map(|s| s.id()).collect::<Vec<_>>(),
            ["rating.msr", "rating.rrf"]
        );
    }
    #[test]
    fn a_failing_subcommand_is_a_failure_exit() {
        let subs: Vec<Box<dyn Subcommand>> = vec![Box::new(Fail)];
        let cmd = Fail.command();
        let matches = cmd.try_get_matches_from(["fail"]).unwrap();
        assert_eq!(
            format!("{:?}", dispatch(&subs, "fail", &matches)),
            format!("{:?}", ExitCode::FAILURE)
        );
    }
    #[test]
    fn the_default_out_is_outside_the_reviewed_folder() {
        let out = default_out(Build::ai2(), Path::new("/tmp/example"));
        assert!(!out.starts_with("/tmp/example"));
        assert!(out.to_string_lossy().contains("reviews"));
    }
}
