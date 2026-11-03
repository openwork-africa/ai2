use super::bridge::{Bridge, Field};
use ::setup::runner::{is_root, Cmd, Ran, Root, Runner};
use ::setup::{inputs_info, InputInfo, State as StepState};
use anyhow::{bail, Context, Result};
use std::cell::RefCell;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Instant;
use zeroize::{Zeroize, Zeroizing};

pub const SUDO_TRIES: usize = 3;
pub const SUDO_REFUSED: &str = "sudo did not take that password. Try it again.";

pub struct Reporting {
    inner: Box<dyn Runner>,
    bridge: Arc<Bridge>,
    live: bool,
    current: RefCell<Option<(String, Instant)>>,
}

fn info(kind: &str) -> InputInfo {
    inputs_info()
        .into_iter()
        .find(|input| input.kind == kind)
        .expect("setup describes every input it asks for")
}

fn fields(info: &InputInfo) -> Vec<Field> {
    info.fields
        .iter()
        .map(|field| Field {
            label: field.label.into(),
            secret: field.secret,
        })
        .collect()
}

fn first(values: Vec<Zeroizing<String>>) -> Zeroizing<String> {
    values.into_iter().next().unwrap_or_default()
}

fn segments(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim_end_matches(['\n', '\r']);
    let last = text.rsplit('\r').find(|part| !part.trim().is_empty())?;
    Some(last.to_string())
}

fn lines(pipe: impl Read, mut each: impl FnMut(String)) {
    let mut reader = BufReader::new(pipe);
    let mut buffer = Vec::new();
    while reader.read_until(b'\n', &mut buffer).unwrap_or(0) > 0 {
        if let Some(line) = segments(&buffer) {
            each(line);
        }
        buffer.clear();
    }
}

impl Reporting {
    pub fn new(inner: Box<dyn Runner>, bridge: Arc<Bridge>, live: bool) -> Self {
        Self {
            inner,
            bridge,
            live,
            current: RefCell::new(None),
        }
    }

    fn step(&self) -> Option<String> {
        self.current.borrow().as_ref().map(|(id, _)| id.clone())
    }

    fn log(&self, stream: &'static str, line: &str) {
        self.bridge.log(self.step().as_deref(), stream, line);
    }

    fn sudo(&self) -> Result<()> {
        if self.inner.run(&Cmd::new("sudo", &["-n", "-v"]))?.ok {
            return Ok(());
        }
        let info = info("sudo");
        let mut refused = None;
        for _ in 0..SUDO_TRIES {
            let mut password = first(self.bridge.ask(
                self.step().as_deref(),
                info.kind,
                info.title,
                info.why,
                fields(&info),
                refused,
            )?);
            password.push('\n');
            let mut cmd = Cmd::new("sudo", &["-S", "-v"]).input(password.as_bytes());
            let ran = self.inner.run(&cmd);
            if let Some(bytes) = cmd.input.as_mut() {
                bytes.zeroize();
            }
            if ran?.ok {
                self.log("say", "sudo is ready for the steps that need root");
                return Ok(());
            }
            refused = Some(SUDO_REFUSED);
        }
        bail!("sudo did not take the password, {SUDO_TRIES} times")
    }

    fn stream(&self, cmd: &Cmd) -> Result<Ran> {
        let mut command = match (cmd.root, is_root()) {
            (Root::No, _) | (_, true) => Command::new(&cmd.program),
            (Root::Yes | Root::IfCached, false) => {
                let mut sudo = Command::new("sudo");
                sudo.args(["-n", &cmd.program]);
                sudo
            }
        };
        command
            .args(&cmd.args)
            .stdin(if cmd.input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Ran::failed()),
            Err(error) => return Err(error).with_context(|| format!("starting {}", cmd.line())),
        };
        if let (Some(bytes), Some(mut stdin)) = (&cmd.input, child.stdin.take()) {
            stdin.write_all(bytes)?;
        }
        let errors = child.stderr.take().map(|pipe| {
            let bridge = self.bridge.clone();
            let step = self.step();
            std::thread::spawn(move || {
                lines(pipe, |line| bridge.log(step.as_deref(), "err", &line))
            })
        });
        let mut out = String::new();
        if let Some(pipe) = child.stdout.take() {
            lines(pipe, |line| {
                self.log("out", &line);
                out.push_str(&line);
                out.push('\n');
            });
        }
        if let Some(thread) = errors {
            let _ = thread.join();
        }
        let status = child.wait()?;
        Ok(Ran {
            ok: status.success(),
            out,
        })
    }
}

impl Runner for Reporting {
    fn run(&self, cmd: &Cmd) -> Result<Ran> {
        if cmd.root == Root::Yes && !is_root() {
            self.sudo()?;
        }
        if cmd.show {
            self.log("cmd", &format!("$ {}", cmd.line()));
            if self.live {
                return self.stream(cmd);
            }
        }
        self.inner.run(cmd)
    }

    fn read(&self, path: &Path) -> Option<String> {
        self.inner.read(path)
    }

    fn write(&self, path: &Path, contents: &str, mode: u32) -> Result<()> {
        self.inner.write(path, contents, mode)
    }

    fn exists(&self, path: &Path) -> bool {
        self.inner.exists(path)
    }

    fn mkdir(&self, path: &Path, mode: u32) -> Result<()> {
        self.inner.mkdir(path, mode)
    }

    fn ask_hidden(&self, prompt: &str) -> Result<String> {
        let title = prompt.trim_end_matches([':', ' ']);
        let label = if title.to_lowercase().contains("passphrase") {
            "Passphrase"
        } else {
            "Answer"
        };
        let value = first(self.bridge.ask(
            self.step().as_deref(),
            "secret",
            title,
            "Setup needs this to carry on. It goes to this step only and is not kept.",
            vec![Field {
                label: label.into(),
                secret: true,
            }],
            None,
        )?);
        Ok(value.to_string())
    }

    fn say(&self, what: &str) {
        self.log("say", what);
    }

    fn step_started(&self, id: &str) {
        *self.current.borrow_mut() = Some((id.into(), Instant::now()));
        self.bridge.started(id);
    }

    fn step_finished(&self, id: &str, state: &StepState) {
        let ms = self
            .current
            .borrow()
            .as_ref()
            .filter(|(current, _)| current == id)
            .map(|(_, at)| at.elapsed().as_millis() as u64);
        self.bridge.finished(id, state, ms);
    }

    fn ask_passphrase(&self, _prompt: &str, _again: &str) -> Result<String> {
        let info = info("passphrase");
        let values = self.bridge.ask(
            self.step().as_deref(),
            info.kind,
            info.title,
            info.why,
            fields(&info),
            None,
        )?;
        match values.as_slice() {
            [one, two] if one == two => Ok(one.to_string()),
            _ => bail!("the two passphrases differ"),
        }
    }
}
