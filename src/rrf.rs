use crate::model::Model;
use crate::prompts;
use crate::step::{Context, Found, Rated, Step};
use crate::steps::{Rating, Scale, Scored, RRF};
use anyhow::{anyhow, bail, Context as _, Result};
use include_dir::{include_dir, Dir};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

static SKILLS: Dir = include_dir!("$CARGO_MANIFEST_DIR/skills");
const SKILL_NAMES: &[&str] = &[
    "security-audit",
    "dependency-audit",
    "patch-sweep",
    "owasp-audit",
];
const LOGIN_CHECK: Duration = Duration::from_secs(20);
const RUN_LIMIT: Duration = Duration::from_secs(30 * 60);

#[derive(Serialize, Deserialize, Clone)]
pub struct Finding {
    pub title: String,
    #[serde(default)]
    pub technology: String,
    #[serde(default)]
    pub file: String,
    pub severity: String,
    #[serde(default)]
    pub likelihood: String,
    #[serde(default)]
    pub impact: String,
    #[serde(default)]
    pub confidence: String,
    #[serde(default)]
    pub fix: String,
    #[serde(default)]
    pub status: String,
}

#[derive(Deserialize)]
struct Answer {
    findings: Vec<Finding>,
    factors: BTreeMap<String, Scored>,
}

