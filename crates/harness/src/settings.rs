use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

pub const AUTO: &str = "auto";
pub const NONE: &str = "none";
pub const MODEL: &str = "model";

pub trait Settings: Serialize + DeserializeOwned + Default {
    const FILE: &'static str;

    fn path(home: &Path) -> PathBuf {
        home.join(Self::FILE)
    }

    fn load(home: &Path) -> Result<Option<Self>> {
        let path = Self::path(home);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error).with_context(|| format!("reading {}", path.display())),
        };
        serde_json::from_str(&text)
            .map(Some)
            .with_context(|| format!("{} is not in the expected shape", path.display()))
    }

    fn save(&self, home: &Path) -> Result<()> {
        write_private(
            &Self::path(home),
            &(serde_json::to_string_pretty(self)? + "\n"),
        )
    }
}

fn auto() -> String {
    AUTO.into()
}

fn model() -> String {
    MODEL.into()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Security {
    #[serde(default = "auto")]
    pub reviewer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl Default for Security {
    fn default() -> Self {
        Self {
            reviewer: auto(),
            provider: None,
            model: None,
        }
    }
}

impl Settings for Security {
    const FILE: &'static str = "security.json";
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultUse {
    #[serde(default = "model")]
    pub vetter: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl Default for VaultUse {
    fn default() -> Self {
        Self {
            vetter: model(),
            provider: None,
            model: None,
        }
    }
}

impl Settings for VaultUse {
    const FILE: &'static str = "vault.json";
}

pub fn model_name(provider: Option<&str>, model: Option<&str>) -> Option<String> {
    match (given(provider), given(model)) {
        (Some(provider), Some(model)) => Some(format!("{provider}/{model}")),
        _ => None,
    }
}

fn given(text: Option<&str>) -> Option<&str> {
    text.map(str::trim).filter(|text| !text.is_empty())
}

pub fn write_private(path: &Path, contents: &str) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("making {}", dir.display()))?;
    }
    let temp = path.with_extension("new");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&temp)
        .with_context(|| format!("writing {}", temp.display()))?;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    file.write_all(contents.as_bytes())?;
    std::fs::rename(&temp, path).with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_come_back_as_saved_and_an_absent_file_is_none() {
        let home = tempfile::tempdir().unwrap();
        assert_eq!(Security::load(home.path()).unwrap(), None);
        let security = Security {
            reviewer: "claude".into(),
            provider: Some("nebius".into()),
            model: Some("Qwen/Qwen3-Coder".into()),
        };
        security.save(home.path()).unwrap();
        assert_eq!(Security::load(home.path()).unwrap(), Some(security));
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(home.path().join("security.json"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        std::fs::write(home.path().join("vault.json"), "{}").unwrap();
        assert_eq!(
            VaultUse::load(home.path()).unwrap(),
            Some(VaultUse::default())
        );
        std::fs::write(home.path().join("vault.json"), "[").unwrap();
        assert!(VaultUse::load(home.path()).is_err());
    }

    #[test]
    fn a_model_name_needs_both_parts() {
        assert_eq!(model_name(Some("a"), Some("b")).as_deref(), Some("a/b"));
        assert_eq!(model_name(None, Some("b")), None);
        assert_eq!(model_name(Some("a"), Some(" ")), None);
    }
}
