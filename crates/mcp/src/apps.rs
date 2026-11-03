use crate::{Failure, Server};
pub use ::apps::{LogLine, NewApp, Shown};
use ::apps::{Problem, Settings, Store, Supervisor, Ui};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router, Json};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

pub type Answer<'a, T> = Pin<Box<dyn Future<Output = Result<T, Failure>> + Send + 'a>>;

pub trait Apps: Send + Sync {
    fn register(&self, app: NewApp) -> Answer<'_, Registered>;
    fn list(&self, include_openapi: bool) -> Answer<'_, Vec<Shown>>;
    fn start<'a>(&'a self, id: &'a str) -> Answer<'a, Shown>;
    fn stop<'a>(&'a self, id: &'a str) -> Answer<'a, Shown>;
    fn status<'a>(&'a self, id: &'a str) -> Answer<'a, Shown>;
    fn logs<'a>(&'a self, id: &'a str, tail: usize) -> Answer<'a, Vec<LogLine>>;
}

impl From<Problem> for Failure {
    fn from(problem: Problem) -> Self {
        Failure::new(&problem.error, problem.message)
    }
}

pub struct Unavailable(pub Failure);

impl Unavailable {
    fn answer<'a, T: Send + 'a>(&self) -> Answer<'a, T> {
        let failure = self.0.clone();
        Box::pin(async move { Err(failure) })
    }
}

impl Apps for Unavailable {
    fn register(&self, _app: NewApp) -> Answer<'_, Registered> {
        self.answer()
    }
    fn list(&self, _include_openapi: bool) -> Answer<'_, Vec<Shown>> {
        self.answer()
    }
    fn start<'a>(&'a self, _id: &'a str) -> Answer<'a, Shown> {
        self.answer()
    }
    fn stop<'a>(&'a self, _id: &'a str) -> Answer<'a, Shown> {
        self.answer()
    }
    fn status<'a>(&'a self, _id: &'a str) -> Answer<'a, Shown> {
        self.answer()
    }
    fn logs<'a>(&'a self, _id: &'a str, _tail: usize) -> Answer<'a, Vec<LogLine>> {
        self.answer()
    }
}

pub struct Local {
    supervisor: Supervisor,
    ui: Option<(PathBuf, u16)>,
}

impl Local {
    pub fn in_process(supervisor: Supervisor) -> Self {
        Self {
            supervisor,
            ui: None,
        }
    }

    pub fn detached(home: &Path) -> Result<Self, Failure> {
        let options = ::apps::Options::new(home.to_path_buf());
        Ok(Self {
            supervisor: Supervisor::with(Store::at_home(home)?, Settings::detached()),
            ui: Some((options.home, options.ui_port)),
        })
    }

    pub fn supervisor(&self) -> &Supervisor {
        &self.supervisor
    }

    async fn ui(&self) -> Option<Ui> {
        let (home, port) = self.ui.clone()?;
        tokio::task::spawn_blocking(move || Ui::find(&home, port))
            .await
            .ok()
            .flatten()
    }

    async fn through_ui(&self, id: &str, action: &str) -> Option<Result<Shown, Failure>> {
        let ui = self.ui().await?;
        let path = format!("/api/apps/{id}/{action}");
        let answer = tokio::task::spawn_blocking(move || ui.call("POST", &path))
            .await
            .map_err(|error| Failure::new("internal", error.to_string()))
            .and_then(|answer| answer.map_err(Failure::from))
            .and_then(|reply| {
                serde_json::from_value::<Shown>(reply["app"].clone())
                    .map_err(|error| Failure::new("ui_error", error.to_string()))
            });
        Some(answer)
    }

    async fn registered(&self, app: NewApp) -> Result<Registered, Failure> {
        if self.ui.is_none() && app.icon.as_ref().is_some_and(|icon| icon.path.is_some()) {
            return Err(Failure::new(
                "icon_path",
                "Send the icon as base64; ai2 ui does not read files from disk for a request.",
            ));
        }
        let store = self.supervisor.store().clone();
        let (app, _) = tokio::task::spawn_blocking(move || store.register(app))
            .await
            .map_err(|error| Failure::new("internal", error.to_string()))??;
        let warning = if ::apps::listening(app.port).await {
            self.supervisor.check(&app.id).await?;
            None
        } else {
            Some(format!(
                "{} is not running yet, so its OpenAPI document was not checked; it is checked once the app runs (apps_start {}).",
                app.name, app.id
            ))
        };
        Ok(Registered {
            app: self.supervisor.show(&app.id).await?,
            warning,
        })
    }

    async fn started(&self, id: &str) -> Result<Shown, Failure> {
        if let Some(answer) = self.through_ui(id, "start").await {
            return answer;
        }
        self.supervisor.adopt().await?;
        Ok(self.supervisor.start(id).await?)
    }