pub struct ReviewTask {
    pub prompt: String,
    pub prompt_file: PathBuf,
    pub skills: PathBuf,
    pub scratch: PathBuf,
    pub folder: PathBuf,
    pub providers: Option<PathBuf>,
    pub model_name: Option<String>,
    pub harness_model: Option<String>,
}
pub type Available = Arc<dyn Fn() -> bool + Send + Sync>;
pub type Commands = Arc<dyn Fn(&ReviewTask) -> Command + Send + Sync>;
#[derive(Clone)]
pub struct Reviewer {
    pub id: String,
    pub label: String,
    pub auto: bool,
    pub available: Available,
    pub command: Commands,
}
impl Reviewer {
    pub fn new(id: &str, label: &str, available: Available, command: Commands) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            auto: true,
            available,
            command,
        }
    }
    pub fn named_only(self) -> Self {
        Self {
            auto: false,
            ..self
        }
    }
}
pub fn muse_reviewer() -> Reviewer {
    Reviewer::new(
        "muse",
        "Muse Code",
        Arc::new(|| on_path("muse") && muse_signed_in()),
        Arc::new(muse_command),
    )
}
pub fn pi_reviewer() -> Reviewer {
    Reviewer::new("pi", "pi", Arc::new(|| on_path("pi")), Arc::new(pi_command))
}
fn muse_command(task: &ReviewTask) -> Command {
    let mut c = Command::new("muse");
    c.args([
        "exec",
        "--json",
        "--yolo",
        "--disable-request-user-input",
        "--prompt-file",
    ])
    .arg(&task.prompt_file);
    c
}
fn pi_command(task: &ReviewTask) -> Command {
    let mut c = Command::new("pi");
    c.args(["-p", "--mode", "json", "--no-session"]);
    for name in SKILL_NAMES {
        c.arg("--skill").arg(task.skills.join(name));
    }
    if let Some(providers) = &task.providers {
        let agent_dir = task.scratch.join("pi");
        let _ = make_private_dir(&agent_dir);
        if let Ok(text) = std::fs::read_to_string(providers) {
            let _ = write_private_bytes(&agent_dir.join("models.json"), text.as_bytes());
        }
        c.env("PI_CODING_AGENT_DIR", agent_dir);
    }
    if let Some(model) = &task.model_name {
        c.args(["--model", model]);
    }
    c.arg(&task.prompt);
    c
}
pub const BY_MODEL: &str = "model";
#[derive(Clone)]
pub enum Chosen {
    Harness(Reviewer),
    Model,
}
impl Chosen {
    pub fn id(&self) -> &str {
        match self {
            Chosen::Harness(reviewer) => &reviewer.id,
            Chosen::Model => BY_MODEL,
        }
    }
}
pub fn pick_from(reviewers: &[Reviewer], choice: &str) -> Result<Option<Chosen>> {
    match choice {
        "none" => Ok(None),
        BY_MODEL => Ok(Some(Chosen::Model)),
        "auto" => Ok(reviewers
            .iter()
            .filter(|r| r.auto)
            .find(|r| (r.available)())
            .cloned()
            .map(Chosen::Harness)),
        id => reviewers
            .iter()
            .find(|r| r.id == id)
            .cloned()
            .map(|r| Some(Chosen::Harness(r)))
            .ok_or_else(|| anyhow!("no reviewer called {id} is registered")),
    }
}
fn make_private_dir(dir: &Path) -> Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    Ok(())
}
pub struct Rrf {
    reviewers: Vec<Reviewer>,
    chosen: Option<Chosen>,
    scratch: Option<tempfile::TempDir>,
    providers: Option<PathBuf>,
    model_name: Option<String>,
    harness_model: Option<String>,
    limit: Duration,
    choice: String,
}
impl Rrf {
    pub fn new(reviewers: Vec<Reviewer>) -> Self {
        Self {
            reviewers,
            chosen: None,
            scratch: None,
            providers: None,
            model_name: None,
            harness_model: None,
            limit: RUN_LIMIT,
            choice: "auto".into(),
        }
    }
}
impl Default for Rrf {
    fn default() -> Self {
        Self::new(default_reviewers())
    }
}
#[cfg(not(feature = "harness"))]
pub fn default_reviewers() -> Vec<Reviewer> {
    vec![muse_reviewer(), pi_reviewer()]
}
#[cfg(feature = "harness")]
pub fn default_reviewers() -> Vec<Reviewer> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    registry_reviewers(
        &::harness::Registry::public(),
        &home,
        &crate::cli::state_dir("ai2"),
    )
}
#[cfg(feature = "harness")]
#[cfg(feature = "muse-only")]
pub const AUTO_ORDER: [&str; 1] = ["muse"];
#[cfg(not(feature = "muse-only"))]
pub const AUTO_ORDER: [&str; 4] = ["muse", "claude", "codex", "pi"];
#[cfg(feature = "harness")]
pub fn registry_reviewers(
    registry: &::harness::Registry,
    home: &Path,
    state: &Path,
) -> Vec<Reviewer> {
    let mut reviewers: Vec<Reviewer> = Vec::new();
    for id in AUTO_ORDER {
        match id {
            "muse" => reviewers.push(muse_reviewer()),
            "pi" => reviewers.push(pi_reviewer()),
            _ => reviewers.extend(
                registry
                    .agent(id)
                    .filter(|setup| setup.headless().is_some())
                    .map(|setup| harness_reviewer(setup, home, state)),
            ),
        }
    }
    for setup in registry.all() {
        let known = reviewers.iter().any(|r| r.id == setup.id);
        if setup.is_agent() && setup.headless().is_some() && !known {
            reviewers.push(harness_reviewer(setup, home, state).named_only());
        }
    }
    reviewers
}
#[cfg(feature = "harness")]
pub fn harness_reviewer(setup: &::harness::Setup, home: &Path, state: &Path) -> Reviewer {
    let ready = setup.clone();
    let (ready_home, ready_state) = (home.to_path_buf(), state.to_path_buf());
    let run = setup.clone();
    let run_home = home.to_path_buf();
    Reviewer::new(
        &setup.id,
        &setup.label,
        Arc::new(move || ready_to_run(&ready, &ready_home, &ready_state)),
        Arc::new(move |task| harness_command(&run, &run_home, task)),
    )
}
#[cfg(feature = "harness")]
pub fn ready_to_run(setup: &::harness::Setup, home: &Path, state: &Path) -> bool {
    use ::harness::state::{is_connected, on_path, workspace_path, State, States};
    let path = workspace_path(home, &std::env::var("PATH").unwrap_or_default());
    let known = States::at(state).of(&setup.id);
    let installed = on_path(&setup.command, &path);
    installed && is_connected(setup, State { installed, ..known })
}
#[cfg(feature = "harness")]
pub fn harness_command(setup: &::harness::Setup, home: &Path, task: &ReviewTask) -> Command {
    use ::harness::state::workspace_path;
    let folder = task.folder.display().to_string();
    let mut line = ::harness::task_line(
        setup,
        &task.prompt,
        &folder,
        task.harness_model.as_deref(),
        None,
    );
    if let Some(flag) = setup.headless().and_then(|h| h.add_dir.as_deref()) {
        line.push_str(&format!(
            " {flag} {}",
            ::harness::sh_quote(&task.scratch.display().to_string())
        ));
    }
    let mut c = Command::new("sh");
    c.arg("-c").arg(line).env(
        "PATH",
        workspace_path(home, &std::env::var("PATH").unwrap_or_default()),
    );
    c
}
impl Step for Rrf {
    fn id(&self) -> &'static str {
        "rating.rrf"
    }
    fn scale(&self) -> &'static Scale {
        &RRF
    }
    fn after(&self) -> &[&'static str] {
        &["rating.msr"]
    }
    fn args(&self) -> Vec<clap::Arg> {
        vec![clap::Arg::new("harness").long("harness").value_name("ID").help("The coding harness for the Run Risk Factor: auto, none, model, or a registered reviewer id [default: the security settings, else auto]")]
    }
    fn provision(&mut self, ctx: &Context) -> Result<()> {
        self.choice = ctx
            .arg::<String>(self.id(), "harness")
            .cloned()
            .or_else(|| ctx.reviewer.clone())
            .unwrap_or_else(|| "auto".into());
        self.chosen = pick_from(&self.reviewers, &self.choice)?;
        self.providers = ctx.providers.clone();
        self.model_name = ctx.model.as_ref().map(Model::name);
        self.harness_model = ctx.reviewer_model.clone();
        if let Some(remaining) = ctx.remaining() {
            self.limit = remaining.min(RUN_LIMIT);
        }
        if matches!(self.chosen, Some(Chosen::Harness(_))) {
            self.scratch = Some(scratch()?);
        }
        Ok(())
    }
    fn cleanup(&mut self) {
        self.scratch.take();
    }
    fn rate(&self, ctx: &Context, found: &Found) -> Result<Rated> {
        let Some(chosen) = &self.chosen else {
            return Ok(Rated {
                notes: vec!["No coding agent is set up and signed in.".into()],
                ..Rated::default()
            });
        };
        let technologies: Vec<&str> = ctx
            .spec
            .languages
            .iter()
            .map(|l| l.name.as_str())
            .chain(ctx.spec.libraries.iter().map(|l| l.name.as_str()))
            .chain([ctx.spec.os.as_str()])
            .collect();
        let task = serde_json::json!({ "objective": ctx.objective, "changed": ctx.changed, "technologies": technologies, "exploits": found.exploits });
        let reviewer = match chosen {
            Chosen::Model => {
                let Some(model) = &ctx.model else {
                    return Ok(Rated {
                        notes: vec!["No model was chosen for the review.".into()],
                        ..Rated::default()
                    });
                };
                crate::cli::progress(&format!("reviewing the code with {}", model.name()));
                let (rating, findings) = model_review(model, ctx, &task)?;
                return Ok(Rated {
                    rating: Some(rating),
                    findings,
                    harness: Some(model.name()),
                    ..Rated::default()
                });
            }
            Chosen::Harness(reviewer) => reviewer,
        };
        crate::cli::progress(&format!("reviewing the code with {}", reviewer.label));
        let work = self
            .scratch
            .as_ref()
            .map(tempfile::TempDir::path)
            .ok_or_else(|| anyhow!("the reviewer scratch was not provisioned"))?;
        let (rating, findings) = rrf(reviewer, self, ctx, &task, work)?;
        Ok(Rated {
            rating: Some(rating),
            findings,
            harness: Some(reviewer.label.clone()),
            ..Rated::default()
        })
    }
}
const MODEL_FILE: usize = 32 * 1024;
const MODEL_FILES: usize = 160 * 1024;
const MODEL_NOTE: &str = "Here you only read: the files come in the message, and nothing can be changed, so every finding is open. The skills named above are not at hand; rely on what you know of them.";
const WRITE_LINE: &str = "Then write one JSON file to {out}, and nothing else there:";

