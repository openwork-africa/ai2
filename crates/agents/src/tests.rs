use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use harness::{Headless, Registry, Setup};
use serde_json::json;

use crate::{
    AgentStore, Agents, EnhanceStatus, LocalStore, NewAgent, Outcome, Review, Reviewed, RunAsk,
    RunEvent, RunState, Stream,
};

const HASH: &str = "0123456789abcdef";

fn script(dir: &Path) -> PathBuf {
    let file = dir.join("fake-agent");
    std::fs::write(
        &file,
        "#!/bin/sh\necho \"working on: $1\"\necho \"args: $*\"\necho \"$1\" > edited.txt\necho 'a warning' >&2\nexit 3\n",
    )
    .unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    file
}

fn registry(command: &Path) -> impl Fn() -> Registry + Send + Sync + 'static {
    let command = command.display().to_string();
    move || {
        let mut registry = Registry::default();
        registry.put(
            Setup {
                id: "fake".into(),
                label: "Fake".into(),
                command: command.clone(),
                install: "true".into(),
                agent: Some(harness::Agent {
                    models: vec![harness::ModelChoice::new("small", "Small")],
                    headless: Some(Headless {
                        run: command.clone(),
                        flags: "--quiet".into(),
                        personality: Some(harness::Personality::Flag("--system".into())),
                        ..Headless::default()
                    }),
                }),
                ..Setup::default()
            },
            None,
        );
        registry
    }
}

#[derive(Default)]
struct FakeReview {
    asked: Mutex<Vec<(PathBuf, String, bool)>>,
}

impl Review for FakeReview {
    fn review(
        &self,
        folder: &Path,
        objective: &str,
        say: &mut dyn FnMut(String),
    ) -> anyhow::Result<Reviewed> {
        let edited = folder.join("edited.txt").is_file();
        self.asked
            .lock()
            .unwrap()
            .push((folder.to_path_buf(), objective.to_string(), edited));
        say("ai2: rating the Modern Security Rating".into());
        Ok(Reviewed {
            hash: HASH.into(),
            review: json!({ "folder": folder.display().to_string(), "objective": objective }),
        })
    }
}

struct Fixture {
    home: tempfile::TempDir,
    bin: tempfile::TempDir,
    folder: tempfile::TempDir,
    review: Arc<FakeReview>,
}

impl Fixture {
    fn new() -> Self {
        Self {
            home: tempfile::tempdir().unwrap(),
            bin: tempfile::tempdir().unwrap(),
            folder: tempfile::tempdir().unwrap(),
            review: Arc::new(FakeReview::default()),
        }
    }

    fn agents(&self) -> Agents {
        let command = script(self.bin.path());
        Agents::local(self.home.path(), registry(&command)).with_review(self.review.clone())
    }

    fn real_folder(&self) -> PathBuf {
        self.folder.path().canonicalize().unwrap()
    }
}

fn long_prompt() -> String {
    format!(
        "<system_prompt>\nWho you are:\nYou are Scout, a careful reviewer of code. {}\n</system_prompt>\n<description>A careful code reviewer</description>",
        "You speak plainly and briefly, in their language. ".repeat(8)
    )
}

fn new(name: &str) -> NewAgent {
    NewAgent {
        name: name.into(),
        harness: "fake".into(),
        ..NewAgent::default()
    }
}

#[test]
fn the_local_store_keeps_agents_by_name_and_forgets_them() {
    let fixture = Fixture::new();
    let agents = fixture.agents();
    let store = LocalStore::new(fixture.home.path());
    assert_eq!(store.dir(), fixture.home.path().join("agents"));
    assert!(store.list().unwrap().is_empty());
    for name in ["beta", "alpha"] {
        agents.create(&new(name), None).unwrap();
    }
    let names: Vec<String> = store.list().unwrap().into_iter().map(|a| a.name).collect();
    assert_eq!(names, ["alpha", "beta"]);
    let kept = store.get("beta").unwrap().unwrap();
    assert_eq!(kept.harness, "fake");
    assert!(fixture.home.path().join("agents/beta.json").is_file());
    let again = agents
        .create(
            &NewAgent {
                model: Some("small".into()),
                ..new("beta")
            },
            None,
        )
        .unwrap();
    assert_eq!(again.created, kept.created);
    assert_eq!(again.model.as_deref(), Some("small"));
    assert_eq!(store.get("missing").unwrap(), None);
    assert_eq!(store.get("../etc/passwd").unwrap(), None);
    assert!(store.remove("beta").unwrap());
    assert!(!store.remove("beta").unwrap());
    let gone = agents.get("beta").unwrap_err();
    assert_eq!(gone.status, 404);
    assert_eq!(gone.message, "There is no agent called beta.");
    assert_eq!(agents.remove("beta").unwrap_err().error, "not_found");
}

