mod admin;
mod agents;
mod apps;
mod bundle;
mod harnesses;
#[cfg(feature = "mcp")]
mod mcp;
mod mcp_auth;
mod nonce;
mod reply;
pub mod reviews;
mod routes;
mod security;
mod server;
pub mod service;
mod settings;
pub mod setup;
mod state;
mod status;
#[cfg(test)]
mod tests;
mod vault;

use anyhow::{Context, Result};
use clap::{value_parser, Arg, ArgMatches, Command};
use std::path::PathBuf;
use std::process::Stdio;

pub use reply::{error, Reply};
pub use routes::{api, router};
pub use security::NONCE_HEADER;
pub use server::Server;
pub use state::{Shared, State};
pub use status::features;

pub const DEFAULT_PORT: u16 = ::setup::files::UI_PORT;
pub const TABS: [&str; 5] = ["apps", "setup", "reviews", "vault", "harnesses"];

#[derive(Clone, Debug)]
pub struct Options {
    pub home: PathBuf,
    pub admin: PathBuf,
    pub agents: ::agents::Agents,
}

pub fn command() -> Command {
    Command::new("ui")
        .about("Opens ai2 in the browser: apps, setup, reviews, the vault and harnesses, served on 127.0.0.1")
        .arg(
            Arg::new("port")
                .long("port")
                .value_name("PORT")
                .env("AI2_UI_PORT")
                .value_parser(value_parser!(u16))
                .help("The port on 127.0.0.1, 7797 unless set; 0 picks a free one and a fresh nonce"),
        )
        .arg(
            Arg::new("no-open")
                .long("no-open")
                .action(clap::ArgAction::SetTrue)
                .help("Prints the address without opening the browser"),
        )
        .arg(
            Arg::new("tab")
                .long("tab")
                .value_name("PAGE")
                .value_parser(TABS)
                .help("The page to open"),
        )
        .subcommand(Command::new("address").about("Prints the address of the page, nonce included"))
        .subcommand(
            Command::new("rotate-nonce")
                .about("Makes a new nonce; addresses printed before stop working"),
        )
}

pub fn run(matches: &ArgMatches, options: &Options) -> Result<bool> {
    let port = matches
        .get_one::<u16>("port")
        .copied()
        .unwrap_or(DEFAULT_PORT);
    match matches.subcommand() {
        Some(("address", _)) => {
            let nonce = nonce::load(&options.home)?;
            println!(
                "{}",
                serde_json::json!({ "url": server::address(port, &nonce, None) })
            );
            return Ok(true);
        }
        Some(("rotate-nonce", _)) => {
            let nonce = nonce::rotate(&options.home)?;
            println!(
                "{}",
                serde_json::json!({ "url": server::address(port, &nonce, None) })
            );
            return Ok(true);
        }
        _ => {}
    }
    let tab = matches.get_one::<String>("tab").cloned();
    serve_page(
        port,
        options,
        tab.as_deref(),
        !matches.get_flag("no-open"),
        serde_json::Map::new(),
    )
}

pub fn serve_page(
    port: u16,
    options: &Options,
    page: Option<&str>,
    open_browser: bool,
    extra: serde_json::Map<String, serde_json::Value>,
) -> Result<bool> {
    let printed = |url: &str, running: bool| {
        let mut printed = serde_json::json!({
            "url": url,
            "admin_socket": options.admin.display().to_string(),
        });
        if let serde_json::Value::Object(printed) = &mut printed {
            if running {
                printed.insert("running".into(), true.into());
            }
            printed.extend(extra.clone());
        }
        println!("{printed}");
        if open_browser {
            open_or_say(url);
        }
    };
    if let Some(url) = service::reuse(&options.home, port, page) {
        printed(&url, true);
        return Ok(true);
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("starting the ui runtime")?;
    runtime.block_on(async move {
        let server = Server::bind(port, options).await?;
        printed(&server.url(page), false);
        server.serve().await
    })?;
    Ok(true)
}

fn open_or_say(url: &str) {
    if let Err(error) = open(url) {
        eprintln!("ai2: could not open the browser, open the url yourself: {error:#}");
    }
}

fn open(url: &str) -> Result<()> {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let mut child = std::process::Command::new(opener)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("running {opener}"))?;
    std::thread::spawn(move || child.wait());
    Ok(())
}
