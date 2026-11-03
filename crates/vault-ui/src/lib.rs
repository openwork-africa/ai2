mod admin;
mod server;
#[cfg(test)]
mod tests;

use anyhow::{Context, Result};
use clap::{value_parser, Arg, Command};
use server::Server;
use std::path::{Path, PathBuf};
use std::process::Stdio;

pub fn command() -> Command {
    Command::new("ui")
        .about("Opens the vault in the browser: keys, tokens, sandboxes, approvals and the audit log")
        .arg(
            Arg::new("report")
                .long("report")
                .value_name("PATH")
                .value_parser(value_parser!(PathBuf))
                .help("A reviewed folder, its out folder or a report.html: serves the latest report with a live Vault tab"),
        )
}

pub fn run(report: Option<PathBuf>, admin_socket: &Path) -> Result<bool> {
    let server = Server::bind(admin_socket, report.clone())?;
    let url = server.url();
    println!(
        "{}",
        serde_json::json!({
            "url": url,
            "admin_socket": admin_socket.display().to_string(),
            "report": report.map(|r| r.display().to_string()),
        })
    );
    if let Err(error) = open(&url) {
        eprintln!("ai2: could not open the browser, open the url yourself: {error:#}");
    }
    server.serve()?;
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
