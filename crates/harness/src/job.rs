use std::collections::HashSet;
use std::io::{Read, Write};
use std::os::unix::process::ExitStatusExt;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::terminal::{code_in, in_terminal, links_in, plain_text};

const PROMPT_PAUSE: Duration = Duration::from_millis(250);
const INPUT_CHARS: usize = 2000;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Event {
    Line {
        text: String,
    },
    Link {
        url: String,
    },
    Code {
        code: String,
    },
    Error {
        error: String,
    },
    Done {
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
}

pub struct Ended {
    pub ok: bool,
    pub message: Option<String>,
}

enum Message {
    Output(usize, Vec<u8>),
    Closed(usize),
    Input(String),
}

#[derive(Default)]
struct Reader {
    typed: HashSet<String>,
    links: HashSet<String>,
    codes: HashSet<String>,
    last: String,
}

impl Reader {
    fn handle(&mut self, output: &str, emit: &mut impl FnMut(&Event)) {
        let mut raw = output.to_string();
        for secret in &self.typed {
            raw = raw.replace(secret.as_str(), "••••");
        }
        for url in links_in(&raw) {
            if self.links.insert(url.clone()) {
                emit(&Event::Link { url });
            }
        }
        for text in plain_text(&raw).split('\n').map(str::trim_end) {
            if text.trim().is_empty() || text == self.last {
                continue;
            }
            self.last = text.to_string();
            emit(&Event::Line {
                text: text.to_string(),
            });
            if let Some(code) = code_in(text) {
                if self.codes.insert(code.clone()) {
                    emit(&Event::Code { code });
                }
            }
        }
    }

    fn type_in(
        &mut self,
        text: &str,
        input: &mut Option<ChildStdin>,
        emit: &mut impl FnMut(&Event),
    ) {
        let clean = text.trim();
        if clean.is_empty() || clean.encode_utf16().count() > INPUT_CHARS {
            emit(&Event::Error {
                error: "Paste the code first.".into(),
            });
            return;
        }
        self.typed.insert(clean.to_string());
        if let Some(stdin) = input {
            let _ = stdin
                .write_all(format!("{clean}\r").as_bytes())
                .and_then(|_| stdin.flush());
        }
    }
}

fn pipe(mut stream: impl Read + Send + 'static, at: usize, to: Sender<Message>) {
    thread::spawn(move || {
        let mut buffer = [0u8; 8192];
        while let Ok(read) = stream.read(&mut buffer) {
            if read == 0
                || to
                    .send(Message::Output(at, buffer[..read].to_vec()))
                    .is_err()
            {
                break;
            }
        }
        let _ = to.send(Message::Closed(at));
    });
}

fn start(line: &str, path: &str) -> Result<Child> {
    let args = in_terminal(line, std::env::consts::OS);
    Command::new(&args[0])
        .args(&args[1..])
        .env("PATH", path)
        .env("TERM", "xterm-256color")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("starting {line}"))
}

fn stop(child: &Child) {
    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGTERM);
    }
}

