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

pub struct Scratch {
    pub dir: PathBuf,
}
impl Scratch {
    pub fn path(&self) -> &Path {
        &self.dir
    }
}
pub struct ReviewTask {
    pub folder: PathBuf,
    pub prompt: String,
    pub prompt_file: PathBuf,
    pub skills: PathBuf,
    pub scratch: PathBuf,
    pub out: PathBuf,
    pub providers: Option<PathBuf>,
    pub model_name: Option<String>,
}
#[derive(Clone, Copy)]
pub struct Reviewer {
    pub id: &'static str,
    pub label: &'static str,
    pub available: fn() -> bool,
    pub command: fn(&ReviewTask) -> Command,
}
pub fn muse_reviewer() -> Reviewer {
    Reviewer {
        id: "muse",
        label: "Muse Code",
        available: || on_path("muse") && muse_signed_in(),
        command: muse_command,
    }
}
pub fn pi_reviewer() -> Reviewer {
    Reviewer {
        id: "pi",
        label: "pi",
        available: || on_path("pi"),
        command: pi_command,
    }
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
            let _ = write_private(&agent_dir.join("models.json"), &text);
        }
        c.env("PI_CODING_AGENT_DIR", agent_dir);
    }
    if let Some(model) = &task.model_name {
        c.args(["--model", model]);
    }
    c.arg(&task.prompt);
    c
}
pub fn pick_from(reviewers: &[Reviewer], choice: &str) -> Result<Option<Reviewer>> {
    match choice {
        "none" => Ok(None),
        "auto" => Ok(reviewers.iter().find(|r| (r.available)()).copied()),
        id => reviewers
            .iter()
            .find(|r| r.id == id)
            .copied()
            .map(Some)
            .ok_or_else(|| anyhow!("no reviewer called {id} is registered")),
    }
}
#[cfg(unix)]
fn make_private_dir(dir: &Path) -> Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    Ok(())
}
#[cfg(not(unix))]
fn make_private_dir(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    Ok(())
}
#[cfg(unix)]
fn write_private(path: &Path, text: &str) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    use std::io::Write as _;
    f.write_all(text.as_bytes())?;
    Ok(())
}
#[cfg(not(unix))]
fn write_private(path: &Path, text: &str) -> Result<()> {
    std::fs::write(path, text)?;
    Ok(())
}
pub struct Rrf {
    reviewers: Vec<Reviewer>,
    chosen: Option<Reviewer>,
    scratch: Option<PathBuf>,
    providers: Option<PathBuf>,
    model_name: Option<String>,
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
            limit: RUN_LIMIT,
            choice: "auto".into(),
        }
    }
    pub fn scratch_path(&self) -> Option<&Path> {
        self.scratch.as_deref()
    }
}
impl Default for Rrf {
    fn default() -> Self {
        Self::new(vec![muse_reviewer(), pi_reviewer()])
    }
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
        vec![clap::Arg::new("harness").long("harness").value_name("ID").default_value("auto").help("The coding harness for the Run Risk Factor: auto, none, or a registered reviewer id")]
    }
    fn provision(&mut self, ctx: &Context) -> Result<()> {
        self.choice = ctx
            .arg::<String>(self.id(), "harness")
            .cloned()
            .unwrap_or_else(|| "auto".into());
        self.chosen = pick_from(&self.reviewers, &self.choice)?;
        self.providers = ctx.args.get_one::<std::path::PathBuf>("providers").cloned();
        self.model_name = ctx.model.as_ref().map(Model::name);
        if let Some(remaining) = ctx.remaining() {
            self.limit = remaining.min(RUN_LIMIT);
        }
        if self.chosen.is_some() {
            self.scratch = Some(scratch()?);
        }
        Ok(())
    }
    fn cleanup(&mut self) {
        if let Some(dir) = self.scratch.take() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
    fn rate(&self, ctx: &Context, found: &Found) -> Result<Rated> {
        let Some(reviewer) = self.chosen else {
            return Ok(Rated {
                notes: vec![
                    "No coding harness: install Muse Code and log in, or install pi.".into(),
                ],
                ..Rated::default()
            });
        };
        crate::cli::progress(&format!("reviewing the code with {}", reviewer.label));
        let technologies: Vec<&str> = ctx
            .spec
            .languages
            .iter()
            .map(|l| l.name.as_str())
            .chain(ctx.spec.libraries.iter().map(|l| l.name.as_str()))
            .chain([ctx.spec.os.as_str()])
            .collect();
        let task = serde_json::json!({ "objective": ctx.objective, "changed": ctx.changed, "technologies": technologies, "exploits": found.exploits });
        let work = self
            .scratch
            .clone()
            .ok_or_else(|| anyhow!("the reviewer scratch was not provisioned"))?;
        let (rating, findings) = rrf(reviewer, self, ctx, &task, &work)?;
        Ok(Rated {
            rating: Some(rating),
            findings,
            harness: Some(reviewer.label),
            ..Rated::default()
        })
    }
}
fn rrf(
    reviewer: Reviewer,
    rrf_step: &Rrf,
    setup: &Context,
    task: &serde_json::Value,
    work: &Path,
) -> Result<(Rating, Vec<Finding>)> {
    let skills = work.join("skills");
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
        folder: setup.folder.clone(),
        prompt: prompt_text,
        prompt_file: prompt_file.clone(),
        skills: skills.clone(),
        scratch: work.to_path_buf(),
        out: out.clone(),
        providers: rrf_step.providers.clone(),
        model_name: rrf_step.model_name.clone(),
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

#[cfg(unix)]
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
#[cfg(not(unix))]
fn write_private_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    std::fs::write(path, bytes)?;
    Ok(())
}

pub fn log_tail(log: &Path) -> String {
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

pub fn on_path(name: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(name).is_file()))
}

fn scratch() -> Result<PathBuf> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("ai2-{}-{nanos}", std::process::id()));
    make_private_dir(&dir)?;
    Ok(dir)
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
        assert_eq!(pick_from(&rs, "muse").unwrap().map(|r| r.id), Some("muse"));
        assert_eq!(pick_from(&rs, "pi").unwrap().map(|r| r.id), Some("pi"));
        assert!(pick_from(&rs, "none").unwrap().is_none());
        assert!(pick_from(&rs, "dm").is_err());
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
        Reviewer {
            id: "fake",
            label: "fake",
            available: || true,
            command: |_| {
                let mut c = Command::new("sh");
                c.args([
                    "-c",
                    "echo starting; echo 'no model: Bearer sekrit-token-1' >&2; exit 1",
                ]);
                c
            },
        }
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
        }
    }

    #[test]
    fn a_harness_that_fails_leaves_its_cause_in_the_error_and_in_out() {
        let folder = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let reviewer = failing_reviewer();
        let step = Rrf::new(vec![reviewer]);
        let ctx = setup(folder.path(), out.path());
        let error = rrf(reviewer, &step, &ctx, &serde_json::json!({}), work.path())
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
        let dir = scratch().unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        let mut step = Rrf {
            scratch: Some(dir.clone()),
            ..Rrf::default()
        };
        step.cleanup();
        assert!(!dir.exists());
    }
}
