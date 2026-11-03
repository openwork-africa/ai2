use crate::State;
use anyhow::{bail, Context, Result};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Root {
    No,
    Yes,
    IfCached,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cmd {
    pub program: String,
    pub args: Vec<String>,
    pub input: Option<Vec<u8>>,
    pub root: Root,
    pub show: bool,
}

impl Cmd {
    pub fn new(program: &str, args: &[&str]) -> Self {
        Self {
            program: program.into(),
            args: args.iter().map(|arg| arg.to_string()).collect(),
            input: None,
            root: Root::No,
            show: false,
        }
    }

    pub fn root(self) -> Self {
        Self {
            root: Root::Yes,
            ..self
        }
    }

    pub fn root_if(self, prompt: bool) -> Self {
        Self {
            root: if prompt { Root::Yes } else { Root::IfCached },
            ..self
        }
    }

    pub fn shown(self) -> Self {
        Self { show: true, ..self }
    }

    pub fn input(self, bytes: &[u8]) -> Self {
        Self {
            input: Some(bytes.to_vec()),
            ..self
        }
    }

    pub fn line(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ran {
    pub ok: bool,
    pub out: String,
}

impl Ran {
    pub fn ok(out: &str) -> Self {
        Self {
            ok: true,
            out: out.into(),
        }
    }

    pub fn failed() -> Self {
        Self::default()
    }
}

pub trait Runner {
    fn run(&self, cmd: &Cmd) -> Result<Ran>;
    fn read(&self, path: &Path) -> Option<String>;
    fn write(&self, path: &Path, contents: &str, mode: u32) -> Result<()>;
    fn exists(&self, path: &Path) -> bool;
    fn mkdir(&self, path: &Path, mode: u32) -> Result<()>;
    fn ask_hidden(&self, prompt: &str) -> Result<String>;
    fn say(&self, what: &str);

    fn step_started(&self, _id: &str) {}

    fn step_finished(&self, _id: &str, _state: &State) {}

    fn ask_passphrase(&self, prompt: &str, again: &str) -> Result<String> {
        let first = self.ask_hidden(prompt)?;
        if first != self.ask_hidden(again)? {
            bail!("the two passphrases differ");
        }
        Ok(first)
    }
}

pub struct System;

pub fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

impl Runner for System {
    fn run(&self, cmd: &Cmd) -> Result<Ran> {
        let mut command = match (cmd.root, is_root()) {
            (Root::No, _) | (_, true) => Command::new(&cmd.program),
            (Root::Yes, false) => {
                self.say(&format!("running as root through sudo: {}", cmd.line()));
                let mut sudo = Command::new("sudo");
                sudo.arg(&cmd.program);
                sudo
            }
            (Root::IfCached, false) => {
                let mut sudo = Command::new("sudo");
                sudo.args(["-n", &cmd.program]);
                sudo
            }
        };
        command.args(&cmd.args);
        command.stdin(if cmd.input.is_some() {
            Stdio::piped()
        } else if cmd.show {
            Stdio::inherit()
        } else {
            Stdio::null()
        });
        command.stdout(if cmd.show {
            Stdio::from(std::io::stderr())
        } else {
            Stdio::piped()
        });
        command.stderr(if cmd.show || cmd.root == Root::Yes {
            Stdio::inherit()
        } else {
            Stdio::null()
        });
        if cmd.show {
            self.say(&format!("$ {}", cmd.line()));
        }
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Ran::failed()),
            Err(error) => return Err(error).with_context(|| format!("starting {}", cmd.line())),
        };
        if let (Some(bytes), Some(mut stdin)) = (&cmd.input, child.stdin.take()) {
            stdin.write_all(bytes)?;
        }
        let output = child.wait_with_output()?;
        Ok(Ran {
            ok: output.status.success(),
            out: String::from_utf8_lossy(&output.stdout).into_owned(),
        })
    }

    fn read(&self, path: &Path) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }

    fn write(&self, path: &Path, contents: &str, mode: u32) -> Result<()> {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("making {}", dir.display()))?;
        }
        let temp = path.with_extension("new");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(mode)
            .open(&temp)
            .with_context(|| format!("writing {}", temp.display()))?;
        file.write_all(contents.as_bytes())?;
        file.set_permissions(std::fs::Permissions::from_mode(mode))?;
        std::fs::rename(&temp, path).with_context(|| format!("writing {}", path.display()))
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn mkdir(&self, path: &Path, mode: u32) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(path).with_context(|| format!("making {}", path.display()))?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
            .with_context(|| format!("securing {}", path.display()))
    }

    fn ask_hidden(&self, prompt: &str) -> Result<String> {
        keys::tty::hidden(prompt).map(|line| line.to_string())
    }

    fn say(&self, what: &str) {
        eprintln!("ai2 setup: {what}");
    }
}

