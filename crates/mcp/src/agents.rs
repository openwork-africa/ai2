use crate::progress;
use crate::reviews::{stored, Reviews, RunReview};
use crate::{Failure, Server};
pub use ::agents::{AgentFound, AgentList, Finished, NewAgent, RunAsk, RunFound, RunId, RunList};
use ::agents::{Problem, Review, Reviewed};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::service::RequestContext;
use rmcp::{tool, tool_router, Json, RoleServer};
use serde_json::Value;
use std::cell::RefCell;
use std::path::Path;
use std::sync::Arc;

impl From<Problem> for Failure {
    fn from(problem: Problem) -> Self {
        Failure::new(&problem.error, problem.message)
    }
}

pub struct Reviewing(pub Arc<dyn Reviews>);

impl Review for Reviewing {
    fn review(
        &self,
        folder: &Path,
        objective: &str,
        say: &mut dyn FnMut(String),
    ) -> anyhow::Result<Reviewed> {
        let ask = RunReview {
            folder: folder.display().to_string(),
            objective: Some(objective.to_string()),
            changed: Vec::new(),
            budget: None,
        };
        let say = RefCell::new(say);
        let review = self.0.run(&ask, &|line| (*say.borrow_mut())(line))?;
        let folder = review.get("folder").cloned().unwrap_or(Value::Null);
        let hash = stored(&self.0.dir())
            .into_iter()
            .find(|(_, kept)| kept.get("folder") == Some(&folder))
            .map(|(hash, _)| hash)
            .unwrap_or_default();
        Ok(Reviewed { hash, review })
    }
}

#[tool_router(router = agents_tools, vis = "pub(crate)")]
impl Server {
    #[tool(
        name = "agents_list",
        description = "Every kept agent: its name, harness, model, personality and default folder"
    )]
    async fn agents_list(&self) -> Result<Json<AgentList>, Failure> {
        let agents = self.pieces.agents.clone();
        let listed = progress::quiet(move || agents.list()).await??;
        Ok(Json(AgentList { agents: listed }))
    }

    #[tool(
        name = "agents_create",
        description = "Keeps an agent by name, or replaces the one with that name: a harness id, an optional model, a personality (its system prompt; a short one is expanded when a model is set up) and an optional default folder"
    )]
    async fn agents_create(
        &self,
        Parameters(asked): Parameters<NewAgent>,
    ) -> Result<Json<AgentFound>, Failure> {
        let agents = self.pieces.agents.clone();
        let agent = progress::quiet(move || agents.create(&asked, None)).await??;
        Ok(Json(AgentFound { agent }))
    }

    #[tool(
        name = "agent_run",
        description = "Runs a kept agent, or a harness with a model and personality, on a prompt in a folder, then reviews the folder. Progress notifications carry each line it prints. Returns the run record with its review"
    )]
    async fn agent_run(
        &self,
        Parameters(asked): Parameters<RunAsk>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<Finished>, Failure> {
        let agents = self.pieces.agents.clone();
        let finished = progress::blocking(&context, move |say| {
            agents.run(&asked, &mut |event| say(event.said()))
        })
        .await??;
        Ok(Json(finished))
    }

    #[tool(
        name = "runs_list",
        description = "Every agent run, newest first: its harness, folder, prompt, exit and review hash"
    )]
    async fn runs_list(&self) -> Result<Json<RunList>, Failure> {
        let agents = self.pieces.agents.clone();
        let runs = progress::quiet(move || agents.runs().list()).await?;
        Ok(Json(RunList { runs }))
    }

    #[tool(name = "runs_get", description = "One agent run's record, by its id")]
    async fn runs_get(
        &self,
        Parameters(asked): Parameters<RunId>,
    ) -> Result<Json<RunFound>, Failure> {
        let agents = self.pieces.agents.clone();
        let run = progress::quiet(move || agents.runs().get(&asked.id)).await??;
        Ok(Json(RunFound { run }))
    }
}
