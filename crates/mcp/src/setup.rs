use crate::progress;
use crate::{Failure, Server};
use anyhow::Result;
use rmcp::{tool, tool_router, Json};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub trait Setup: Send + Sync {
    fn check(&self) -> Result<Value>;
}

pub struct Local(pub ::setup::Options);

impl Setup for Local {
    fn check(&self) -> Result<Value> {
        ::setup::setup(&self.0, true).map(|(_, report)| report)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ready,
    Missing,
    Unsupported,
    Unknown,
    Failed,
    Skipped,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct StepReport {
    pub id: String,
    pub status: Status,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub info: Map<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct SetupReport {
    pub ok: bool,
    pub check: bool,
    pub steps: Vec<StepReport>,
}

fn step(id: &str, state: &Value) -> Result<StepReport, Failure> {
    let mut info = state.as_object().cloned().unwrap_or_default();
    let status = info
        .remove("status")
        .map(Status::deserialize)
        .transpose()
        .map_err(|error| Failure::new("bad_report", format!("setup step {id}: {error}")))?
        .unwrap_or(Status::Unknown);
    let warnings = info
        .remove("warnings")
        .and_then(|warnings| Vec::<String>::deserialize(warnings).ok())
        .unwrap_or_default();
    let error = info
        .remove("error")
        .and_then(|error| error.as_str().map(str::to_string));
    Ok(StepReport {
        id: id.into(),
        status,
        warnings,
        error,
        info,
    })
}

pub fn report(value: &Value) -> Result<SetupReport, Failure> {
    let steps = value
        .get("steps")
        .and_then(Value::as_object)
        .map(|steps| {
            steps
                .iter()
                .map(|(id, state)| step(id, state))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(SetupReport {
        ok: value.get("ok").and_then(Value::as_bool).unwrap_or(false),
        check: value.get("check").and_then(Value::as_bool).unwrap_or(true),
        steps,
    })
}

#[tool_router(router = setup_tools, vis = "pub(crate)")]
impl Server {
    #[tool(
        name = "setup_check",
        description = "Reports each setup step (containers, the master key, presence, the daemon) as ready, missing or unsupported on this machine, changing nothing"
    )]
    async fn setup_check(&self) -> Result<Json<SetupReport>, Failure> {
        let setup = self.pieces.setup.clone();
        let checked = progress::quiet(move || setup.check()).await??;
        report(&checked).map(Json)
    }
}
