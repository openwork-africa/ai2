use crate::passphrase::Passphrase;
use crate::{Caller, Presence, PASSPHRASE, POLKIT};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const ACTION: &str = "africa.openwork.ai2-vault.change";
pub const POLICY_FILE: &str = "/usr/share/polkit-1/actions/africa.openwork.ai2-vault.policy";
pub const PKCHECK: &str = "pkcheck";
pub const PKTTYAGENT: &str = "pkttyagent";
const PROMPT_LIMIT: Duration = Duration::from_secs(120);

pub fn policy(daemon_user: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE policyconfig PUBLIC "-//freedesktop//DTD PolicyKit Policy Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/PolicyKit/1/policyconfig.dtd">
<policyconfig>
  <vendor>Openwork</vendor>
  <vendor_url>https://openwork.africa</vendor_url>
  <action id="{ACTION}">
    <description>Change the ai2 vault</description>
    <message>Confirm it is you to change the ai2 vault</message>
    <defaults>
      <allow_any>auth_self</allow_any>
      <allow_inactive>auth_self</allow_inactive>
      <allow_active>auth_self</allow_active>
    </defaults>
    <annotate key="org.freedesktop.policykit.owner">unix-user:{daemon_user}</annotate>
  </action>
</policyconfig>
"#
    )
}

pub fn start_time(stat: &str) -> Option<u64> {
    let after = &stat[stat.rfind(')')? + 1..];
    after.split_whitespace().nth(19)?.parse().ok()
}

pub fn process_start(proc: &Path, pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(proc.join(pid.to_string()).join("stat")).ok()?;
    start_time(&stat)
}

pub fn subject(pid: u32, start: u64, uid: Option<u32>) -> String {
    match uid {
        Some(uid) => format!("{pid},{start},{uid}"),
        None => format!("{pid},{start}"),
    }
}

pub fn check_args(action: &str, subject: &str) -> Vec<String> {
    [
        "--action-id",
        ACTION,
        "--process",
        subject,
        "--allow-user-interaction",
        "--detail",
        "polkit.message",
        action,
    ]
    .map(String::from)
    .to_vec()
}

pub fn tty_agent_args(subject: &str, notify_fd: i32) -> Vec<String> {
    [
        "--process".to_string(),
        subject.to_string(),
        "--fallback".to_string(),
        "--notify-fd".to_string(),
        notify_fd.to_string(),
    ]
    .to_vec()
}

