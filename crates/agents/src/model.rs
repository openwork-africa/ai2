use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::definition::Agent;
use crate::enhance::Outcome;
use crate::runs::Run;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema, utoipa::ToSchema))]
pub struct AgentList {
    pub agents: Vec<Agent>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema, utoipa::ToSchema))]
pub struct AgentFound {
    pub agent: Agent,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema, utoipa::ToSchema))]
pub struct Removed {
    pub removed: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema, utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum EnhanceStatus {
    Enhanced,
    Skipped,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema, utoipa::ToSchema))]
pub struct Enhanced {
    pub status: EnhanceStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub agent: Agent,
}

impl Enhanced {
    pub fn of(outcome: &Outcome, agent: Agent) -> Self {
        let (status, reason) = match outcome {
            Outcome::Enhanced { .. } => (EnhanceStatus::Enhanced, None),
            Outcome::Skipped => (EnhanceStatus::Skipped, None),
            Outcome::Failed { reason, .. } => (EnhanceStatus::Failed, Some(reason.clone())),
        };
        Self {
            status,
            reason,
            agent,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema, utoipa::ToSchema))]
pub struct RunAsk {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub personality: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    pub prompt: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub no_review: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema, utoipa::ToSchema))]
pub struct RunList {
    pub runs: Vec<Run>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema, utoipa::ToSchema))]
pub struct RunFound {
    pub run: Run,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema, utoipa::ToSchema))]
pub struct RunId {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema, utoipa::ToSchema))]
pub struct Finished {
    pub run: Run,
    #[cfg_attr(feature = "schema", schema(value_type = Option<Object>))]
    pub review: Option<Value>,
}
