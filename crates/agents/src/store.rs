use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::definition::{valid_name, Agent};

pub trait AgentStore: Send + Sync {
    fn list(&self) -> Result<Vec<Agent>>;
    fn get(&self, name: &str) -> Result<Option<Agent>>;
    fn put(&self, agent: &Agent) -> Result<()>;
    fn remove(&self, name: &str) -> Result<bool>;
}

#[derive(Clone, Debug)]
pub struct LocalStore {
    dir: PathBuf,
}

impl LocalStore {
    pub fn new(ai2_home: &Path) -> Self {
        Self::at(&ai2_home.join("agents"))
    }

    pub fn at(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn file(&self, name: &str) -> Option<PathBuf> {
        valid_name(name).then(|| self.dir.join(format!("{name}.json")))
    }
}

impl AgentStore for LocalStore {
    fn list(&self) -> Result<Vec<Agent>> {
        let mut all: Vec<Agent> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
            .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
            .filter_map(|text| serde_json::from_str(&text).ok())
            .collect();
        all.sort_by(|a: &Agent, b: &Agent| a.name.cmp(&b.name));
        Ok(all)
    }

    fn get(&self, name: &str) -> Result<Option<Agent>> {
        let Some(file) = self.file(name) else {
            return Ok(None);
        };
        match std::fs::read_to_string(&file) {
            Ok(text) => {
                Ok(Some(serde_json::from_str(&text).with_context(|| {
                    format!("{} is not an agent", file.display())
                })?))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error).with_context(|| format!("reading {}", file.display())),
        }
    }

    fn put(&self, agent: &Agent) -> Result<()> {
        let Some(file) = self.file(&agent.name) else {
            bail!("{} is not an agent name", agent.name);
        };
        std::fs::create_dir_all(&self.dir)
            .with_context(|| format!("making {}", self.dir.display()))?;
        let fresh = file.with_extension("json.new");
        std::fs::write(&fresh, serde_json::to_string_pretty(agent)? + "\n")?;
        std::fs::rename(&fresh, &file)?;
        Ok(())
    }

    fn remove(&self, name: &str) -> Result<bool> {
        let Some(file) = self.file(name) else {
            return Ok(false);
        };
        match std::fs::remove_file(&file) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error).with_context(|| format!("removing {}", file.display())),
        }
    }
}