    async fn stopped(&self, id: &str) -> Result<Shown, Failure> {
        if let Some(answer) = self.through_ui(id, "stop").await {
            return answer;
        }
        self.supervisor.adopt().await?;
        Ok(self.supervisor.stop(id).await?)
    }

    async fn shown(&self, id: &str) -> Result<Shown, Failure> {
        self.supervisor.adopt().await?;
        Ok(self.supervisor.show(id).await?)
    }
}

impl Apps for Local {
    fn register(&self, app: NewApp) -> Answer<'_, Registered> {
        Box::pin(self.registered(app))
    }
    fn list(&self, include_openapi: bool) -> Answer<'_, Vec<Shown>> {
        Box::pin(async move { Ok(self.supervisor.list(include_openapi).await?) })
    }
    fn start<'a>(&'a self, id: &'a str) -> Answer<'a, Shown> {
        Box::pin(self.started(id))
    }
    fn stop<'a>(&'a self, id: &'a str) -> Answer<'a, Shown> {
        Box::pin(self.stopped(id))
    }
    fn status<'a>(&'a self, id: &'a str) -> Answer<'a, Shown> {
        Box::pin(self.shown(id))
    }
    fn logs<'a>(&'a self, id: &'a str, tail: usize) -> Answer<'a, Vec<LogLine>> {
        Box::pin(async move { Ok(self.supervisor.logs(id, tail)?) })
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct Registered {
    pub app: Shown,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct AppList {
    pub apps: Vec<Shown>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct AppFound {
    pub app: Shown,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Logs {
    pub lines: Vec<LogLine>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AppId {
    pub id: String,
}

#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct ListAsk {
    #[serde(default)]
    pub include_openapi: bool,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct LogsAsk {
    pub id: String,
    #[serde(default)]
    pub tail: Option<usize>,
}

#[tool_router(router = apps_tools, vis = "pub(crate)")]
impl Server {
    #[tool(
        name = "apps_register",
        description = "Registers a local app, or updates the one with the same name: its command, folder, port and the path of its OpenAPI document, which is required"
    )]
    async fn apps_register(
        &self,
        Parameters(asked): Parameters<NewApp>,
    ) -> Result<Json<Registered>, Failure> {
        if asked.openapi.trim().is_empty() {
            return Err(Failure::new(
                "openapi_required",
                "Send the path of the app's OpenAPI document, e.g. /openapi.json, or a file.",
            ));
        }
        self.pieces.apps.register(asked).await.map(Json)
    }

    #[tool(
        name = "apps_list",
        description = "Every local app: whether it runs, whether ai2 started it, and its API block; with include_openapi, each app's OpenAPI document too, so one call says what runs locally and how to call it"
    )]
    async fn apps_list(
        &self,
        Parameters(asked): Parameters<ListAsk>,
    ) -> Result<Json<AppList>, Failure> {
        let apps = self.pieces.apps.list(asked.include_openapi).await?;
        Ok(Json(AppList { apps }))
    }

    #[tool(
        name = "apps_start",
        description = "Starts a registered app and waits for its port to open; needs ai2 ui running"
    )]
    async fn apps_start(
        &self,
        Parameters(asked): Parameters<AppId>,
    ) -> Result<Json<AppFound>, Failure> {
        let app = self.pieces.apps.start(&asked.id).await?;
        Ok(Json(AppFound { app }))
    }

    #[tool(
        name = "apps_stop",
        description = "Stops an app ai2 started; one it did not start is refused with not_ours"
    )]
    async fn apps_stop(
        &self,
        Parameters(asked): Parameters<AppId>,
    ) -> Result<Json<AppFound>, Failure> {
        let app = self.pieces.apps.stop(&asked.id).await?;
        Ok(Json(AppFound { app }))
    }

    #[tool(
        name = "apps_status",
        description = "Whether an app runs, its pid, since when and its port"
    )]
    async fn apps_status(
        &self,
        Parameters(asked): Parameters<AppId>,
    ) -> Result<Json<AppFound>, Failure> {
        let app = self.pieces.apps.status(&asked.id).await?;
        Ok(Json(AppFound { app }))
    }

    #[tool(
        name = "apps_logs",
        description = "The last lines an app wrote (200 unless tail says)"
    )]
    async fn apps_logs(
        &self,
        Parameters(asked): Parameters<LogsAsk>,
    ) -> Result<Json<Logs>, Failure> {
        let lines = self
            .pieces
            .apps
            .logs(&asked.id, asked.tail.unwrap_or(200))
            .await?;
        Ok(Json(Logs { lines }))
    }
}
