use crate::error::{Problem, Result};
use crate::logs::Follow;
use crate::model::{IconUpload, LogLine, NewApp, Restart, Shown};
use crate::store::Store;
use crate::supervisor::{Settings, Supervisor};
use crate::ui::{Ui, DEFAULT_PORT};
use clap::{ArgMatches, FromArgMatches, Subcommand};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: u8 = 64;

#[derive(Subcommand, Debug)]
enum AppsCommand {
    #[command(about = "Add an app, or update the one with the same name")]
    Register {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        command: Option<String>,
        #[arg(
            long,
            help = "The app's OpenAPI document: a path under its url, or a file"
        )]
        openapi: Option<String>,
        #[arg(long, help = "The favicon: png, svg, ico, webp or jpg")]
        icon: Option<PathBuf>,
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(long)]
        port: Option<u16>,
        #[arg(long = "env", value_name = "K=V")]
        env: Vec<String>,
        #[arg(long, value_enum)]
        restart: Option<Restart>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long, help = "Do not set PORT in the app's environment")]
        no_port_env: bool,
    },
    #[command(about = "Every app with its API; --openapi inlines the documents")]
    List {
        #[arg(long)]
        openapi: bool,
    },
    #[command(about = "One app")]
    Show { id: String },
    #[command(about = "Fetch and validate the app's OpenAPI document now")]
    Check { id: String },
    #[command(about = "Print the app's OpenAPI document")]
    Api { id: String },
    #[command(about = "Start the app and wait for its port")]
    Start { id: String },
    #[command(about = "Stop the app, only when ai2 started it")]
    Stop { id: String },
    #[command(about = "Stop the app if ai2 started it, then start it")]
    Restart { id: String },
    #[command(about = "Whether the app runs, its pid, since when, its port and last exit")]
    Status { id: String },
    #[command(about = "The app's output")]
    Logs {
        id: String,
        #[arg(short = 'n', long = "lines", default_value_t = 200)]
        lines: usize,
        #[arg(long)]
        follow: bool,
    },
    #[command(about = "Remove the app from the list; a running app keeps running")]
    Remove { id: String },
}

pub struct Options {
    pub home: PathBuf,
    pub ui_port: u16,
}

impl Options {
    pub fn new(ai2_home: PathBuf) -> Options {
        let ui_port = std::env::var("AI2_UI_PORT")
            .ok()
            .and_then(|port| port.parse().ok())
            .unwrap_or(DEFAULT_PORT);
        Options {
            home: ai2_home,
            ui_port,
        }
    }
}

pub fn command() -> clap::Command {
    AppsCommand::augment_subcommands(
        clap::Command::new("apps")
            .about("Local apps: register them with their OpenAPI document, start, stop and read their logs")
            .subcommand_required(true),
    )
}

pub fn run(matches: &ArgMatches, opts: &Options) -> anyhow::Result<ExitCode> {
    let command = AppsCommand::from_arg_matches(matches)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let done = runtime.block_on(execute(command, opts));
    runtime.shutdown_background();
    Ok(match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(problem) => {
            print(&problem);
            ExitCode::from(match problem.status {
                400 | 413 | 415 | 422 => USAGE,
                _ => 1,
            })
        }
    })
}

fn print(value: &impl Serialize) {
    let mut out = std::io::stdout().lock();
    let _ = serde_json::to_writer(&mut out, value)
        .map_err(std::io::Error::from)
        .and_then(|_| {
            out.write_all(b"\n")?;
            out.flush()
        });
}

fn usage(message: impl Into<String>) -> Problem {
    Problem::new(400, "usage", message)
}

