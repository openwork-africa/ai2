mod admin;
mod bundle;
mod harnesses;
mod nonce;
mod reply;
mod routes;
mod security;
mod server;
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

pub const DEFAULT_PORT: u16 = 7790;
pub const TABS: [&str; 5] = ["apps", "setup", "reviews", "vault", "harnesses"];

#[derive(Clone, Debug)]
pub struct Options {
    pub home: PathBuf,
    pub admin: PathBuf,
}

pub fn command() -> Command {
    Command::new("ui")
        .about("Opens ai2 in the browser: apps, setup, reviews, the vault and harnesses, served on 127.0.0.1")
        .arg(
            Arg::new("port")
                .long("port")
                .value_name("PORT")
                .env("AI2_UI_PORT")
                .default_value("7790")
                .value_parser(value_parser!(u16))
                .help("The port on 127.0.0.1; 0 picks a free one and a fresh nonce"),
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
    let open_browser = !matches.get_flag("no-open");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("starting the ui runtime")?;
    runtime.block_on(async move {
        let server = Server::bind(port, options).await?;
        let url = server.url(tab.as_deref());
        println!(
            "{}",
            serde_json::json!({
                "url": url,
                "admin_socket": options.admin.display().to_string(),
            })
        );
        if open_browser {
            if let Err(error) = open(&url) {
                eprintln!("ai2: could not open the browser, open the url yourself: {error:#}");
            }
        }
        server.serve().await
    })?;
    Ok(true)
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
