#[cfg(feature = "agents")]
pub mod agents;
#[cfg(feature = "apps")]
pub mod apps;
pub mod clients;
mod failure;
#[cfg(feature = "harness")]
pub mod harness;
pub mod openapi;
mod progress;
pub mod reviews;
mod server;
#[cfg(feature = "setup")]
pub mod setup;
#[cfg(test)]
mod tests;
mod transport;
#[cfg(feature = "vault")]
pub mod vault;

pub use failure::Failure;
pub use server::{tools, Pieces, Server};
pub use transport::{command, http_service, run, serve, stdio, HttpService};

pub fn features() -> Vec<&'static str> {
    vec![
        "reviews",
        #[cfg(feature = "harness")]
        "harness",
        #[cfg(feature = "vault")]
        "vault",
        #[cfg(feature = "setup")]
        "setup",
        #[cfg(feature = "apps")]
        "apps",
        #[cfg(feature = "agents")]
        "agents",
    ]
}
