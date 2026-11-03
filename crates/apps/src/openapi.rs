use crate::error::Result;
use crate::model::{Api, ApiState, App};
use crate::store::{href, Store};
use crate::time;
use serde_json::Value;
use std::path::Path;
use std::time::Duration;

pub const DOCUMENT_LIMIT: u64 = 4 * 1024 * 1024;
const METHODS: [&str; 8] = [
    "get", "put", "post", "delete", "options", "head", "patch", "trace",
];

pub enum Source {
    File(String),
    Url(String),
}

pub fn source(app: &App) -> Source {
    let openapi = app.openapi.trim();
    if openapi.starts_with("http://") || openapi.starts_with("https://") {
        return Source::Url(openapi.to_string());
    }
    let path = Path::new(openapi);
    if path.is_absolute() && path.is_file() {
        return Source::File(openapi.to_string());
    }
    let base = app.url.trim_end_matches('/');
    Source::Url(format!("{base}/{}", openapi.trim_start_matches('/')))
}

pub fn location(app: &App) -> String {
    match source(app) {
        Source::File(path) => path,
        Source::Url(url) => url,
    }
}

pub fn check(store: &Store, app: &App) -> Result<Api> {
    let mut api = Api {
        openapi: app.openapi.clone(),
        href: href(&app.id),
        checked: Some(time::stamp()),
        ..Api::default()
    };
    let document = match fetch(&source(app)) {
        Err(error) => {
            api.state = ApiState::Unreachable;
            api.error = Some(error);
            None
        }
        Ok(text) => match validate(&text) {
            Err(error) => {
                api.state = ApiState::Invalid;
                api.error = Some(error);
                None
            }
            Ok((document, title, version, operations)) => {
                api.state = ApiState::Valid;
                api.title = title;
                api.version = version;
                api.operations = Some(operations);
                Some(document)
            }
        },
    };
    store.save_api(&app.id, &api, document.as_ref())?;
    Ok(api)
}

fn fetch(source: &Source) -> std::result::Result<String, String> {
    match source {
        Source::File(path) => {
            let size = std::fs::metadata(path)
                .map_err(|e| format!("{path}: {e}"))?
                .len();
            if size > DOCUMENT_LIMIT {
                return Err(format!("{path} is larger than 4 MiB"));
            }
            std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))
        }
        Source::Url(url) => {
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .timeout_global(Some(Duration::from_secs(2)))
                .http_status_as_error(false)
                .max_redirects(0)
                .build()
                .into();
            let mut response = agent.get(url).call().map_err(|e| format!("{url}: {e}"))?;
            let status = response.status().as_u16();
            if !(200..300).contains(&status) {
                return Err(format!("{url} answered HTTP {status}"));
            }
            response
                .body_mut()
                .with_config()
                .limit(DOCUMENT_LIMIT)
                .read_to_string()
                .map_err(|e| format!("{url}: {e}"))
        }
    }
}

type Checked = (Value, Option<String>, Option<String>, u32);

pub fn validate(text: &str) -> std::result::Result<Checked, String> {
    let document: Value = serde_json::from_str(text).map_err(|error| {
        let start = text.trim_start();
        if start.starts_with("openapi:")
            || start.starts_with("---")
            || start.starts_with("swagger:")
        {
            "the document is YAML; serve it as JSON".to_string()
        } else {
            format!("the document is not JSON: {error}")
        }
    })?;
    let version = document["openapi"].as_str().unwrap_or_default();
    if !version.starts_with("3.") {
        let found = match &document["openapi"] {
            Value::Null => match document["swagger"].as_str() {
                Some(swagger) => format!("swagger {swagger}"),
                None => "no openapi field".to_string(),
            },
            other => format!("openapi {other}"),
        };
        return Err(format!("expected OpenAPI 3.x, found {found}"));
    }
    let Some(paths) = document["paths"].as_object() else {
        return Err("the document has no paths object".to_string());
    };
    let operations = paths
        .values()
        .filter_map(Value::as_object)
        .map(|item| METHODS.iter().filter(|m| item.contains_key(**m)).count() as u32)
        .sum();
    let title = document["info"]["title"].as_str().map(str::to_string);
    let info_version = document["info"]["version"].as_str().map(str::to_string);
    Ok((document, title, info_version, operations))
}