#[test]
fn a_short_personality_is_expanded_by_the_model_passed_in() {
    let fixture = Fixture::new();
    let asked = Arc::new(Mutex::new(Vec::new()));
    let seen = asked.clone();
    let agents = fixture
        .agents()
        .with_enhancer(Arc::new(move |name: Option<&str>| {
            seen.lock().unwrap().push(name.map(String::from));
            let ask: crate::Ask = Arc::new(|system: &str, input: &str| {
                assert!(system.starts_with("You write system prompts"));
                assert!(input.contains("<their_words>a careful reviewer</their_words>"));
                Ok(long_prompt())
            });
            Ok(ask)
        }));
    let made = agents
        .create(
            &NewAgent {
                personality: Some("a careful reviewer".into()),
                ..new("scout")
            },
            Some("p/m"),
        )
        .unwrap();
    let personality = made.personality.clone().unwrap();
    assert!(personality
        .content
        .starts_with("Who you are:\nYou are Scout"));
    assert_eq!(personality.seed.as_deref(), Some("a careful reviewer"));
    assert_eq!(personality.enhanced, Some(true));
    assert_eq!(made.description.as_deref(), Some("A careful code reviewer"));
    assert_eq!(agents.get("scout").unwrap(), made);
    assert_eq!(asked.lock().unwrap().clone(), [Some("p/m".to_string())]);
    let again = agents.enhance("scout", None).unwrap();
    assert_eq!(again.status, EnhanceStatus::Skipped);
    assert_eq!(again.agent, made);

    let plain = fixture.agents();
    plain
        .create(
            &NewAgent {
                personality: Some("be brief".into()),
                ..new("terse")
            },
            None,
        )
        .unwrap();
    let failed = plain.enhance("terse", None).unwrap();
    assert_eq!(failed.status, EnhanceStatus::Failed);
    assert_eq!(
        failed.reason.as_deref(),
        Some(crate::enhance::FAILED_NO_MODEL)
    );
    assert_eq!(
        plain.get("terse").unwrap().personality.unwrap().content,
        "be brief"
    );
    assert!(matches!(
        plain.enhance_words("be kind", "", None),
        Outcome::Failed { .. }
    ));
}

#[test]
fn a_run_streams_its_lines_records_its_exit_and_is_reviewed_after() {
    let fixture = Fixture::new();
    let agents = fixture.agents();
    agents
        .create(
            &NewAgent {
                model: Some("small".into()),
                personality: Some("Be brief.".into()),
                folder: Some(fixture.folder.path().display().to_string()),
                ..new("scout")
            },
            None,
        )
        .unwrap();
    let mut events = Vec::new();
    let finished = agents
        .run(
            &RunAsk {
                agent: Some("scout".into()),
                prompt: " fix the login ".into(),
                ..RunAsk::default()
            },
            &mut |event| events.push(event.clone()),
        )
        .unwrap();
    let real = fixture.real_folder();
    let run = &finished.run;
    assert_eq!(run.agent.as_deref(), Some("scout"));
    assert_eq!(run.harness, "fake");
    assert_eq!(run.model.as_deref(), Some("small"));
    assert_eq!(run.folder, real.display().to_string());
    assert_eq!(run.prompt, "fix the login");
    assert_eq!(run.exit, Some(3));
    assert!(!run.ok());
    assert!(run.ended.is_some());
    assert_eq!(run.state, RunState::Ended);
    assert_eq!(run.review.as_deref(), Some(HASH));
    assert_eq!(
        finished.review.as_ref().unwrap()["objective"],
        "fix the login"
    );
    assert_eq!(
        std::fs::read_to_string(real.join("edited.txt")).unwrap(),
        "fix the login\n"
    );
    let asked = fixture.review.asked.lock().unwrap().clone();
    assert_eq!(asked, [(real.clone(), "fix the login".to_string(), true)]);

    let names: Vec<&str> = events.iter().map(RunEvent::name).collect();
    assert_eq!(names.first(), Some(&"started"));
    assert_eq!(
        names[names.len() - 4..],
        ["exited", "review", "reviewed", "done"]
    );
    assert!(events.contains(&RunEvent::Line {
        stream: Stream::Out,
        text: "working on: fix the login".into()
    }));
    assert!(events.contains(&RunEvent::Line {
        stream: Stream::Out,
        text: "args: fix the login --quiet --model small --system Be brief.".into()
    }));
    assert!(events.contains(&RunEvent::Line {
        stream: Stream::Err,
        text: "a warning".into()
    }));
    assert!(events.contains(&RunEvent::Exited {
        exit: Some(3),
        error: None
    }));

    assert_eq!(agents.runs().get(&run.id).unwrap(), *run);
    assert_eq!(agents.runs().list(), std::slice::from_ref(run));
    let log = std::fs::read_to_string(&run.log).unwrap();
    assert!(log.contains("working on: fix the login") && log.contains("a warning"));
    assert!(Path::new(&run.log).starts_with(fixture.home.path().join("runs").join(&run.id)));
    let mut follow = agents.runs().follow(&run.id, 0).unwrap();
    let replayed: Vec<RunEvent> = follow.read().into_iter().map(|(_, e)| e).collect();
    assert_eq!(replayed, events);
    assert!(follow.done());
    let mut later = agents.runs().follow(&run.id, events.len() - 1).unwrap();
    let last = later.read();
    assert_eq!(last.len(), 1);
    assert_eq!(last[0].0, events.len() - 1);
    assert_eq!(last[0].1.name(), "done");
}

