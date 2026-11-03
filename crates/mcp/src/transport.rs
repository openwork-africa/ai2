use crate::{Pieces, Server};
use anyhow::Result;
use clap::{Arg, ArgMatches};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::ServiceExt;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};

pub type HttpService = StreamableHttpService<Server, LocalSessionManager>;

pub const DEFAULT_SERVER: &str = "http://127.0.0.1:7790/mcp";

pub fn command() -> clap::Command {
    clap::Command::new("mcp")
        .about("Serves every ai2 piece as MCP tools over stdio: add { \"command\": \"ai2\", \"args\": [\"mcp\"] } to an MCP client")
        .subcommand(
            clap::Command::new("openapi")
                .about("Prints the OpenAPI document made from the MCP tool list")
                .arg(
                    Arg::new("server")
                        .long("server")
                        .default_value(DEFAULT_SERVER)
                        .help("The MCP endpoint the document names as its server"),
                ),
        )
}

pub fn run(matches: &ArgMatches, pieces: Pieces) -> Result<bool> {
    if let Some(("openapi", sub)) = matches.subcommand() {
        let server = sub.get_one::<String>("server").cloned().unwrap_or_default();
        let document = Server::new(pieces).openapi(&server);
        println!("{}", serde_json::to_string_pretty(&document)?);
        return Ok(true);
    }
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(stdio(pieces))?;
    Ok(true)
}

pub async fn serve<R, W>(server: Server, read: R, write: W) -> Result<()>
where
    R: AsyncRead + Send + Unpin + 'static,
    W: AsyncWrite + Send + Unpin + 'static,
{
    server.serve((read, write)).await?.waiting().await?;
    Ok(())
}

pub async fn stdio(pieces: Pieces) -> Result<()> {
    let (read, write) = rmcp::transport::stdio();
    serve(Server::new(pieces), read, write).await
}

pub fn http_service(pieces: Pieces, port: u16) -> HttpService {
    let config = StreamableHttpServerConfig::default()
        .with_allowed_hosts([format!("127.0.0.1:{port}")])
        .with_allowed_origins([format!("http://127.0.0.1:{port}")])
        .with_json_response(true);
    StreamableHttpService::new(
        move || Ok(Server::new(pieces.clone())),
        Arc::new(LocalSessionManager::default()),
        config,
    )
}
