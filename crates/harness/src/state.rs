use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::setup::Setup;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub installed: bool,
    pub signed_in: bool,
}

pub fn is_connected(setup: &Setup, state: State) -> bool {
    state.signed_in || (state.installed && setup.sign_in.is_none())
}

pub struct States {
    file: PathBuf,
}

impl States {
    pub fn at(dir: &Path) -> Self {
        Self {
            file: dir.join("harness.json"),
        }
    }

    fn all(&self) -> BTreeMap<String, State> {
        std::fs::read_to_string(&self.file)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn of(&self, id: &str) -> State {
        self.all().get(id).copied().unwrap_or_default()
    }

    pub fn set(&self, id: &str, state: State) -> Result<()> {
        let mut all = self.all();
        all.insert(id.to_string(), state);
        if let Some(dir) = self.file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&self.file, serde_json::to_string_pretty(&all)? + "\n")?;
        Ok(())
    }
}

pub fn workspace_path(home: &Path, rest: &str) -> String {
    let own = [".local/bin", ".opencode/bin", ".deno/bin"]
        .map(|dir| home.join(dir).display().to_string());
    own.into_iter()
        .chain((!rest.is_empty()).then(|| rest.to_string()))
        .collect::<Vec<_>>()
        .join(":")
}

pub fn on_path(command: &str, path: &str) -> bool {
    path.split(':').filter(|dir| !dir.is_empty()).any(|dir| {
        std::fs::metadata(Path::new(dir).join(command))
            .is_ok_and(|found| found.is_file() && found.permissions().mode() & 0o111 != 0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_set_up_is_remembered_and_connected_reads_it() {
        let dir = tempfile::tempdir().unwrap();
        let states = States::at(dir.path());
        assert_eq!(states.of("claude"), State::default());
        states
            .set(
                "claude",
                State {
                    installed: true,
                    signed_in: false,
                },
            )
            .unwrap();
        assert!(states.of("claude").installed);
        let registry = crate::Registry::public();
        let claude = registry.get("claude").unwrap();
        assert!(!is_connected(claude, states.of("claude")));
        assert!(is_connected(
            claude,
            State {
                installed: true,
                signed_in: true
            }
        ));
        assert!(is_connected(
            registry.get("pi").unwrap(),
            State {
                installed: true,
                signed_in: false
            }
        ));
    }

    #[test]
    fn the_workspace_installs_come_first_on_the_path() {
        assert_eq!(
            workspace_path(Path::new("/w"), "/usr/bin:/bin"),
            "/w/.local/bin:/w/.opencode/bin:/w/.deno/bin:/usr/bin:/bin"
        );
        assert!(on_path("sh", "/nowhere:/bin"));
        assert!(!on_path("surely-not-a-command", "/bin"));
    }
}
