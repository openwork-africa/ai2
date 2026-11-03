use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Problem, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema, utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum RunState {
    Running,
    Reviewing,
    Ended,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema, utoipa::ToSchema))]
pub struct Run {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    pub harness: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub folder: String,
    pub prompt: String,
    pub state: RunState,
    pub started: String,
    pub ended: Option<String>,
    pub exit: Option<i32>,
    pub log: String,
    pub review: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Run {
    pub fn ok(&self) -> bool {
        self.exit == Some(0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema, utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum Stream {
    Out,
    Err,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema, utoipa::ToSchema))]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum RunEvent {
    Waiting {
        folder: String,
    },
    Started {
        run: Run,
    },
    Line {
        stream: Stream,
        text: String,
    },
    Exited {
        exit: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    Review {
        text: String,
    },
    Reviewed {
        hash: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    Done {
        run: Run,
    },
}

impl RunEvent {
    pub fn name(&self) -> &'static str {
        match self {
            RunEvent::Waiting { .. } => "waiting",
            RunEvent::Started { .. } => "started",
            RunEvent::Line { .. } => "line",
            RunEvent::Exited { .. } => "exited",
            RunEvent::Review { .. } => "review",
            RunEvent::Reviewed { .. } => "reviewed",
            RunEvent::Done { .. } => "done",
        }
    }

    pub fn said(&self) -> String {
        match self {
            RunEvent::Waiting { folder } => {
                format!("Another run is working in {folder}; this one starts when it ends.")
            }
            RunEvent::Started { run } => format!("{} started in {}", run.harness, run.folder),
            RunEvent::Line { text, .. } | RunEvent::Review { text } => text.clone(),
            RunEvent::Exited {
                error: Some(error), ..
            } => error.clone(),
            RunEvent::Exited { exit, .. } => match exit {
                Some(code) => format!("It ended with {code}."),
                None => "It was stopped.".into(),
            },
            RunEvent::Reviewed {
                hash: Some(hash), ..
            } => format!("Reviewed: {hash}"),
            RunEvent::Reviewed { error, .. } => error
                .clone()
                .unwrap_or_else(|| "The review was left out.".into()),
            RunEvent::Done { run } => format!("Run {} is done.", run.id),
        }
    }
}

pub fn valid_id(id: &str) -> bool {
    id.len() == 16
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn new_id() -> String {
    let mut random = [0u8; 2];
    let _ = std::fs::File::open("/dev/urandom").and_then(|mut file| file.read_exact(&mut random));
    let ms = crate::time::now_ms() & 0xffff_ffff_ffff;
    format!("{ms:012x}{:02x}{:02x}", random[0], random[1])
}

#[derive(Clone, Debug)]
pub struct Runs {
    dir: PathBuf,
}

impl Runs {
    pub fn new(ai2_home: &Path) -> Self {
        Self {
            dir: ai2_home.join("runs"),
        }
    }

    pub fn root(&self) -> &Path {
        &self.dir
    }

    fn folder(&self, id: &str) -> Result<PathBuf> {
        if !valid_id(id) {
            return Err(Problem::no_run(id));
        }
        Ok(self.dir.join(id))
    }

    pub fn log(&self, id: &str) -> Result<PathBuf> {
        Ok(self.folder(id)?.join("output.log"))
    }

    fn events(&self, id: &str) -> Result<PathBuf> {
        Ok(self.folder(id)?.join("events.jsonl"))
    }

    pub fn save(&self, run: &Run) -> Result<()> {
        let dir = self.folder(&run.id)?;
        std::fs::create_dir_all(&dir)?;
        let fresh = dir.join("run.json.new");
        std::fs::write(&fresh, serde_json::to_string_pretty(run)? + "\n")?;
        std::fs::rename(&fresh, dir.join("run.json"))?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<Run> {
        let file = self.folder(id)?.join("run.json");
        let text = std::fs::read_to_string(file).map_err(|_| Problem::no_run(id))?;
        Ok(serde_json::from_str(&text)?)
    }

    pub fn list(&self) -> Vec<Run> {
        let mut all: Vec<Run> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|id| valid_id(id))
            .filter_map(|id| self.get(&id).ok())
            .collect();
        all.sort_by(|a, b| b.id.cmp(&a.id));
        all
    }

    pub fn append(&self, id: &str, event: &RunEvent) -> Result<()> {
        let mut line = serde_json::to_vec(event)?;
        line.push(b'\n');
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.events(id)?)?
            .write_all(&line)?;
        Ok(())
    }

    pub fn follow(&self, id: &str, from: usize) -> Result<Follow> {
        self.get(id)?;
        Ok(Follow {
            file: self.events(id)?,
            offset: 0,
            index: 0,
            from,
            pending: Vec::new(),
            done: false,
        })
    }
}

pub struct Follow {
    file: PathBuf,
    offset: u64,
    index: usize,
    from: usize,
    pending: Vec<u8>,
    done: bool,
}

impl Follow {
    pub fn read(&mut self) -> Vec<(usize, RunEvent)> {
        let mut found = Vec::new();
        let Ok(mut file) = std::fs::File::open(&self.file) else {
            return found;
        };
        let mut fresh = Vec::new();
        if file.seek(SeekFrom::Start(self.offset)).is_err() || file.read_to_end(&mut fresh).is_err()
        {
            return found;
        }
        self.offset += fresh.len() as u64;
        self.pending.extend(fresh);
        let Some(cut) = self.pending.iter().rposition(|&b| b == b'\n') else {
            return found;
        };
        let whole: Vec<u8> = self.pending.drain(..=cut).collect();
        for line in whole.split(|&b| b == b'\n').filter(|line| !line.is_empty()) {
            let Ok(event) = serde_json::from_slice::<RunEvent>(line) else {
                continue;
            };
            let at = self.index;
            self.index += 1;
            if matches!(event, RunEvent::Done { .. }) {
                self.done = true;
            }
            if at >= self.from {
                found.push((at, event));
            }
        }
        found
    }

    pub fn done(&self) -> bool {
        self.done
    }
}

pub struct FolderLock(std::fs::File);

impl FolderLock {
    pub fn take(
        root: &Path,
        folder: &Path,
        waiting: &mut dyn FnMut(),
    ) -> anyhow::Result<FolderLock> {
        use std::os::fd::AsRawFd;
        let locks = root.join("locks");
        std::fs::create_dir_all(&locks)?;
        let name = format!("{:016x}.lock", fnv(folder.to_string_lossy().as_bytes()));
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(locks.join(name))?;
        let fd = file.as_raw_fd();
        if unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            waiting();
            if unsafe { libc::flock(fd, libc::LOCK_EX) } != 0 {
                anyhow::bail!("could not wait for {}", folder.display());
            }
        }
        Ok(FolderLock(file))
    }
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}
