mod admin;
pub mod api;
mod approvals;
mod changes;
pub mod cli;
pub mod client;
pub mod crypto;
mod daemon;
mod gateway;
mod guard;
pub mod http;
mod inject;
mod migrate;
mod openapi;
mod policy;
pub mod sandbox;
mod scrub;
mod skill;
mod store;
#[cfg(test)]
mod testing;
pub mod time;
mod ttyagent;
mod vet;
mod with;

pub use cli::{
    command, command_with, default_paths, exit, outcome, Options, Outcome, Serve, ServeHook,
};
pub use daemon::{start, Config, Resolver, Running};
pub const NOT_SAFE: &str =
    "Not safe: an agent outside a container can read anything you can. Docker support is coming soon.";

pub use vet::{KeyInfo, NoVetter, Proposal, Verdict, Vetter, Vetting, PROPOSE_SYSTEM, VET_SYSTEM};
