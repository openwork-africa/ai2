use crate::harness::Finding;
use crate::model::Model;
use crate::scan::Spec;
use crate::steps::{Exploit, Factor, Rating, Scale};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Serialize, Deserialize, Clone)]
pub struct Item {
    pub index: usize,
    pub text: String,
    pub score: f64,
    pub factors: Vec<Factor>,
}

pub struct Context {
    pub folder: PathBuf,
    pub out: PathBuf,
    pub objective: String,
    pub changed: Vec<String>,
    pub days: u64,
    pub spec: Spec,
    pub model: Option<Model>,
    pub args: clap::ArgMatches,
    pub deadline: Option<Instant>,
}

impl Context {
    pub fn arg<T: Clone + Send + Sync + 'static>(&self, step: &str, name: &str) -> Option<&T> {
        self.args.get_one::<T>(&format!("{step}.{name}"))
    }
    pub fn remaining(&self) -> Option<Duration> {
        self.deadline
            .map(|d| d.saturating_duration_since(Instant::now()))
    }
}

pub type Run = Context;

#[derive(Default)]
pub struct Found {
    pub exploits: Vec<Exploit>,
    pub findings: Vec<Finding>,
}

#[derive(Default)]
pub struct Rated {
    pub rating: Option<Rating>,
    pub exploits: Vec<Exploit>,
    pub findings: Vec<Finding>,
    pub items: Vec<Item>,
    pub harness: Option<&'static str>,
    pub notes: Vec<String>,
}

pub trait Step {
    fn id(&self) -> &'static str;
    fn scale(&self) -> &'static Scale;
    fn args(&self) -> Vec<clap::Arg> {
        Vec::new()
    }
    fn after(&self) -> &[&'static str] {
        &[]
    }
    fn provision(&mut self, _ctx: &Context) -> Result<()> {
        Ok(())
    }
    fn rate(&self, ctx: &Context, found: &Found) -> Result<Rated>;
    fn cleanup(&mut self) {}
}
