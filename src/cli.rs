use crate::harness::Rrf;
use crate::report::{self, Card, Review};
use crate::step::{Run, Step};
use crate::steps::{self, Msr};
use crate::{model, scan, sources};
use anyhow::{Context, Result};
use clap::{ArgMatches, CommandFactory, FromArgMatches, Parser, Subcommand};
use harness::cli::{print, Context as HarnessContext, HarnessCommand};
use harness::Registry;
use std::path::{Path, PathBuf};

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
    #[arg(
        long,
        env = "AI2_PROVIDERS",
        help = "A providers file, as dm and pi take them [default: ~/.ai2/providers.json]"
    )]
    providers: Option<PathBuf>,
    #[arg(
        long,
        help = "<provider>/<model-id> [default: the providers file's default]"
    )]
    model: Option<String>,
    #[arg(long, default_value = "auto", value_parser = ["auto", "muse", "pi", "none"], help = "The coding harness for the Run Risk Factor")]
    harness: String,
    #[arg(
        long,
        default_value_t = 7,
        help = "How many days back an exploit counts as recent"
    )]
    days: u64,
    #[arg(
        long,
        help = "Where review.json and report.html go [default: <run-path>/.ai2]"
    )]
    out: Option<PathBuf>,
    #[arg(long, help = "Print the review as JSON")]
    json: bool,
    #[arg(long, help = "Write review.json alone, without report.html")]
    disable_dashboard: bool,
}

pub fn builtin() -> Vec<Box<dyn Step>> {
    vec![Box::new(Msr), Box::new(Rrf)]
}

pub fn run(steps: Vec<Box<dyn Step>>) -> Result<()> {
    run_with(steps, Vec::new(), |_, _| Ok(()))
}

pub fn run_with(
    steps: Vec<Box<dyn Step>>,
    commands: Vec<clap::Command>,
    other: impl FnOnce(&str, &ArgMatches) -> Result<()>,
) -> Result<()> {
    let names: Vec<_> = steps.iter().map(|s| s.scale().name).collect();
    let bin = std::env::args()
        .next()
        .and_then(|a| {
            PathBuf::from(a)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "ai2".into());
    let command = steps.iter().flat_map(|s| s.args()).fold(
        Cli::command()
            .name(bin)
            .about(format!("Rates a coding agent's run: {}", names.join(", "))),
        |c, arg| c.arg(arg),
    );
    let args = command
        .subcommands(commands)
        .args_conflicts_with_subcommands(true)
        .get_matches();
    if let Some((name, matches)) = args.subcommand() {
        return other(name, matches);
    }
    let cli = Cli::from_arg_matches(&args)?;
    let folder = cli
        .run_path
        .canonicalize()
        .with_context(|| format!("{} is not a folder", cli.run_path.display()))?;
    let mut notes = Vec::new();
    let providers = cli.providers.clone().or_else(|| {
        std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(".ai2/providers.json"))
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

    progress("reading the folder and the environment");
    let spec = scan::scan(&folder, model.as_ref(), &mut notes);
    let mut run = Run {
        folder: folder.clone(),
        objective: cli.objective,
        changed: cli.changed,
        providers,
        model_name: cli.model,
        model,
        harness: cli.harness,
        days: cli.days,
        spec,
        exploits: Vec::new(),
        findings: Vec::new(),
        args,
    };
    let mut ratings = Vec::new();
    let mut harness = None;
    for step in &steps {
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
        match step.rate(&run) {
            Ok(rated) => {
                run.exploits.extend(rated.exploits);
                run.findings.extend(rated.findings);
                harness = harness.or(rated.harness);
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
        ratings.push(card);
    }

    let risks: Vec<(String, f64)> = run
        .exploits
        .iter()
        .map(|e| (e.technology.clone(), steps::weight(&e.severity)))
        .chain(
            run.findings
                .iter()
                .map(|f| (f.technology.clone(), steps::weight(&f.severity))),
        )
        .collect();
    let review = Review {
        version: env!("CARGO_PKG_VERSION"),
        folder: folder.display().to_string(),
        generated_at: sources::now(),
        harness,
        technologies: steps::technologies(&run.spec, &risks),
        ratings,
        objective: run.objective,
        exploits: run.exploits,
        findings: run.findings,
        spec: run.spec,
        notes,
    };

    let out = cli.out.unwrap_or_else(|| folder.join(".ai2"));
    let written = write(&review, &out, !cli.disable_dashboard)?;
    if cli.json {
        println!("{}", serde_json::to_string_pretty(&review)?);
    } else {
        println!("{}", written.display());
    }
    Ok(())
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

pub fn run_harness(registry: Registry, state: PathBuf, matches: &ArgMatches) -> Result<()> {
    let command = HarnessCommand::from_arg_matches(matches)?;
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    match (HarnessContext {
        registry,
        home,
        state,
    })
    .run(command)
    {
        Ok(true) => Ok(()),
        Ok(false) => std::process::exit(1),
        Err(error) => {
            print(&serde_json::json!({ "error": error.to_string() }));
            std::process::exit(1)
        }
    }
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
    eprintln!("ai2: {what}");
}
