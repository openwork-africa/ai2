use crate::progress;
use crate::{Failure, Server};
use anyhow::{bail, Context, Result};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::service::RequestContext;
use rmcp::{tool, tool_router, Json, RoleServer};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub trait Reviews: Send + Sync {
    fn run(&self, ask: &RunReview, say: &dyn Fn(String)) -> Result<Value>;
    fn dir(&self) -> PathBuf;
}

pub struct Child {
    pub bin: PathBuf,
    pub home: PathBuf,
}

impl Child {
    pub fn new(bin: PathBuf, home: PathBuf) -> Self {
        Self { bin, home }
    }
}

pub fn arguments(ask: &RunReview) -> Vec<String> {
    let mut args = vec![
        "--run-path".to_string(),
        ask.folder.clone(),
        "--json".into(),
    ];
    if let Some(objective) = ask.objective.as_ref().filter(|text| !text.is_empty()) {
        args.extend(["--objective".into(), objective.clone()]);
    }
    if !ask.changed.is_empty() {
        args.extend(["--changed".into(), ask.changed.join(",")]);
    }
    if let Some(budget) = ask.budget {
        args.extend(["--budget".into(), budget.to_string()]);
    }
    args
}

impl Reviews for Child {
    fn run(&self, ask: &RunReview, say: &dyn Fn(String)) -> Result<Value> {
        let mut child = Command::new(&self.bin)
            .args(arguments(ask))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("starting {}", self.bin.display()))?;
        let mut out = child.stdout.take().context("no output")?;
        let printed = std::thread::spawn(move || {
            let mut text = String::new();
            out.read_to_string(&mut text).map(|_| text)
        });
        let mut last = String::new();
        if let Some(errors) = child.stderr.take() {
            for line in BufReader::new(errors).lines().map_while(Result::ok) {
                say(line.clone());
                last = line;
            }
        }
        let status = child.wait()?;
        let text = printed
            .join()
            .map_err(|_| anyhow::anyhow!("reading the review failed"))??;
        if !status.success() {
            bail!("the review ended with an error: {last}");
        }
        serde_json::from_str(&text).context("the review printed no JSON")
    }

    fn dir(&self) -> PathBuf {
        self.home.join("reviews")
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct RunReview {
    pub folder: String,
    #[serde(default)]
    pub objective: Option<String>,
    #[serde(default)]
    pub changed: Vec<String>,
    #[serde(default)]
    pub budget: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ReviewAsk {
    #[serde(default)]
    pub hash: Option<String>,
    #[serde(default)]
    pub folder: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct RatingSummary {
    pub key: String,
    pub name: String,
    pub score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missing: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct ReviewSummary {
    pub hash: Option<String>,
    pub folder: String,
    pub objective: String,
    pub generated_at: String,
    pub ratings: Vec<RatingSummary>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct ReviewList {
    pub reviews: Vec<ReviewSummary>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct ReviewFound {
    pub summary: ReviewSummary,
    pub review: Value,
}

fn text(review: &Value, field: &str) -> String {
    review
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

pub fn summary(hash: Option<String>, review: &Value) -> ReviewSummary {
    let ratings = review
        .get("ratings")
        .and_then(Value::as_array)
        .map(|cards| {
            cards
                .iter()
                .map(|card| RatingSummary {
                    key: text(card, "key"),
                    name: text(card, "name"),
                    score: card
                        .get("rating")
                        .and_then(|rating| rating.get("score"))
                        .and_then(Value::as_f64),
                    missing: card
                        .get("missing")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                })
                .collect()
        })
        .unwrap_or_default();
    ReviewSummary {
        hash,
        folder: text(review, "folder"),
        objective: text(review, "objective"),
        generated_at: text(review, "generated_at"),
        ratings,
    }
}

pub fn stored(dir: &Path) -> Vec<(String, Value)> {
    let mut found: Vec<(String, Value)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let text = std::fs::read_to_string(entry.path().join("review.json")).ok()?;
            let review = serde_json::from_str(&text).ok()?;
            Some((entry.file_name().to_string_lossy().into_owned(), review))
        })
        .collect();
    found.sort_by_key(|(_, review)| std::cmp::Reverse(text(review, "generated_at")));
    found
}

fn same_folder(review: &Value, folder: &str) -> bool {
    let wanted = Path::new(folder)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(folder));
    Path::new(&text(review, "folder")) == wanted
}

pub fn find(dir: &Path, ask: &ReviewAsk) -> Result<ReviewFound, Failure> {
    let found = stored(dir).into_iter().find(|(hash, review)| match ask {
        ReviewAsk {
            hash: Some(wanted), ..
        } => hash == wanted,
        ReviewAsk {
            folder: Some(folder),
            ..
        } => same_folder(review, folder),
        _ => false,
    });
    match (found, ask) {
        (Some((hash, review)), _) => Ok(ReviewFound {
            summary: summary(Some(hash), &review),
            review,
        }),
        (
            None,
            ReviewAsk {
                hash: None,
                folder: None,
            },
        ) => Err(Failure::new(
            "bad_request",
            "send the review's hash or folder",
        )),
        (None, _) => Err(Failure::new(
            "not_found",
            "there is no review for that: run reviews_run on the folder first",
        )),
    }
}

#[tool_router(router = reviews_tools, vis = "pub(crate)")]
impl Server {
    #[tool(
        name = "reviews_run",
        description = "Rates a coding agent's run on a folder: the Modern Security Rating and the Run Risk Factor. Takes what the run was asked to do, the files it changed and a time budget in minutes; progress notifications carry each stage. Returns the review"
    )]
    async fn reviews_run(
        &self,
        Parameters(asked): Parameters<RunReview>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<ReviewFound>, Failure> {
        let reviews = self.pieces.reviews.clone();
        let reviewed = progress::blocking(&context, move |say| {
            let review = reviews.run(&asked, say)?;
            let hash = stored(&reviews.dir())
                .into_iter()
                .find(|(_, stored)| text(stored, "folder") == text(&review, "folder"))
                .map(|(hash, _)| hash);
            Ok::<_, anyhow::Error>(ReviewFound {
                summary: summary(hash, &review),
                review,
            })
        })
        .await??;
        Ok(Json(reviewed))
    }

    #[tool(
        name = "reviews_list",
        description = "Every stored review, newest first: its hash, folder, objective, date and each rating's score"
    )]
    async fn reviews_list(&self) -> Result<Json<ReviewList>, Failure> {
        let dir = self.pieces.reviews.dir();
        let reviews = progress::quiet(move || {
            stored(&dir)
                .into_iter()
                .map(|(hash, review)| summary(Some(hash), &review))
                .collect()
        })
        .await?;
        Ok(Json(ReviewList { reviews }))
    }

    #[tool(
        name = "reviews_get",
        description = "One stored review.json, by its hash or by the folder it reviewed"
    )]
    async fn reviews_get(
        &self,
        Parameters(asked): Parameters<ReviewAsk>,
    ) -> Result<Json<ReviewFound>, Failure> {
        let dir = self.pieces.reviews.dir();
        progress::quiet(move || find(&dir, &asked)).await?.map(Json)
    }
}
