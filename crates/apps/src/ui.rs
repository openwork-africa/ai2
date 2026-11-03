use crate::error::{Problem, Result};
use crate::model::LogLine;
use serde_json::Value;
use std::io::BufRead;
use std::path::Path;
use std::time::Duration;

pub const DEFAULT_PORT: u16 = 7797;
const NONCE_HEADER: &str = "X-Ai2-UI-Nonce";

pub struct Ui {
    origin: String,
    nonce: String,
}

fn agent(timeout: Option<Duration>) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(timeout)
        .http_status_as_error(false)
        .max_redirects(0)
        .build()
        .into()
}

impl Ui {
    pub fn find(home: &Path, port: u16) -> Option<Ui> {
        let nonce = std::fs::read_to_string(home.join("ui").join("nonce")).ok()?;
        let ui = Ui {
            origin: format!("http://127.0.0.1:{port}"),
            nonce: nonce.trim().to_string(),
        };
        if ui.nonce.is_empty() || !crate::port::listening(port) {
            return None;
        }
        let (status, body) = ui
            .call_with("GET", "/api/status", Some(Duration::from_secs(2)))
            .ok()?;
        (status == 200 && body.get("features").is_some()).then_some(ui)
    }

    pub fn call(&self, method: &str, path: &str) -> Result<Value> {
        let (status, body) = self.call_with(method, path, Some(Duration::from_secs(60)))?;
        if (200..300).contains(&status) {
            return Ok(body);
        }
        let mut problem = Problem::new(
            status,
            body["error"].as_str().unwrap_or("ui_error"),
            body["message"]
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| format!("ai2 ui answered HTTP {status}")),
        );
        if let Some(lines) = body.get("lines").cloned() {
            problem.lines = serde_json::from_value(lines).unwrap_or_default();
        }
        Err(problem)
    }

    fn call_with(
        &self,
        method: &str,
        path: &str,
        timeout: Option<Duration>,
    ) -> Result<(u16, Value)> {
        let url = format!("{}{path}", self.origin);
        let agent = agent(timeout);
        let sent = match method {
            "GET" => agent.get(&url).header(NONCE_HEADER, &self.nonce).call(),
            _ => agent
                .post(&url)
                .header(NONCE_HEADER, &self.nonce)
                .header("Origin", &self.origin)
                .send_empty(),
        };
        let mut response =
            sent.map_err(|e| Problem::new(502, "ui_unreachable", format!("{url}: {e}")))?;
        let status = response.status().as_u16();
        let text = response.body_mut().read_to_string().unwrap_or_default();
        Ok((status, serde_json::from_str(&text).unwrap_or(Value::Null)))
    }

    pub fn follow(&self, id: &str, mut each: impl FnMut(LogLine)) -> Result<()> {
        let url = format!("{}/api/apps/{id}/logs/events", self.origin);
        let response = agent(None)
            .get(&url)
            .header(NONCE_HEADER, &self.nonce)
            .header("Accept", "text/event-stream")
            .call()
            .map_err(|e| Problem::new(502, "ui_unreachable", format!("{url}: {e}")))?;
        let status = response.status().as_u16();
        if status != 200 {
            return Err(Problem::new(
                status,
                "ui_error",
                format!("{url} answered HTTP {status}"),
            ));
        }
        let reader = std::io::BufReader::new(response.into_body().into_reader());
        for line in reader.lines() {
            let line = line?;
            if let Some(data) = line.strip_prefix("data:") {
                if let Ok(log) = serde_json::from_str::<LogLine>(data.trim()) {
                    each(log);
                }
            }
        }
        Ok(())
    }
}