pub fn model_prompt(task: &serde_json::Value) -> Result<String> {
    let text = prompts::RRF.replace(WRITE_LINE, "Then answer with one JSON object alone:");
    Ok(format!(
        "{}\n\n{MODEL_NOTE}",
        text.replace("{skills}", &SKILL_NAMES.join("\n"))
            .replace("{task}", &serde_json::to_string_pretty(task)?)
    ))
}

pub fn model_files(folder: &Path, changed: &[String]) -> String {
    let mut files: Vec<PathBuf> = if changed.is_empty() {
        let mut found = Vec::new();
        walk(folder, &mut found);
        found
    } else {
        changed.iter().map(|name| folder.join(name)).collect()
    };
    files.sort();
    let mut out = String::new();
    for file in files {
        let inside = file
            .canonicalize()
            .is_ok_and(|real| real.starts_with(folder));
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        if !inside || text.len() > MODEL_FILE || out.len() + text.len() > MODEL_FILES {
            continue;
        }
        let name = file
            .strip_prefix(folder)
            .unwrap_or(&file)
            .display()
            .to_string();
        out.push_str(&format!("=== {name} ===\n{text}\n"));
    }
    out
}

fn walk(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || crate::scan::SKIPPED.contains(&name.as_str()) {
            continue;
        }
        let path = entry.path();
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => walk(&path, found),
            Ok(kind) if kind.is_file() => found.push(path),
            _ => {}
        }
    }
}

