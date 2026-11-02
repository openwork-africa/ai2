use crate::harness::Finding;
use crate::model::Model;
use crate::scan::Spec;
use crate::steps::{Exploit, Factor, Rating, Scale};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
pub struct Item {
    pub index: usize,
    pub text: String,
    pub score: f64,
    pub factors: Vec<Factor>,
}

pub struct Run {
    pub folder: PathBuf,
    pub objective: String,
    pub changed: Vec<String>,
    pub providers: Option<PathBuf>,
    pub model_name: Option<String>,
    pub model: Option<Model>,
    pub harness: String,
    pub days: u64,
    pub spec: Spec,
    pub exploits: Vec<Exploit>,
    pub findings: Vec<Finding>,
    pub args: clap::ArgMatches,
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
    fn scale(&self) -> &'static Scale;
    fn args(&self) -> Vec<clap::Arg> {
        Vec::new()
    }
    fn rate(&self, run: &Run) -> Result<Rated>;
}
