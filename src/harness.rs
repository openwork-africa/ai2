use crate::model::Model;
use crate::prompts;
use crate::step::{Rated, Run, Step};
use crate::steps::{Rating, Scale, Scored, RRF};
use anyhow::{anyhow, bail, Context, Result};
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

#[derive(Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Harness {
    Muse,
    Pi,
}

impl Harness {
    pub fn label(self) -> &'static str {
        match self {
            Harness::Muse => "Muse Code",
            Harness::Pi => "pi",
        }
    }
}

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

pub struct Rrf;

impl Step for Rrf {
    fn scale(&self) -> &'static Scale {
        &RRF
    }

    fn rate(&self, run: &Run) -> Result<Rated> {
        let Some(harness) = pick(&run.harness) else {
            return Ok(Rated {
                notes: vec![
                    "No coding harness: install Muse Code and log in, or install pi.".into(),
                ],
                ..Rated::default()
            });
        };
        eprintln!("ai2: reviewing the code with {}", harness.label());
        let technologies: Vec<&str> = run
            .spec
            .languages
            .iter()
            .map(|l| l.name.as_str())
            .chain(run.spec.libraries.iter().map(|l| l.name.as_str()))
            .chain([run.spec.os.as_str()])
            .collect();
        let task = serde_json::json!({ "objective": run.objective, "changed": run.changed, "technologies": technologies, "exploits": run.exploits });
        let (rating, findings) = rrf(harness, run, &task)?;
        Ok(Rated {
            rating: Some(rating),
            findings,
            harness: Some(harness.label()),
            ..Rated::default()
        })
    }
}

pub fn pick(choice: &str) -> Option<Harness> {
    match choice {
        "muse" => Some(Harness::Muse),
        "pi" => Some(Harness::Pi),
        "none" => None,
        _ if on_path("muse") && muse_signed_in() => Some(Harness::Muse),
        _ if on_path("pi") => Some(Harness::Pi),
        _ => None,
    }
}

fn rrf(harness: Harness, setup: &Run, task: &serde_json::Value) -> Result<(Rating, Vec<Finding>)> {
    let work = scratch()?;
    let skills = work.join("skills");
    SKILLS.extract(&skills).context("writing the skills")?;
    let out = work.join("findings.json");
    let read: Vec<String> = SKILL_NAMES
        .iter()
        .map(|name| skills.join(name).join("SKILL.md").display().to_string())
        .collect();
    let prompt = prompt(&read, &out, task)?;
    let prompt_file = work.join("prompt.md");
    std::fs::write(&prompt_file, &prompt)?;
    let mut command = match harness {
        Harness::Muse => {
            let mut c = Command::new("muse");
            c.args([
                "exec",
                "--json",
                "--yolo",
                "--disable-request-user-input",
                "--prompt-file",
            ])
            .arg(&prompt_file);
            c
        }
        Harness::Pi => {
            let mut c = Command::new("pi");
            c.args(["-p", "--mode", "json", "--no-session"]);
            for name in SKILL_NAMES {
                c.arg("--skill").arg(skills.join(name));
            }
            if let Some(providers) = &setup.providers {
                let agent_dir = work.join("pi");
                std::fs::create_dir_all(&agent_dir)?;
                std::fs::copy(providers, agent_dir.join("models.json"))
                    .context("giving pi the providers")?;
                c.env("PI_CODING_AGENT_DIR", agent_dir);
            }
            if let Some(model) = setup.model.as_ref().map(Model::name) {
                c.args(["--model", &model]);
            }
            c.arg(&prompt);
            c
        }
    };
    let log = std::fs::File::create(work.join("harness.log"))?;
    let status = run(
        command
            .current_dir(&setup.folder)
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log),
        RUN_LIMIT,
    )?;
    let text = std::fs::read_to_string(&out).map_err(|_| {
        anyhow!(
            "{} ended ({status}) without writing its findings; its log is {}",
            harness.label(),
            work.join("harness.log").display()
        )
    })?;
    let answer: Answer =
        serde_json::from_str(&text).context("the findings were not in the expected shape")?;
    let _ = std::fs::remove_dir_all(&work);
    Ok((RRF.rate(&answer.factors), answer.findings))
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
    std::fs::create_dir_all(&dir)?;
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
        assert!(pick("muse") == Some(Harness::Muse));
        assert!(pick("pi") == Some(Harness::Pi));
        assert!(pick("none").is_none());
    }
}
