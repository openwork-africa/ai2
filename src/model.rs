use anyhow::{anyhow, bail, Context, Result};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const MAX_TOKENS: u64 = 8192;
const TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Deserialize)]
struct Providers {
    providers: BTreeMap<String, Provider>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Provider {
    api: String,
    base_url: String,
    api_key: Option<String>,
    #[serde(default)]
    headers: BTreeMap<String, String>,
    #[serde(default)]
    auth_header: bool,
    #[serde(default)]
    models: Vec<ModelEntry>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ModelEntry {
    id: String,
    #[serde(default)]
    default: bool,
    max_tokens: Option<u64>,
}

pub struct Model {
    provider_name: String,
    provider: Provider,
    id: String,
    max_tokens: u64,
    agent: ureq::Agent,
}

impl Model {
    pub fn load(file: &Path, name: Option<&str>) -> Result<Model> {
        let text =
            std::fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
        let all: Providers = serde_json::from_str(&text)
            .with_context(|| format!("{} is not a providers file", file.display()))?;
        let (provider_name, provider, id) = match name.and_then(|name| name.split_once('/')) {
            Some((provider, id)) => (
                provider.to_string(),
                all.providers
                    .get(provider)
                    .ok_or_else(|| anyhow!("no provider {provider} in {}", file.display()))?,
                id.to_string(),
            ),
            None => {
                let listed: Vec<(&String, &Provider, &ModelEntry)> = all
                    .providers
                    .iter()
                    .flat_map(|(n, p)| p.models.iter().map(move |m| (n, p, m)))
                    .collect();
                let (provider_name, provider, model) = listed
                    .iter()
                    .find(|(_, _, m)| m.default)
                    .or(listed.first())
                    .copied()
                    .ok_or_else(|| anyhow!("{} lists no model", file.display()))?;
                (provider_name.clone(), provider, model.id.clone())
            }
        };
        let max_tokens = provider
            .models
            .iter()
            .find(|m| m.id == id)
            .and_then(|m| m.max_tokens)
            .unwrap_or(MAX_TOKENS);
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .http_status_as_error(false)
            .build()
            .new_agent();
        Ok(Model {
            provider_name,
            provider: provider.clone(),
            id,
            max_tokens,
            agent,
        })
    }

    pub fn name(&self) -> String {
        format!("{}/{}", self.provider_name, self.id)
    }

    pub fn ask<T: DeserializeOwned>(&self, system: &str, input: &str) -> Result<T> {
        let answer = self.complete(system, input)?;
        match parse(&answer) {
            Ok(value) => Ok(value),
            Err(_) => parse(&self.complete(
                system,
                &format!("{input}\n\nAnswer with the JSON object alone, nothing else."),
            )?),
        }
    }

    fn complete(&self, system: &str, input: &str) -> Result<String> {
        let p = &self.provider;
        let key = p.api_key.as_deref().map(resolve).transpose()?;
        let base = p.base_url.trim_end_matches('/');
        let (url, body) = match p.api.as_str() {
            "openai-completions" => (
                format!("{base}/chat/completions"),
                json!({ "model": self.id, "max_tokens": self.max_tokens, "messages": [{ "role": "system", "content": system }, { "role": "user", "content": input }] }),
            ),
            "anthropic-messages" => (
                format!("{base}/v1/messages"),
                json!({ "model": self.id, "max_tokens": self.max_tokens, "system": system, "messages": [{ "role": "user", "content": input }] }),
            ),
            other => bail!(
                "ai2 does not speak the {other} API yet (openai-completions or anthropic-messages)"
            ),
        };
        let mut request = self.agent.post(&url);
        if let Some(key) = &key {
            request = if p.api == "anthropic-messages" && !p.auth_header {
                request
                    .header("x-api-key", key)
                    .header("anthropic-version", "2023-06-01")
            } else {
                request.header("authorization", &format!("Bearer {key}"))
            };
        }
        for (name, value) in &p.headers {
            request = request.header(name, &resolve(value)?);
        }
        let mut response = request
            .send_json(&body)
            .with_context(|| format!("calling {url}"))?;
        let status = response.status();
        let answer: Value = response.body_mut().read_json().unwrap_or(Value::Null);
        if !status.is_success() {
            bail!(
                "{url} answered {status}: {}",
                answer.get("error").unwrap_or(&answer)
            );
        }
        let text = match p.api.as_str() {
            "openai-completions" => answer["choices"][0]["message"]["content"]
                .as_str()
                .map(str::to_string),
            _ => answer["content"].as_array().map(|parts| {
                parts
                    .iter()
                    .filter_map(|part| part["text"].as_str())
                    .collect()
            }),
        };
        text.ok_or_else(|| anyhow!("{url} gave no text"))
    }
}

fn resolve(value: &str) -> Result<String> {
    if let Some(name) = value.strip_prefix('$') {
        return std::env::var(name).with_context(|| format!("${name} is not set"));
    }
    if let Some(command) = value.strip_prefix('!') {
        let out = Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .with_context(|| format!("running {command}"))?;
        if !out.status.success() {
            bail!("{command} failed");
        }
        return Ok(String::from_utf8_lossy(&out.stdout).trim().to_string());
    }
    Ok(value.to_string())
}

fn parse<T: DeserializeOwned>(answer: &str) -> Result<T> {
    let text = answer
        .rsplit_once("</think>")
        .map_or(answer, |(_, after)| after);
    let (start, end) = (text.find('{'), text.rfind('}'));
    match (start, end) {
        (Some(start), Some(end)) if start < end => serde_json::from_str(&text[start..=end])
            .context("the model's JSON was not in the expected shape"),
        _ => bail!("the model gave no JSON object"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct Answer {
        score: f64,
    }

    #[test]
    fn reads_the_object_after_thinking_and_prose() {
        let answer: Answer =
            parse("<think>{\"score\": 1}</think> Here it is: {\"score\": 80} done").unwrap();
        assert_eq!(answer.score, 80.0);
        assert!(parse::<Answer>("no json here").is_err());
    }

    #[test]
    fn resolves_keys_without_naming_them() {
        assert_eq!(resolve("!printf ' k '").unwrap(), "k");
        assert_eq!(resolve("plain").unwrap(), "plain");
        assert!(resolve("$AI2_SURELY_UNSET")
            .unwrap_err()
            .to_string()
            .contains("AI2_SURELY_UNSET"));
    }
}