pub fn run(
    line: &str,
    path: &str,
    limit: Duration,
    input: impl Iterator<Item = String> + Send + 'static,
    mut emit: impl FnMut(&Event),
) -> Result<Ended> {
    let mut child = start(line, path)?;
    emit(&Event::Line {
        text: format!("$ {line}"),
    });
    let (to, from) = channel();
    pipe(child.stdout.take().context("no output")?, 0, to.clone());
    pipe(child.stderr.take().context("no errors")?, 1, to.clone());
    thread::spawn(move || {
        for text in input {
            if to.send(Message::Input(text)).is_err() {
                break;
            }
        }
    });

    let mut stdin = child.stdin.take();
    let mut reader = Reader::default();
    let mut pending: [Vec<u8>; 2] = [Vec::new(), Vec::new()];
    let mut paused: [Option<Instant>; 2] = [None, None];
    let mut open = 2;
    let deadline = Instant::now() + limit;
    let mut stopped = false;
    while open > 0 {
        match from.recv_timeout(PROMPT_PAUSE / 5) {
            Ok(Message::Output(at, bytes)) => {
                pending[at].extend(bytes);
                if let Some(cut) = pending[at].iter().rposition(|&byte| byte == b'\n') {
                    let whole: Vec<u8> = pending[at].drain(..=cut).collect();
                    reader.handle(&String::from_utf8_lossy(&whole), &mut emit);
                }
                paused[at] = (!pending[at].is_empty()).then(Instant::now);
            }
            Ok(Message::Closed(at)) => {
                let rest = std::mem::take(&mut pending[at]);
                if !rest.is_empty() {
                    reader.handle(&String::from_utf8_lossy(&rest), &mut emit);
                }
                paused[at] = None;
                open -= 1;
            }
            Ok(Message::Input(text)) => reader.type_in(&text, &mut stdin, &mut emit),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        for at in 0..2 {
            if paused[at].is_some_and(|since| since.elapsed() >= PROMPT_PAUSE) {
                let part = std::mem::take(&mut pending[at]);
                reader.handle(&String::from_utf8_lossy(&part), &mut emit);
                paused[at] = None;
            }
        }
        if !stopped && Instant::now() >= deadline {
            stop(&child);
            stopped = true;
        }
    }
    drop(stdin);
    let status = child.wait()?;
    let ok = status.success() && !stopped;
    let message = (!ok).then(|| match status.signal() {
        _ if stopped => "It took too long and was stopped.".to_string(),
        Some(libc::SIGTERM) => "It took too long and was stopped.".to_string(),
        signal => format!(
            "It ended with an error ({}).",
            status.code().unwrap_or(128 + signal.unwrap_or(0))
        ),
    });
    Ok(Ended { ok, message })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_job_streams_what_it_prints_takes_what_is_typed_and_never_shows_it_back() {
        let line = r#"printf 'Visit https://example.com/device\nEnter WXYZ-9876\n'; printf 'Paste code > '; read answer; echo "got $answer""#;
        let mut events = Vec::new();
        let input = vec!["  ".to_string(), "secret-key-123".to_string()].into_iter();
        let path = std::env::var("PATH").unwrap_or_default();
        let ended = run(line, &path, Duration::from_secs(20), input, |event| {
            events.push(event.clone())
        })
        .unwrap();
        let lines: Vec<&str> = events
            .iter()
            .filter_map(|event| match event {
                Event::Line { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(lines[0], format!("$ {line}"));
        assert!(events.contains(&Event::Link {
            url: "https://example.com/device".into()
        }));
        assert!(events.contains(&Event::Code {
            code: "WXYZ-9876".into()
        }));
        assert!(events.contains(&Event::Error {
            error: "Paste the code first.".into()
        }));
        assert!(
            lines.iter().any(|text| text.contains("got ••••")),
            "{lines:?}"
        );
        assert!(!lines.iter().any(|text| text.contains("secret-key-123")));
        assert!(ended.ok && ended.message.is_none());
    }

    #[test]
    fn a_job_that_takes_too_long_is_stopped() {
        let path = std::env::var("PATH").unwrap_or_default();
        let ended = run(
            "sleep 30",
            &path,
            Duration::from_millis(300),
            std::iter::empty(),
            |_| {},
        )
        .unwrap();
        assert!(!ended.ok);
        assert_eq!(
            ended.message.as_deref(),
            Some("It took too long and was stopped.")
        );
    }

    #[test]
    fn a_failed_job_says_how_it_ended() {
        let path = std::env::var("PATH").unwrap_or_default();
        let ended = run(
            "exit 3",
            &path,
            Duration::from_secs(20),
            std::iter::empty(),
            |_| {},
        )
        .unwrap();
        assert_eq!(
            ended.message.as_deref(),
            Some("It ended with an error (3).")
        );
    }
}
