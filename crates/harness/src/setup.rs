use std::path::Path;

use serde::Serialize;

/// How a harness takes the agent's own instructions (its personality) for one run, as a real system or developer prompt.
/// A harness with none of these has no per-run way to take them (a rules file would have to live in the person's folder), so
/// the caller puts them at the head of the prompt instead (`takes_personality` says which).
#[derive(Clone, Debug)]
pub enum Personality {
    /// ` <name> '<words>'`: a flag of the harness (`--append-system-prompt`, `--system`).
    Flag(String),
    /// ` -c '<key>="<words as a TOML string>"'`: a config override, as Codex (`developer_instructions`) and Muse
    /// (`run.developer_prompt`) take them. The words are escaped as a TOML basic string (see `line::toml_string`).
    Config(String),
    /// `NAME='<words>'` in the harness's environment (Hermes).
    Env(String),
}

#[derive(Clone, Debug, Default)]
pub struct Headless {
    pub run: String,
    pub flags: String,
    pub env: Option<String>,
    pub personality: Option<Personality>,
    pub no_model: bool,
    pub add_dir: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ModelChoice {
    pub id: String,
    pub label: String,
}

impl ModelChoice {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Agent {
    pub models: Vec<ModelChoice>,
    pub headless: Option<Headless>,
}

#[derive(Clone, Debug, Default)]
pub struct Setup {
    pub id: String,
    pub label: String,
    pub command: String,
    pub install: String,
    pub sign_in: Option<String>,
    pub sign_in_hint: Option<String>,
    pub logo: Option<String>,
    pub use_text: Option<String>,
    pub skills: Option<String>,
    pub agent: Option<Agent>,
}

impl Setup {
    pub fn is_agent(&self) -> bool {
        self.agent.is_some()
    }

    pub fn models(&self) -> &[ModelChoice] {
        self.agent.as_ref().map_or(&[], |agent| &agent.models)
    }

    pub fn headless(&self) -> Option<&Headless> {
        self.agent
            .as_ref()
            .and_then(|agent| agent.headless.as_ref())
    }
}

pub type Installed = Box<dyn Fn(&Setup, &Path) -> anyhow::Result<()>>;

#[derive(Default)]
pub struct Registry {
    setups: Vec<Setup>,
    installed: Vec<Installed>,
}

impl Registry {
    pub fn public() -> Self {
        let mut registry = Self::default();
        for setup in crate::list::public() {
            registry.put(setup, None);
        }
        registry
    }

    pub fn select(&mut self, ids: &[&str]) -> Vec<String> {
        let mut dropped = Vec::new();
        self.setups.retain(|setup| {
            let keep = ids.contains(&setup.id.as_str());
            if !keep {
                dropped.push(setup.id.clone());
            }
            keep
        });
        dropped
    }

    pub fn put(&mut self, setup: Setup, before: Option<&str>) {
        if let Some(at) = self.setups.iter().position(|known| known.id == setup.id) {
            self.setups[at] = setup;
        } else if let Some(at) =
            before.and_then(|id| self.setups.iter().position(|known| known.id == id))
        {
            self.setups.insert(at, setup);
        } else {
            self.setups.push(setup);
        }
    }

    pub fn on_installed(&mut self, hook: Installed) {
        self.installed.push(hook);
    }

    pub fn all(&self) -> Vec<&Setup> {
        let (agents, tools): (Vec<&Setup>, Vec<&Setup>) =
            self.setups.iter().partition(|setup| setup.is_agent());
        agents.into_iter().chain(tools).collect()
    }

    pub fn get(&self, id: &str) -> Option<&Setup> {
        self.setups.iter().find(|setup| setup.id == id)
    }

    pub fn agent(&self, id: &str) -> Option<&Setup> {
        self.get(id).filter(|setup| setup.is_agent())
    }

    pub fn after_install(&self, setup: &Setup, home: &Path) {
        for hook in &self.installed {
            let _ = hook(setup, home);
        }
    }
}
