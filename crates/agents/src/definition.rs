use std::path::PathBuf;

use harness::Registry;
use serde::{Deserialize, Serialize};

use crate::enhance::{js_len, Personality, PERSONALITY_MAX};
use crate::error::{Problem, Result};

pub const NAME_MAX: usize = 24;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema, utoipa::ToSchema))]
pub struct Agent {
    pub name: String,
    pub harness: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub personality: Option<Personality>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub created: String,
    pub updated: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema, utoipa::ToSchema))]
pub struct NewAgent {
    pub name: String,
    pub harness: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub personality: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
}

pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= NAME_MAX
        && !name.starts_with('-')
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn clean_name(name: &str) -> Result<String> {
    let clean = name.trim();
    if clean.is_empty() {
        return Err(Problem::invalid("name_required", "Give it a name."));
    }
    if js_len(clean) > NAME_MAX {
        return Err(Problem::invalid(
            "invalid_name",
            format!("A name is {NAME_MAX} characters at most."),
        ));
    }
    if !valid_name(clean) {
        return Err(Problem::invalid(
            "invalid_name",
            "A name is lowercase letters, digits and dashes, like code-review.",
        ));
    }
    Ok(clean.to_string())
}

fn clean_personality(personality: Option<&str>) -> Result<String> {
    let clean = personality.unwrap_or("").trim();
    if js_len(clean) > PERSONALITY_MAX {
        return Err(Problem::invalid(
            "personality_too_long",
            "The personality is 20,000 characters at most.",
        ));
    }
    Ok(clean.to_string())
}

pub fn clean_harness(
    registry: &Registry,
    harness: &str,
    model: Option<&str>,
) -> Result<(String, Option<String>)> {
    let harness = harness.trim();
    let setup = registry.agent(harness).ok_or_else(|| {
        Problem::invalid(
            "unknown_harness",
            format!("There is no harness called {harness}."),
        )
    })?;
    let chosen = model.unwrap_or("").trim();
    let listed = setup.models();
    if !chosen.is_empty() && !listed.is_empty() && !listed.iter().any(|known| known.id == chosen) {
        return Err(Problem::invalid(
            "unknown_model",
            format!("{} has no model called {chosen}.", setup.label),
        ));
    }
    Ok((
        setup.id.clone(),
        (!chosen.is_empty()).then(|| chosen.to_string()),
    ))
}

pub fn folder(path: &str) -> Result<PathBuf> {
    let given = path.trim();
    if given.is_empty() {
        return Err(Problem::invalid(
            "folder_required",
            "Say which folder the agent works in.",
        ));
    }
    PathBuf::from(given)
        .canonicalize()
        .ok()
        .filter(|found| found.is_dir())
        .ok_or_else(|| Problem::invalid("not_a_folder", format!("{given} is not a folder.")))
}

pub fn define(
    registry: &Registry,
    new: &NewAgent,
    now: &str,
    before: Option<&Agent>,
) -> Result<Agent> {
    let name = clean_name(&new.name)?;
    let (harness, model) = clean_harness(registry, &new.harness, new.model.as_deref())?;
    let personality = clean_personality(new.personality.as_deref())?;
    let folder = new
        .folder
        .as_deref()
        .filter(|path| !path.trim().is_empty())
        .map(folder)
        .transpose()?;
    Ok(Agent {
        name,
        harness,
        model,
        personality: (!personality.is_empty()).then(|| Personality::written(&personality)),
        folder: folder.map(|path| path.display().to_string()),
        description: None,
        created: before.map_or_else(|| now.to_string(), |agent| agent.created.clone()),
        updated: now.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: &str = "2026-11-03T10:00:00Z";

    fn asked(name: &str, harness: &str, model: Option<&str>) -> NewAgent {
        NewAgent {
            name: name.into(),
            harness: harness.into(),
            model: model.map(String::from),
            ..NewAgent::default()
        }
    }

    #[test]
    fn an_agent_is_refused_with_a_reason_for_each_wrong_field() {
        let registry = Registry::public();
        let refused = |new: NewAgent| define(&registry, &new, NOW, None).unwrap_err();
        assert_eq!(
            refused(asked(" ", "claude", None)).message,
            "Give it a name."
        );
        assert_eq!(
            refused(asked("Scout One", "claude", None)).error,
            "invalid_name"
        );
        assert_eq!(
            refused(asked(&"x".repeat(25), "claude", None)).message,
            "A name is 24 characters at most."
        );
        assert_eq!(
            refused(asked("scout", "nope", None)).message,
            "There is no harness called nope."
        );
        assert_eq!(refused(asked("scout", "gh", None)).error, "unknown_harness");
        assert_eq!(
            refused(asked("scout", "claude", Some("gpt-5"))).message,
            "Claude Code has no model called gpt-5."
        );
        assert_eq!(
            refused(NewAgent {
                personality: Some("x".repeat(20_001)),
                ..asked("scout", "claude", None)
            })
            .message,
            "The personality is 20,000 characters at most."
        );
        let missing = refused(NewAgent {
            folder: Some("/surely/not/a/folder".into()),
            ..asked("scout", "claude", None)
        });
        assert_eq!(
            [missing.error.as_str(), missing.message.as_str()],
            ["not_a_folder", "/surely/not/a/folder is not a folder."]
        );
    }

    #[test]
    fn an_agent_keeps_its_harness_model_personality_and_real_folder() {
        let registry = Registry::public();
        let dir = tempfile::tempdir().unwrap();
        let made = define(
            &registry,
            &NewAgent {
                personality: Some(" Be brief. ".into()),
                folder: Some(dir.path().display().to_string()),
                ..asked(" scout-1 ", "claude", Some("sonnet"))
            },
            NOW,
            None,
        )
        .unwrap();
        let real = dir.path().canonicalize().unwrap();
        assert_eq!(
            serde_json::to_value(&made).unwrap(),
            serde_json::json!({ "name": "scout-1", "harness": "claude", "model": "sonnet", "personality": { "content": "Be brief." }, "folder": real.display().to_string(), "created": NOW, "updated": NOW })
        );
        let plain = define(
            &registry,
            &asked("scout", "pi", Some(" ")),
            NOW,
            Some(&made),
        )
        .unwrap();
        assert_eq!(
            [
                plain.model,
                plain.personality.map(|p| p.content),
                plain.folder
            ],
            [None, None, None]
        );
        assert_eq!(plain.created, made.created);
        let free = define(&registry, &asked("scout", "pi", Some("a/b")), NOW, None).unwrap();
        assert_eq!(free.model.as_deref(), Some("a/b"));
    }
}
