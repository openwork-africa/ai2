use crate::progress;
use crate::{Failure, Server};
use ::harness::cli::{CommandLine, Context, Listed};
use ::harness::job::Event;
use ::harness::Registry;
use anyhow::Result;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::service::RequestContext;
use rmcp::{tool, tool_router, Json, RoleServer};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub struct Ended {
    pub ok: bool,
    pub message: Option<String>,
}

pub trait Harnesses: Send + Sync {
    fn list(&self) -> Result<Vec<Listed>>;
    fn job(&self, id: &str, install: bool, emit: &mut dyn FnMut(&Event)) -> Result<Ended>;
    fn command(&self, ask: &CommandAsk) -> Result<CommandLine>;
}

pub struct Local {
    pub registry: Box<dyn Fn() -> Registry + Send + Sync>,
    pub home: PathBuf,
    pub state: PathBuf,
}

pub fn local(registry: impl Fn() -> Registry + Send + Sync + 'static, state: PathBuf) -> Local {
    Local {
        registry: Box::new(registry),
        home: std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default(),
        state,
    }
}

impl Local {
    fn context(&self) -> Context {
        Context {
            registry: (self.registry)(),
            home: self.home.clone(),
            state: self.state.clone(),
        }
    }
}

impl Harnesses for Local {
    fn list(&self) -> Result<Vec<Listed>> {
        Ok(self.context().list())
    }

    fn job(&self, id: &str, install: bool, emit: &mut dyn FnMut(&Event)) -> Result<Ended> {
        let ended = self.context().job(id, install, std::iter::empty(), emit)?;
        Ok(Ended {
            ok: ended.ok,
            message: ended.message,
        })
    }

    fn command(&self, ask: &CommandAsk) -> Result<CommandLine> {
        self.context().command_line(
            &ask.id,
            ask.prompt.as_deref().unwrap_or_default(),
            ask.folder.clone(),
            ask.model.as_deref(),
            ask.personality.as_deref(),
        )
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct HarnessId {
    pub id: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct CommandAsk {
    pub id: String,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub folder: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub personality: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct HarnessList {
    pub harnesses: Vec<Listed>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct JobReport {
    pub id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub events: Vec<Event>,
}

fn said(event: &Event) -> Option<String> {
    match event {
        Event::Line { text } => Some(text.clone()),
        Event::Link { url } => Some(format!("Open {url}")),
        Event::Code { code } => Some(format!("Code: {code}")),
        Event::Error { error } => Some(error.clone()),
        Event::Done { .. } => None,
    }
}

impl Server {
    async fn harness_job(
        &self,
        id: String,
        install: bool,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<JobReport>, Failure> {
        let harnesses = self.pieces.harness.clone();
        let (ended, events) = progress::blocking(&context, move |say| {
            let mut events = Vec::new();
            let ended = harnesses.job(&id, install, &mut |event| {
                if let Some(text) = said(event) {
                    say(text);
                }
                events.push(event.clone());
            });
            (ended.map(|ended| (id, ended)), events)
        })
        .await?;
        let (id, ended) = ended?;
        Ok(Json(JobReport {
            id,
            ok: ended.ok,
            message: ended.message,
            events,
        }))
    }
}

#[tool_router(router = harness_tools, vis = "pub(crate)")]
impl Server {
    #[tool(
        name = "harness_list",
        description = "Every coding agent and tool ai2 can install: whether it is installed and signed in, its models, and whether it runs headless"
    )]
    async fn harness_list(&self) -> Result<Json<HarnessList>, Failure> {
        let harnesses = self.pieces.harness.clone();
        let listed = progress::quiet(move || harnesses.list()).await??;
        Ok(Json(HarnessList { harnesses: listed }))
    }

    #[tool(
        name = "harness_install",
        description = "Installs a harness by its id and returns what the install printed; progress notifications carry each line"
    )]
    async fn harness_install(
        &self,
        Parameters(asked): Parameters<HarnessId>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<JobReport>, Failure> {
        self.harness_job(asked.id, true, context).await
    }

    #[tool(
        name = "harness_sign_in",
        description = "Starts a harness's sign-in and returns what it printed: the link to open and the code to enter, also sent as progress notifications while it waits. It cannot type into the sign-in; one that asks for a pasted key is done in the CLI or ai2 ui"
    )]
    async fn harness_sign_in(
        &self,
        Parameters(asked): Parameters<HarnessId>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<JobReport>, Failure> {
        self.harness_job(asked.id, false, context).await
    }

    #[tool(
        name = "harness_command",
        description = "The headless command line that runs an agent harness on a prompt in a folder, with an optional model and personality"
    )]
    async fn harness_command(
        &self,
        Parameters(asked): Parameters<CommandAsk>,
    ) -> Result<Json<CommandLine>, Failure> {
        let harnesses = self.pieces.harness.clone();
        let line = progress::quiet(move || harnesses.command(&asked)).await??;
        Ok(Json(line))
    }
}