async fn execute(command: AppsCommand, opts: &Options) -> Result<()> {
    let store = Store::at_home(&opts.home)?;
    let local = Supervisor::with(store.clone(), Settings::detached());
    match command {
        AppsCommand::Register {
            name,
            description,
            command,
            openapi,
            icon,
            cwd,
            port,
            env,
            restart,
            url,
            no_port_env,
        } => {
            let cwd = match cwd {
                Some(cwd) => cwd,
                None => std::env::current_dir()?,
            };
            let cwd = std::path::absolute(&cwd).unwrap_or(cwd);
            let new = NewApp {
                name: name.unwrap_or_default(),
                description: description.unwrap_or_default(),
                command: command.unwrap_or_default(),
                openapi: openapi.unwrap_or_default(),
                icon: icon.map(|path| IconUpload {
                    path: Some(path.to_string_lossy().into_owned()),
                    ..IconUpload::default()
                }),
                cwd: Some(cwd.to_string_lossy().into_owned()),
                port,
                url,
                env: parse_env(&env)?,
                set_port: Some(!no_port_env),
                restart,
            };
            let (app, _) = store.register(new)?;
            let mut out = json!({});
            if crate::supervisor::listening(app.port).await {
                local.check(&app.id).await?;
            } else {
                out["warning"] = json!(format!(
                    "{} is not running yet, so its OpenAPI document was not checked; it is checked once the app runs (ai2 apps start {}).",
                    app.name, app.id
                ));
            }
            out["app"] = json!(local.show(&app.id).await?);
            print(&out);
        }
        AppsCommand::List { openapi } => {
            print(&json!({ "apps": local.list(openapi).await? }));
        }
        AppsCommand::Show { id } => {
            local.adopt().await?;
            print(&json!({ "app": local.show(&id).await? }));
        }
        AppsCommand::Check { id } => {
            print(&json!({ "api": local.check(&id).await? }));
        }
        AppsCommand::Api { id } => {
            let app = store.app(&id)?;
            match store.document(&id) {
                Some(document) => print(&document),
                None => {
                    return Err(Problem::new(
                        404,
                        "openapi_unchecked",
                        format!(
                            "{}'s OpenAPI document has not been checked yet; it is at {} (ai2 apps check {id}).",
                            app.name,
                            crate::openapi::location(&app)
                        ),
                    ))
                }
            }
        }
        AppsCommand::Start { id } => match ui(opts) {
            Some(ui) => print(&ui.call("POST", &format!("/api/apps/{id}/start"))?),
            None => {
                let app = store.app(&id)?;
                local.adopt().await?;
                let shown = local.start(&id).await?;
                print(&started(shown, app.restart));
            }
        },
        AppsCommand::Stop { id } => match ui(opts) {
            Some(ui) => print(&ui.call("POST", &format!("/api/apps/{id}/stop"))?),
            None => {
                local.adopt().await?;
                print(&json!({ "app": local.stop(&id).await? }));
            }
        },
        AppsCommand::Restart { id } => match ui(opts) {
            Some(ui) => {
                let shown = ui.call("GET", &format!("/api/apps/{id}"))?;
                if shown["app"]["started_here"] == json!(true) {
                    ui.call("POST", &format!("/api/apps/{id}/stop"))?;
                }
                print(&ui.call("POST", &format!("/api/apps/{id}/start"))?);
            }
            None => {
                let app = store.app(&id)?;
                local.adopt().await?;
                let shown = local.restart(&id).await?;
                print(&started(shown, app.restart));
            }
        },
        AppsCommand::Status { id } => {
            let app = match ui(opts) {
                Some(ui) => serde_json::from_value::<Shown>(
                    ui.call("GET", &format!("/api/apps/{id}"))?["app"].clone(),
                )?,
                None => {
                    local.adopt().await?;
                    local.show(&id).await?
                }
            };
            print(&status(&app));
        }
        AppsCommand::Logs { id, lines, follow } => {
            let tail = local.logs(&id, lines)?;
            if !follow {
                print(&json!({ "lines": tail }));
                return Ok(());
            }
            for line in &tail {
                print(line);
            }
            match ui(opts) {
                Some(ui) => ui.follow(&id, |line: LogLine| print(&line))?,
                None => follow_file(&store, &id).await,
            }
        }
        AppsCommand::Remove { id } => {
            local.remove(&id)?;
            print(&json!({ "removed": id }));
        }
    }
    Ok(())
}

fn ui(opts: &Options) -> Option<Ui> {
    Ui::find(&opts.home, opts.ui_port)
}

fn started(shown: Shown, restart: Restart) -> Value {
    let mut out = json!({ "app": shown });
    if restart != Restart::Never {
        out["warning"] = json!(
            "ai2 ui is not running, so the app was started on its own: it keeps running, and its restart policy applies once ai2 ui starts it."
        );
    }
    out
}

fn status(app: &Shown) -> Value {
    json!({
        "id": app.app.id,
        "port": app.app.port,
        "running": app.status.running,
        "started_here": app.status.started_here,
        "pid": app.status.pid,
        "since": app.status.since,
        "restarts": app.status.restarts,
        "last_exit": app.status.last_exit,
        "crashed": app.status.crashed,
    })
}

fn parse_env(pairs: &[String]) -> Result<BTreeMap<String, String>> {
    pairs
        .iter()
        .map(|pair| match pair.split_once('=') {
            Some((key, value)) if !key.is_empty() => Ok((key.to_string(), value.to_string())),
            _ => Err(usage(format!("--env takes K=V, not \"{pair}\""))),
        })
        .collect()
}

async fn follow_file(store: &Store, id: &str) {
    let mut follow = Follow::from_end(&store.log_path(id));
    loop {
        for line in follow.read() {
            print(&LogLine {
                t: None,
                stream: crate::model::Stream::Log,
                line,
            });
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
}