#[cfg(any(test, feature = "fake"))]
pub mod fake {
    use super::*;
    use serde_json::json;
    use std::cell::RefCell;
    use std::collections::{BTreeMap, VecDeque};
    use std::path::PathBuf;

    #[derive(Default)]
    pub struct Fake {
        pub ran: RefCell<Vec<String>>,
        pub answers: RefCell<Vec<(String, Ran, bool)>>,
        pub files: RefCell<BTreeMap<PathBuf, (String, u32)>>,
        pub typed: RefCell<VecDeque<String>>,
        pub said: RefCell<Vec<String>>,
        pub inputs: RefCell<Vec<Vec<u8>>>,
        pub asked: RefCell<Vec<String>>,
        pub steps: RefCell<Vec<String>>,
    }

    impl Fake {
        pub fn answer(&self, prefix: &str, ran: Ran) -> &Self {
            self.answers
                .borrow_mut()
                .insert(0, (prefix.into(), ran, false));
            self
        }

        pub fn once(&self, prefix: &str, ran: Ran) -> &Self {
            self.answers
                .borrow_mut()
                .insert(0, (prefix.into(), ran, true));
            self
        }

        pub fn at(&self, prefix: &str) -> Option<usize> {
            self.ran
                .borrow()
                .iter()
                .position(|line| line.starts_with(prefix))
        }

        pub fn file(&self, path: &str, contents: &str) -> &Self {
            self.files
                .borrow_mut()
                .insert(path.into(), (contents.into(), 0o644));
            self
        }

        pub fn type_in(&self, text: &str) -> &Self {
            self.typed.borrow_mut().push_back(text.into());
            self
        }

        pub fn lines(&self) -> Vec<String> {
            self.ran.borrow().clone()
        }

        pub fn ran_any(&self, prefix: &str) -> bool {
            self.ran
                .borrow()
                .iter()
                .any(|line| line.starts_with(prefix))
        }
    }

    impl Runner for Fake {
        fn run(&self, cmd: &Cmd) -> Result<Ran> {
            let line = match cmd.root {
                Root::No => cmd.line(),
                Root::Yes => format!("root {}", cmd.line()),
                Root::IfCached => format!("root? {}", cmd.line()),
            };
            self.ran.borrow_mut().push(line.clone());
            if let Some(bytes) = &cmd.input {
                self.inputs.borrow_mut().push(bytes.clone());
                if cmd.program == "sh" && cmd.args.len() == 4 && cmd.args[1].contains("cat >") {
                    self.files.borrow_mut().insert(
                        cmd.args[3].clone().into(),
                        (String::from_utf8_lossy(bytes).into_owned(), 0),
                    );
                }
            }
            let mut answers = self.answers.borrow_mut();
            let Some(at) = answers
                .iter()
                .position(|(prefix, _, _)| line.starts_with(prefix.as_str()))
            else {
                return Ok(Ran::ok(""));
            };
            let ran = answers[at].1.clone();
            if answers[at].2 {
                answers.remove(at);
            }
            Ok(ran)
        }

        fn read(&self, path: &Path) -> Option<String> {
            self.files.borrow().get(path).map(|(text, _)| text.clone())
        }

        fn write(&self, path: &Path, contents: &str, mode: u32) -> Result<()> {
            self.files
                .borrow_mut()
                .insert(path.into(), (contents.into(), mode));
            Ok(())
        }

        fn exists(&self, path: &Path) -> bool {
            self.files.borrow().contains_key(path)
        }

        fn mkdir(&self, path: &Path, mode: u32) -> Result<()> {
            self.files
                .borrow_mut()
                .insert(path.into(), (String::new(), mode));
            Ok(())
        }

        fn ask_hidden(&self, prompt: &str) -> Result<String> {
            self.asked.borrow_mut().push(prompt.into());
            self.typed
                .borrow_mut()
                .pop_front()
                .ok_or_else(|| anyhow::anyhow!("nothing typed"))
        }

        fn say(&self, what: &str) {
            self.said.borrow_mut().push(what.into());
        }

        fn step_started(&self, id: &str) {
            self.steps.borrow_mut().push(format!("started {id}"));
        }

        fn step_finished(&self, id: &str, state: &State) {
            self.steps
                .borrow_mut()
                .push(format!("finished {id} {}", json!(state.status)));
        }
    }
}