pub fn find(program: &str, path: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    let path = path?;
    std::env::split_paths(path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}

pub trait Exec: Send + Sync {
    fn status(&self, program: &Path, args: &[String], limit: Duration) -> Result<Option<i32>>;
}

pub struct System;

impl Exec for System {
    fn status(&self, program: &Path, args: &[String], limit: Duration) -> Result<Option<i32>> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("running {}", program.display()))?;
        let deadline = Instant::now() + limit;
        loop {
            if let Some(status) = child.try_wait()? {
                return Ok(status.code());
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(None);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

pub struct Polkit {
    pub policy: PathBuf,
    pub proc: PathBuf,
    pub path: Option<std::ffi::OsString>,
    pub limit: Duration,
    pub exec: Box<dyn Exec>,
}

impl Default for Polkit {
    fn default() -> Self {
        Self {
            policy: POLICY_FILE.into(),
            proc: "/proc".into(),
            path: std::env::var_os("PATH"),
            limit: PROMPT_LIMIT,
            exec: Box::new(System),
        }
    }
}

impl Polkit {
    pub fn pkcheck(&self) -> Option<PathBuf> {
        find(PKCHECK, self.path.as_deref())
    }

    pub fn available(&self) -> bool {
        self.policy.is_file() && self.pkcheck().is_some()
    }

    pub fn ask(&self, action: &str, caller: &Caller) -> Result<bool> {
        if action.trim().is_empty() {
            bail!("say what needs a human present");
        }
        let Some(pid) = caller.pid else {
            bail!("the caller's process is unknown, so polkit cannot ask");
        };
        let Some(start) = process_start(&self.proc, pid) else {
            bail!("process {pid} is gone or unreadable, so polkit cannot ask");
        };
        let Some(pkcheck) = self.pkcheck() else {
            bail!("{PKCHECK} is not installed");
        };
        let args = check_args(action, &subject(pid, start, caller.uid));
        Ok(self.exec.status(&pkcheck, &args, self.limit)? == Some(0))
    }
}

pub struct PolkitOrPassphrase {
    pub polkit: Polkit,
    pub passphrase: Passphrase,
}

impl Presence for PolkitOrPassphrase {
    fn confirm(&self, action: &str, caller: &Caller, passphrase: Option<&str>) -> Result<bool> {
        if self.polkit.available() {
            return self.polkit.ask(action, caller);
        }
        self.passphrase.confirm(action, caller, passphrase)
    }

    fn kind(&self) -> &'static str {
        if self.polkit.available() {
            POLKIT
        } else {
            PASSPHRASE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    const STAT: &str = "4242 (a (weird) name) S 1 4242 4242 0 -1 4194560 100 0 0 0 1 2 0 0 20 0 1 0 987654 1000000 200 18446744073709551615";

    type Ran = Arc<Mutex<Vec<(PathBuf, Vec<String>)>>>;

    #[derive(Clone, Default)]
    struct Fake {
        ran: Ran,
        answer: Option<i32>,
    }

    impl Exec for Fake {
        fn status(&self, program: &Path, args: &[String], _: Duration) -> Result<Option<i32>> {
            self.ran
                .lock()
                .unwrap()
                .push((program.to_path_buf(), args.to_vec()));
            Ok(self.answer)
        }
    }

    struct Fixture {
        _dir: tempfile::TempDir,
        polkit: Polkit,
        ran: Ran,
    }

    fn fixture(answer: Option<i32>, installed: bool) -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join(PKCHECK), "").unwrap();
        let proc = dir.path().join("proc");
        std::fs::create_dir_all(proc.join("4242")).unwrap();
        std::fs::write(proc.join("4242").join("stat"), STAT).unwrap();
        let policy = dir.path().join("action.policy");
        if installed {
            std::fs::write(&policy, policy_text()).unwrap();
        }
        let fake = Fake {
            answer,
            ..Fake::default()
        };
        let ran = Arc::clone(&fake.ran);
        Fixture {
            polkit: Polkit {
                policy,
                proc,
                path: Some(bin.into_os_string()),
                limit: Duration::from_secs(1),
                exec: Box::new(fake),
            },
            ran,
            _dir: dir,
        }
    }

    fn policy_text() -> String {
        policy("ai2-vault")
    }

    #[test]
    fn the_start_time_is_read_after_the_command_name() {
        assert_eq!(start_time(STAT), Some(987654));
        assert_eq!(start_time("1 (x) S"), None);
        assert_eq!(subject(7, 9, Some(1000)), "7,9,1000");
        assert_eq!(subject(7, 9, None), "7,9");
    }

    #[test]
    fn pkcheck_is_asked_about_the_calling_process_with_the_action_shown() {
        let fixture = fixture(Some(0), true);
        let caller = Caller {
            pid: Some(4242),
            uid: Some(1000),
        };
        assert!(fixture
            .polkit
            .ask("Delete the vault key k", &caller)
            .unwrap());
        let ran = fixture.ran.lock().unwrap().clone();
        assert_eq!(ran.len(), 1);
        assert!(ran[0].0.ends_with(PKCHECK));
        assert_eq!(
            ran[0].1,
            [
                "--action-id",
                ACTION,
                "--process",
                "4242,987654,1000",
                "--allow-user-interaction",
                "--detail",
                "polkit.message",
                "Delete the vault key k"
            ]
        );
    }

    #[test]
    fn anything_but_a_zero_exit_is_no() {
        let caller = Caller {
            pid: Some(4242),
            uid: None,
        };
        for answer in [Some(1), Some(2), Some(3), None] {
            assert!(!fixture(answer, true).polkit.ask("x", &caller).unwrap());
        }
        let fixture = fixture(Some(0), true);
        assert!(fixture.polkit.ask("x", &Caller::default()).is_err());
        let gone = Caller {
            pid: Some(1),
            uid: None,
        };
        assert!(fixture.polkit.ask("x", &gone).is_err());
        assert!(fixture.polkit.ask(" ", &caller).is_err());
        assert!(fixture.ran.lock().unwrap().is_empty());
    }

    #[test]
    fn without_polkit_the_passphrase_decides() {
        let fixture = fixture(Some(0), false);
        let dir = tempfile::tempdir().unwrap();
        let hash = dir.path().join("passphrase");
        crate::passphrase::set(&hash, "correct horse battery").unwrap();
        let both = PolkitOrPassphrase {
            polkit: fixture.polkit,
            passphrase: Passphrase::new(&hash),
        };
        let caller = Caller {
            pid: Some(4242),
            uid: None,
        };
        assert_eq!(both.kind(), PASSPHRASE);
        assert!(both
            .confirm("x", &caller, Some("correct horse battery"))
            .unwrap());
        assert!(!both.confirm("x", &caller, None).unwrap());
        assert!(fixture.ran.lock().unwrap().is_empty());
        std::fs::write(&both.polkit.policy, policy_text()).unwrap();
        assert_eq!(both.kind(), POLKIT);
        assert!(both.confirm("x", &caller, None).unwrap());
    }

    #[test]
    fn the_policy_asks_for_the_users_own_password_and_trusts_the_daemon() {
        let text = policy("ai2-vault");
        assert!(text.contains(&format!("<action id=\"{ACTION}\">")));
        assert_eq!(text.matches("auth_self<").count(), 3);
        assert!(!text.contains("auth_self_keep") && !text.contains("auth_admin"));
        assert!(text.contains(
            "<annotate key=\"org.freedesktop.policykit.owner\">unix-user:ai2-vault</annotate>"
        ));
        assert_eq!(
            tty_agent_args("10,20", 3),
            ["--process", "10,20", "--fallback", "--notify-fd", "3"]
        );
    }

    #[test]
    fn programs_are_found_on_the_path() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("pkcheck"), "").unwrap();
        let path = std::env::join_paths([Path::new("/nonexistent"), dir.path()]).unwrap();
        assert_eq!(
            find("pkcheck", Some(&path)),
            Some(dir.path().join("pkcheck"))
        );
        assert_eq!(find("pkttyagent", Some(&path)), None);
        assert_eq!(find("pkcheck", None), None);
    }

    #[test]
    #[ignore]
    fn the_desktop_or_terminal_agent_asks_for_the_login_password() {
        let polkit = Polkit::default();
        assert!(polkit.available(), "install {POLICY_FILE} first");
        let caller = Caller {
            pid: Some(std::process::id()),
            uid: Some(unsafe { libc::geteuid() }),
        };
        assert!(polkit.ask("test the ai2 vault prompt", &caller).unwrap());
    }
}