fn model_review(
    model: &Model,
    ctx: &Context,
    task: &serde_json::Value,
) -> Result<(Rating, Vec<Finding>)> {
    let answer: Answer = model.ask(
        &model_prompt(task)?,
        &model_files(&ctx.folder, &ctx.changed),
    )?;
    Ok((RRF.rate(&answer.factors), answer.findings))
}
fn rrf(
    reviewer: &Reviewer,
    rrf_step: &Rrf,
    setup: &Context,
    task: &serde_json::Value,
    work: &Path,
) -> Result<(Rating, Vec<Finding>)> {
    let skills = work.join(".claude").join("skills");
    // A loose file at the top (a Finder .DS_Store) is written before any folder makes the base.
    std::fs::create_dir_all(&skills).context("writing the skills")?;
    SKILLS.extract(&skills).context("writing the skills")?;
    let out = work.join("findings.json");
    let read: Vec<String> = SKILL_NAMES
        .iter()
        .map(|name| skills.join(name).join("SKILL.md").display().to_string())
        .collect();
    let prompt_text = prompt(&read, &out, task)?;
    let prompt_file = work.join("prompt.md");
    std::fs::write(&prompt_file, &prompt_text)?;
    let task_ctx = ReviewTask {
        prompt: prompt_text,
        prompt_file: prompt_file.clone(),
        skills: skills.clone(),
        scratch: work.to_path_buf(),
        folder: setup.folder.clone(),
        providers: rrf_step.providers.clone(),
        model_name: rrf_step.model_name.clone(),
        harness_model: rrf_step.harness_model.clone(),
    };
    let mut command = (reviewer.command)(&task_ctx);
    let log_path = work.join("harness.log");
    let log = std::fs::File::create(&log_path)?;
    let ran = run(
        command
            .current_dir(&setup.folder)
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log),
        rrf_step.limit,
    );
    let status = match ran {
        Ok(status) => status,
        Err(error) => bail!("{error:#}{}", failure_trail(&log_path, &setup.out)),
    };
    let text = match std::fs::read_to_string(&out) {
        Ok(text) => text,
        Err(_) => bail!(
            "{} ended ({status}) without writing its findings{}",
            reviewer.label,
            failure_trail(&log_path, &setup.out)
        ),
    };
    let answer: Answer = match serde_json::from_str(&text) {
        Ok(answer) => answer,
        Err(error) => bail!(
            "the findings were not in the expected shape: {error}{}",
            failure_trail(&log_path, &setup.out)
        ),
    };
    Ok((RRF.rate(&answer.factors), answer.findings))
}

