use rmcp::model::{ContentBlock, IntoContents};
use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Failure {
    pub error: String,
    pub message: String,
}

impl Failure {
    pub fn new(error: &str, message: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            message: message.into(),
        }
    }

    pub fn from_reply(status: u16, reply: &Value) -> Self {
        let text = |field: &str| reply.get(field).and_then(Value::as_str);
        Self::new(
            text("error").unwrap_or("failed"),
            text("message")
                .map(str::to_string)
                .unwrap_or_else(|| format!("the call ended with {status}")),
        )
    }
}

impl From<anyhow::Error> for Failure {
    fn from(error: anyhow::Error) -> Self {
        Self::new("failed", format!("{error:#}"))
    }
}

impl IntoContents for Failure {
    fn into_contents(self) -> Vec<ContentBlock> {
        vec![ContentBlock::text(
            serde_json::to_string(&self).unwrap_or(self.message),
        )]
    }
}
