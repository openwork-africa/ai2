use crate::model::LogLine;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, JsonSchema, ToSchema, Clone, PartialEq, Debug)]
pub struct Problem {
    #[serde(skip)]
    pub status: u16,
    pub error: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<LogLine>,
}

pub type Result<T> = std::result::Result<T, Problem>;

impl Problem {
    pub fn new(status: u16, error: &str, message: impl Into<String>) -> Problem {
        Problem {
            status,
            error: error.to_string(),
            message: message.into(),
            lines: Vec::new(),
        }
    }

    pub fn with_lines(mut self, lines: Vec<LogLine>) -> Problem {
        self.lines = lines;
        self
    }

    pub fn internal(message: impl Into<String>) -> Problem {
        Problem::new(500, "internal", message)
    }

    pub fn not_found(id: &str) -> Problem {
        Problem::new(404, "not_found", format!("There is no app \"{id}\""))
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.error, self.message)
    }
}

impl std::error::Error for Problem {}

impl From<std::io::Error> for Problem {
    fn from(error: std::io::Error) -> Problem {
        Problem::internal(error.to_string())
    }
}

impl From<serde_json::Error> for Problem {
    fn from(error: serde_json::Error) -> Problem {
        Problem::internal(error.to_string())
    }
}

impl From<anyhow::Error> for Problem {
    fn from(error: anyhow::Error) -> Problem {
        match error.downcast::<Problem>() {
            Ok(problem) => problem,
            Err(error) => Problem::internal(format!("{error:#}")),
        }
    }
}