const LOG_TAIL: usize = 4000;

fn failure_trail(log: &Path, out: &Path) -> String {
    let mut trail = String::new();
    match keep_log(log, out) {
        Ok(kept) => trail.push_str(&format!("; its full log was kept at {}", kept.display())),
        Err(_) => trail.push_str("; its log could not be kept"),
    }
    let tail = log_tail(log);
    if tail.is_empty() {
        trail.push_str("; it wrote nothing");
    } else {
        trail.push_str(&format!("; the end of its output:\n{tail}"));
    }
    trail
}

fn keep_log(log: &Path, out: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(out)?;
    let kept = out.join("harness.log");
    let bytes = std::fs::read(log)?;
    write_private_bytes(&kept, &bytes)?;
    Ok(kept)
}

fn write_private_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    f.write_all(bytes)?;
    Ok(())
}

fn log_tail(log: &Path) -> String {
    let Ok(bytes) = std::fs::read(log) else {
        return String::new();
    };
    let redacted = redact(&String::from_utf8_lossy(&bytes));
    tail_of(redacted.trim_end(), LOG_TAIL).to_string()
}

fn tail_of(text: &str, limit: usize) -> &str {
    if text.len() <= limit {
        return text;
    }
    let mut start = text.len() - limit;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..]
}

pub fn redact(text: &str) -> String {
    static PATTERNS: std::sync::OnceLock<[regex::Regex; 3]> = std::sync::OnceLock::new();
    let [bearer, key, named] = PATTERNS.get_or_init(|| {
        [
            regex::Regex::new(r"(?i)\bBearer\s+\S+").expect("bearer pattern"),
            regex::Regex::new(r"\bsk-[A-Za-z0-9_-]{8,}").expect("key pattern"),
            regex::Regex::new(
                r#"(?i)((?:api[_-]?key|token|secret|password)["']?\s*[=:]\s*)(?:"[^"\n]*"|'[^'\n]*'|\S+)"#,
            )
            .expect("named pattern"),
        ]
    });
    let text = bearer.replace_all(text, "Bearer [redacted]");
    let text = key.replace_all(&text, "[redacted]");
    named.replace_all(&text, "${1}[redacted]").into_owned()
}

pub fn prompt(skills: &[String], out: &Path, task: &serde_json::Value) -> Result<String> {
    Ok(prompts::RRF
        .replace("{skills}", &skills.join("\n"))
        .replace("{out}", &out.display().to_string())
        .replace("{task}", &serde_json::to_string_pretty(task)?))
}

fn run(command: &mut Command, limit: Duration) -> Result<std::process::ExitStatus> {
    let label = command.get_program().to_string_lossy().to_string();
    let mut child = command
        .spawn()
        .with_context(|| format!("starting {label}"))?;
    wait(&mut child, &label, limit)
}

