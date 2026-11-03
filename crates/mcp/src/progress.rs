use crate::Failure;
use rmcp::model::{ProgressNotificationParam, ProgressToken};
use rmcp::service::RequestContext;
use rmcp::{Peer, RoleServer};

struct Progress {
    peer: Peer<RoleServer>,
    token: Option<ProgressToken>,
    count: f64,
}

impl Progress {
    async fn say(&mut self, message: String) {
        let Some(token) = self.token.clone() else {
            return;
        };
        self.count += 1.0;
        let _ = self
            .peer
            .notify_progress(
                ProgressNotificationParam::new(token, self.count).with_message(message),
            )
            .await;
    }
}

pub(crate) async fn blocking<T: Send + 'static>(
    context: &RequestContext<RoleServer>,
    work: impl FnOnce(&dyn Fn(String)) -> T + Send + 'static,
) -> Result<T, Failure> {
    let mut progress = Progress {
        peer: context.peer.clone(),
        token: context.meta.get_progress_token(),
        count: 0.0,
    };
    let (to, mut from) = tokio::sync::mpsc::unbounded_channel();
    let done = tokio::task::spawn_blocking(move || {
        work(&|message| {
            let _ = to.send(message);
        })
    });
    while let Some(message) = from.recv().await {
        progress.say(message).await;
    }
    done.await
        .map_err(|error| Failure::new("failed", error.to_string()))
}

pub(crate) async fn quiet<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, Failure> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| Failure::new("failed", error.to_string()))
}
