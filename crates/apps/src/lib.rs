mod error;
pub mod icon;
mod model;
pub mod openapi;
pub mod port;
mod store;
mod time;

#[cfg(test)]
mod tests;

pub use error::{Problem, Result};
pub use model::{
    Api, ApiState, App, AppEvent, AppPatch, Exit, Icon, IconUpload, LogLine, NewApp, Restart,
    Shown, Status, Stream,
};
pub use store::{href, slug, Running, Store, LOG_LIMIT};

impl NewApp {
    pub fn parse(value: serde_json::Value) -> Result<NewApp> {
        const REQUIRED: [(&str, &str, &str); 4] = [
            ("name", "name_required", "Send the app's name."),
            (
                "description",
                "description_required",
                "Send a description: one line on what the app is for.",
            ),
            (
                "command",
                "command_required",
                "Send the command that runs the app, e.g. deno task start.",
            ),
            (
                "openapi",
                "openapi_required",
                "Send the path of the app's OpenAPI document, e.g. /openapi.json, or a file.",
            ),
        ];
        if !value.is_object() {
            return Err(Problem::new(
                400,
                "invalid",
                "Send the app as a JSON object.",
            ));
        }
        for (field, error, message) in REQUIRED {
            if value[field].as_str().is_none_or(|s| s.trim().is_empty()) {
                return Err(Problem::new(422, error, message));
            }
        }
        serde_json::from_value(value).map_err(|e| Problem::new(400, "invalid", e.to_string()))
    }
}