pub fn wait(child: &mut Child, label: &str, limit: Duration) -> Result<std::process::ExitStatus> {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if start.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            bail!(
                "{label} ran past {} minutes and was stopped",
                limit.as_secs() / 60
            );
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

fn on_path(name: &str) -> bool {
    crate::doctor::on_path(name, std::env::var_os("PATH").as_deref()).is_some()
}

fn muse_signed_in() -> bool {
    run(
        Command::new("muse")
            .arg("usage")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
        LOGIN_CHECK,
    )
    .map(|s| s.success())
    .unwrap_or(false)
}

fn scratch() -> Result<tempfile::TempDir> {
    use std::os::unix::fs::PermissionsExt;
    Ok(tempfile::Builder::new()
        .prefix("ai2-")
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_named_skill_is_embedded() {
        for name in SKILL_NAMES {
            assert!(
                SKILLS.get_file(format!("{name}/SKILL.md")).is_some(),
                "{name}"
            );
        }
    }

    #[test]
    fn the_document_rule_reaches_the_harness_before_the_skills_rules() {
        let given = prompt(
            &["skills/security-audit/SKILL.md".into()],
            Path::new("/tmp/findings.json"),
            &serde_json::json!({ "objective": "Summarise the report" }),
        )
        .unwrap();
        for part in [
            "before any rule in them that asks for a code-level boundary failure",
            "`display:none`",
            "an HTML comment",
            "a script, inline or loaded",
            "`@import`",
            "a secret or API key in the markup",
            "a form that sends data out",
            "Resolving a hidden instruction means removing it",
            "inlining it or removing it",
            "/tmp/findings.json",
            "Summarise the report",
        ] {
            assert!(given.contains(part), "{part}");
        }
        assert!(!given.contains("{skills}") && !given.contains("{task}"));
    }

    #[test]
    fn a_named_harness_is_taken_as_named() {
        let rs = vec![muse_reviewer(), pi_reviewer()];
        let id = |choice: &str| {
            pick_from(&rs, choice)
                .unwrap()
                .map(|chosen| chosen.id().to_string())
        };
        assert_eq!(id("muse").as_deref(), Some("muse"));
        assert_eq!(id("pi").as_deref(), Some("pi"));
        assert_eq!(id("model").as_deref(), Some("model"));
        assert!(id("none").is_none());
        assert!(pick_from(&rs, "dm").is_err());
    }

    fn fixed(id: &str, ready: bool) -> Reviewer {
        let available: Available = if ready {
            Arc::new(|| true)
        } else {
            Arc::new(|| false)
        };
        Reviewer::new(id, id, available, Arc::new(|_| Command::new("true")))
    }

    #[test]
    fn auto_takes_the_first_ready_one_and_never_one_that_must_be_named() {
        let rs = vec![
            fixed("muse", false),
            fixed("claude", true),
            fixed("codex", true),
            fixed("opencode", true).named_only(),
        ];
        let auto = |rs: &[Reviewer]| {
            pick_from(rs, "auto")
                .unwrap()
                .map(|chosen| chosen.id().to_string())
        };
        assert_eq!(auto(&rs).as_deref(), Some("claude"));
        assert_eq!(auto(&rs[2..]).as_deref(), Some("codex"));
        assert_eq!(auto(&rs[3..]), None);
        assert_eq!(
            pick_from(&rs, "opencode")
                .unwrap()
                .map(|c| c.id().to_string())
                .as_deref(),
            Some("opencode")
        );
    }

    fn task(work: &Path) -> ReviewTask {
        ReviewTask {
            prompt: "review it".into(),
            prompt_file: work.join("prompt.md"),
            skills: work.join(".claude/skills"),
            scratch: work.to_path_buf(),
            folder: PathBuf::from("/w"),
            providers: None,
            model_name: Some("p/m".into()),
            harness_model: None,
        }
    }

    fn args(command: &Command) -> Vec<String> {
        command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn muse_and_pi_keep_their_command_lines() {
        let work = Path::new("/s");
        let muse = (muse_reviewer().command)(&task(work));
        assert_eq!(muse.get_program(), "muse");
        assert_eq!(
            args(&muse),
            [
                "exec",
                "--json",
                "--yolo",
                "--disable-request-user-input",
                "--prompt-file",
                "/s/prompt.md"
            ]
        );
        let pi = (pi_reviewer().command)(&task(work));
        assert_eq!(pi.get_program(), "pi");
        let given = args(&pi);
        assert_eq!(given[..4], ["-p", "--mode", "json", "--no-session"]);
        assert!(given.contains(&"/s/.claude/skills/owasp-audit".to_string()));
        assert_eq!(given[given.len() - 3..], ["--model", "p/m", "review it"]);
    }

    #[cfg(feature = "harness")]
    fn fake_registry() -> ::harness::Registry {
        let public = ::harness::Registry::public();
        let mut registry = ::harness::Registry::default();
        #[cfg(feature = "muse-only")]
        let ids = ["muse"];
        #[cfg(not(feature = "muse-only"))]
        let ids = ["claude", "codex", "hermes", "gh"];
        for id in ids {
            registry.put(public.get(id).unwrap().clone(), None);
        }
        registry
    }

    #[cfg(feature = "harness")]
    #[test]
    fn claude_and_codex_review_in_the_folder_headless_with_the_skills_folder_added() {
        let home = tempfile::tempdir().unwrap();
        let registry = fake_registry();
        let rs = registry_reviewers(&registry, home.path(), home.path());
        #[cfg(feature = "muse-only")]
        assert_eq!(
            rs.iter()
                .map(|r| (r.id.as_str(), r.auto))
                .collect::<Vec<_>>(),
            [("muse", true)]
        );
        #[cfg(not(feature = "muse-only"))]
        assert_eq!(
            rs.iter()
                .map(|r| (r.id.as_str(), r.auto))
                .collect::<Vec<_>>(),
            [
                ("muse", true),
                ("claude", true),
                ("codex", true),
                ("pi", true),
                ("hermes", false)
            ]
        );
        assert!(rs
            .iter()
            .all(|r| r.id == "muse" || r.id == "pi" || !(r.available)()));
        let line = |id: &str, model: Option<&str>| {
            let mut given = task(Path::new("/s"));
            given.harness_model = model.map(str::to_string);
            let command = (rs.iter().find(|r| r.id == id).unwrap().command)(&given);
            assert_eq!(command.get_program(), "sh");
            let path = command
                .get_envs()
                .find(|(name, _)| *name == "PATH")
                .and_then(|(_, value)| value)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            assert!(path.starts_with(&home.path().join(".local/bin").display().to_string()));
            args(&command)
        };
        #[cfg(not(feature = "muse-only"))]
        {
            assert_eq!(
                line("claude", None),
                ["-c", "cd '/w' && exec claude -p 'review it' --output-format stream-json --verbose --dangerously-skip-permissions --add-dir '/s'"]
            );
            assert_eq!(
                line("claude", Some("opus")),
                ["-c", "cd '/w' && exec claude -p 'review it' --output-format stream-json --verbose --dangerously-skip-permissions --model 'opus' --add-dir '/s'"]
            );
            assert_eq!(
                line("codex", None),
                ["-c", "cd '/w' && exec codex exec 'review it' --json --dangerously-bypass-approvals-and-sandbox --add-dir '/s'"]
            );
            assert_eq!(
                line("hermes", Some("x")),
                ["-c", "cd '/w' && exec hermes chat -q 'review it' --format stream-json --yolo --model 'x'"]
            );
        }
    }

    #[cfg(feature = "harness")]
    #[test]
    fn a_harness_is_ready_once_installed_and_signed_in() {
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join(".local/bin");
        std::fs::create_dir_all(&bin).unwrap();
        let registry = fake_registry();
        #[cfg(feature = "muse-only")]
        let id = "muse";
        #[cfg(not(feature = "muse-only"))]
        let id = "claude";
        let setup = registry.get(id).unwrap();
        assert!(!ready_to_run(setup, home.path(), home.path()));
        std::fs::write(bin.join(id), "#!/bin/sh\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(bin.join(id), std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(!ready_to_run(setup, home.path(), home.path()));
        ::harness::state::States::at(home.path())
            .set(
                id,
                ::harness::state::State {
                    installed: true,
                    signed_in: true,
                },
            )
            .unwrap();
        assert!(ready_to_run(setup, home.path(), home.path()));
    }

    #[test]
    fn the_harness_flag_beats_the_settings_and_the_settings_beat_auto() {
        let folder = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let mut ctx = setup(folder.path(), out.path());
        let flag =
            || clap::Command::new("t").arg(clap::Arg::new("rating.rrf.harness").long("harness"));
        ctx.args = flag().get_matches_from(["t"]);
        let mut step = Rrf::new(vec![fixed("muse", true)]);
        let chosen = |step: &mut Rrf, ctx: &Context| {
            step.provision(ctx).unwrap();
            let id = step.chosen.as_ref().map(|c| c.id().to_string());
            step.cleanup();
            id
        };
        assert_eq!(chosen(&mut step, &ctx).as_deref(), Some("muse"));
        ctx.reviewer = Some("none".into());
        assert_eq!(chosen(&mut step, &ctx), None);
        ctx.args = flag().get_matches_from(["t", "--harness", "muse"]);
        assert_eq!(chosen(&mut step, &ctx).as_deref(), Some("muse"));
    }

    #[test]
    fn a_model_review_reads_the_files_and_answers_in_one_object() {
        let given = model_prompt(&serde_json::json!({ "objective": "o" })).unwrap();
        assert!(given.contains("Then answer with one JSON object alone:"));
        assert!(!given.contains("{out}") && !given.contains("{skills}"));
        assert!(given.contains("every finding is open"));
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path().canonicalize().unwrap();
        std::fs::write(root.join("a.rs"), "fn a() {}").unwrap();
        std::fs::create_dir(root.join("node_modules")).unwrap();
        std::fs::write(root.join("node_modules/x.js"), "x").unwrap();
        std::fs::write(root.join(".env"), "SECRET=1").unwrap();
        let all = model_files(&root, &[]);
        assert_eq!(all, "=== a.rs ===\nfn a() {}\n");
        assert_eq!(model_files(&root, &["../etc/passwd".into()]), "");
    }
    #[test]
    fn secrets_are_redacted_from_a_log() {
        let text = "Authorization: Bearer abc.def-123\nkey sk-ant-api03-ABCDEFGH12345 used\napi_key=hunter2 next\n\"token\": \"a b c\", ok\nPASSWORD: pw1 end\nmax_tokens: 5\n";
        let clean = redact(text);
        for secret in ["abc.def-123", "ABCDEFGH12345", "hunter2", "a b c", "pw1"] {
            assert!(!clean.contains(secret), "{secret} in {clean}");
        }
        assert!(clean.contains("Bearer [redacted]"));
        assert!(clean.contains("max_tokens: 5"));
        assert!(clean.contains("next"));
    }

    #[test]
    fn the_tail_never_splits_a_character() {
        let text = format!("{}{}", "a", "é".repeat(10));
        let tail = tail_of(&text, 5);
        assert_eq!(tail, "éé");
        assert_eq!(tail_of("short", 100), "short");
    }

    fn failing_reviewer() -> Reviewer {
        Reviewer::new(
            "fake",
            "fake",
            Arc::new(|| true),
            Arc::new(|_| {
                let mut c = Command::new("sh");
                c.args([
                    "-c",
                    "echo starting; echo 'no model: Bearer sekrit-token-1' >&2; exit 1",
                ]);
                c
            }),
        )
    }

    fn setup(folder: &Path, out: &Path) -> Context {
        Context {
            folder: folder.to_path_buf(),
            out: out.to_path_buf(),
            objective: "o".into(),
            changed: Vec::new(),
            days: 7,
            spec: crate::scan::Spec {
                os: "Linux".into(),
                languages: Vec::new(),
                libraries: Vec::new(),
            },
            model: None,
            args: clap::Command::new("t").get_matches_from(["t"]),
            deadline: None,
            providers: None,
            reviewer: None,
            reviewer_model: None,
        }
    }

    #[test]
    fn a_harness_that_fails_leaves_its_cause_in_the_error_and_in_out() {
        let folder = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let reviewer = failing_reviewer();
        let step = Rrf::new(vec![reviewer.clone()]);
        let ctx = setup(folder.path(), out.path());
        let error = rrf(&reviewer, &step, &ctx, &serde_json::json!({}), work.path())
            .err()
            .expect("the harness failed");
        let message = format!("{error:#}");
        println!("{message}");
        assert!(message.contains("fake ended (exit status: 1) without writing its findings"));
        assert!(message.contains("no model: Bearer [redacted]"));
        assert!(message.contains("starting"));
        assert!(!message.contains("sekrit-token-1"));
        let kept = out.path().join("harness.log");
        assert!(message.contains(&kept.display().to_string()));
        assert!(std::fs::read_to_string(&kept)
            .unwrap()
            .contains("sekrit-token-1"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&kept).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn cleanup_removes_the_scratch_after_an_error() {
        let scratch_dir = scratch().unwrap();
        let dir = scratch_dir.path().to_path_buf();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        let mut step = Rrf {
            scratch: Some(scratch_dir),
            ..Rrf::default()
        };
        step.cleanup();
        assert!(!dir.exists());
    }
}