#[test]
fn a_run_without_a_reviewer_still_ends_and_a_harness_runs_without_an_agent() {
    let fixture = Fixture::new();
    let command = script(fixture.bin.path());
    let agents = Agents::local(fixture.home.path(), registry(&command));
    let finished = agents
        .run(
            &RunAsk {
                harness: Some("fake".into()),
                folder: Some(fixture.folder.path().display().to_string()),
                prompt: "hi".into(),
                ..RunAsk::default()
            },
            &mut |_| {},
        )
        .unwrap();
    assert_eq!(finished.run.agent, None);
    assert_eq!(finished.run.exit, Some(3));
    assert_eq!(finished.run.review, None);
    assert_eq!(finished.review, None);
}

#[test]
fn a_run_is_refused_before_it_starts_when_its_folder_or_prompt_or_harness_is_wrong() {
    let fixture = Fixture::new();
    let agents = fixture.agents();
    let refused = |ask: RunAsk| agents.run(&ask, &mut |_| {}).unwrap_err();
    let folder = Some(fixture.folder.path().display().to_string());
    let missing = refused(RunAsk {
        harness: Some("fake".into()),
        folder: Some(fixture.folder.path().join("nope").display().to_string()),
        prompt: "hi".into(),
        ..RunAsk::default()
    });
    assert_eq!(missing.status, 400);
    assert_eq!(missing.error, "not_a_folder");
    let file = fixture.folder.path().join("a-file");
    std::fs::write(&file, "x").unwrap();
    assert_eq!(
        refused(RunAsk {
            harness: Some("fake".into()),
            folder: Some(file.display().to_string()),
            prompt: "hi".into(),
            ..RunAsk::default()
        })
        .error,
        "not_a_folder"
    );
    assert_eq!(
        refused(RunAsk {
            harness: Some("fake".into()),
            prompt: "hi".into(),
            ..RunAsk::default()
        })
        .error,
        "folder_required"
    );
    assert_eq!(
        refused(RunAsk {
            harness: Some("fake".into()),
            folder: folder.clone(),
            prompt: "  ".into(),
            ..RunAsk::default()
        })
        .error,
        "prompt_required"
    );
    assert_eq!(
        refused(RunAsk {
            agent: Some("scout".into()),
            harness: Some("fake".into()),
            folder: folder.clone(),
            prompt: "hi".into(),
            ..RunAsk::default()
        })
        .error,
        "agent_or_harness"
    );
    assert_eq!(
        refused(RunAsk {
            agent: Some("ghost".into()),
            folder: folder.clone(),
            prompt: "hi".into(),
            ..RunAsk::default()
        })
        .error,
        "not_found"
    );
    assert_eq!(
        refused(RunAsk {
            harness: Some("fake".into()),
            model: Some("huge".into()),
            folder,
            prompt: "hi".into(),
            ..RunAsk::default()
        })
        .message,
        "Fake has no model called huge."
    );
    assert!(agents.runs().list().is_empty());
    assert!(fixture.review.asked.lock().unwrap().is_empty());
}

#[test]
fn no_review_skips_the_review_after_the_run() {
    let fixture = Fixture::new();
    let finished = fixture
        .agents()
        .run(
            &RunAsk {
                harness: Some("fake".into()),
                folder: Some(fixture.folder.path().display().to_string()),
                prompt: "hi".into(),
                no_review: true,
                ..RunAsk::default()
            },
            &mut |_| {},
        )
        .unwrap();
    assert_eq!(finished.run.review, None);
    assert_eq!(finished.run.state, RunState::Ended);
    assert!(fixture.review.asked.lock().unwrap().is_empty());
}

#[test]
fn a_second_run_in_the_same_folder_waits_until_the_first_ends() {
    let fixture = Fixture::new();
    let real = fixture.real_folder();
    let held = crate::runs::FolderLock::take(&fixture.home.path().join("runs"), &real, &mut || {
        panic!("nothing else holds the folder yet")
    })
    .unwrap();
    let agents = fixture.agents();
    let folder = fixture.folder.path().display().to_string();
    let (tx, rx) = std::sync::mpsc::channel();
    let waiting = std::thread::spawn(move || {
        agents
            .run(
                &RunAsk {
                    harness: Some("fake".into()),
                    folder: Some(folder),
                    prompt: "hi".into(),
                    no_review: true,
                    ..RunAsk::default()
                },
                &mut |event| {
                    let _ = tx.send(event.name());
                },
            )
            .unwrap()
    });
    let first = rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
    assert_eq!(first, "waiting");
    assert!(rx
        .recv_timeout(std::time::Duration::from_millis(300))
        .is_err());
    drop(held);
    let finished = waiting.join().unwrap();
    assert_eq!(finished.run.exit, Some(3));
}
