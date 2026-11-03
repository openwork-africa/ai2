use crate::state::{Shared, State};
use crate::{nonce, routes, Options};
use anyhow::{Context, Result};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

pub struct Server {
    listener: TcpListener,
    state: Shared,
}

impl Server {
    pub async fn bind(port: u16, options: &Options) -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", port))
            .await
            .with_context(|| format!("listening on 127.0.0.1:{port}"))?;
        let bound = listener.local_addr()?.port();
        let nonce = if port == 0 {
            nonce::fresh()?
        } else {
            nonce::load(&options.home)?
        };
        let state = Arc::new(State::new(bound, nonce, options)?);
        state.apps.adopt().await?;
        Ok(Self { listener, state })
    }

    pub fn state(&self) -> Shared {
        self.state.clone()
    }

    pub fn addr(&self) -> Result<SocketAddr> {
        Ok(self.listener.local_addr()?)
    }

    pub fn url(&self, tab: Option<&str>) -> String {
        address(self.state.port, &self.state.nonce, tab)
    }

    pub async fn serve(self) -> Result<()> {
        let (router, _) = routes::router(self.state.clone());
        let mut stop = self.state.stop.subscribe();
        axum::serve(self.listener, router)
            .with_graceful_shutdown(async move {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = stop.wait_for(|stopped| *stopped) => {}
                }
            })
            .await
            .context("serving the ui")
    }
}

pub fn address(port: u16, nonce: &str, tab: Option<&str>) -> String {
    let path = match tab {
        None | Some("apps") => "",
        Some(tab) => tab,
    };
    format!("http://127.0.0.1:{port}/{path}?nonce={nonce}")
}
