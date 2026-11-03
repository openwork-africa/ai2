use crate::model::{LogLine, Stream};
use crate::store::LOG_LIMIT;
use crate::time;
use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tokio::sync::broadcast;

pub const RING: usize = 2_000;

pub struct Logs {
    ring: Mutex<(u64, VecDeque<(u64, LogLine)>)>,
    tx: broadcast::Sender<(u64, LogLine)>,
}

impl Default for Logs {
    fn default() -> Logs {
        Logs {
            ring: Mutex::new((0, VecDeque::with_capacity(RING))),
            tx: broadcast::channel(1024).0,
        }
    }
}

impl Logs {
    pub fn push(&self, stream: Stream, line: String) -> u64 {
        let line = LogLine {
            t: Some(time::now_ms()),
            stream,
            line,
        };
        let mut ring = self.ring.lock().unwrap_or_else(|e| e.into_inner());
        ring.0 += 1;
        let id = ring.0;
        if ring.1.len() == RING {
            ring.1.pop_front();
        }
        ring.1.push_back((id, line.clone()));
        let _ = self.tx.send((id, line));
        id
    }

    pub fn since(&self, after: Option<u64>) -> Vec<(u64, LogLine)> {
        let ring = self.ring.lock().unwrap_or_else(|e| e.into_inner());
        ring.1
            .iter()
            .filter(|(id, _)| after.is_none_or(|after| *id > after))
            .cloned()
            .collect()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<(u64, LogLine)> {
        self.tx.subscribe()
    }
}

pub struct Sink {
    path: PathBuf,
    file: File,
}

impl Sink {
    pub fn new(path: &Path, file: File) -> Sink {
        Sink {
            path: path.to_path_buf(),
            file,
        }
    }

    pub fn write(&mut self, line: &str) {
        let _ = writeln!(self.file, "{line}");
        if self.file.metadata().map(|m| m.len()).unwrap_or(0) >= LOG_LIMIT
            && std::fs::rename(&self.path, self.path.with_extension("log.1")).is_ok()
        {
            if let Ok(file) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
            {
                self.file = file;
            }
        }
    }
}

pub struct Follow {
    path: PathBuf,
    offset: u64,
    partial: Vec<u8>,
}

impl Follow {
    pub fn from_end(path: &Path) -> Follow {
        Follow {
            path: path.to_path_buf(),
            offset: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
            partial: Vec::new(),
        }
    }

    pub fn read(&mut self) -> Vec<String> {
        let Ok(mut file) = File::open(&self.path) else {
            return Vec::new();
        };
        let len = file.metadata().map(|m| m.len()).unwrap_or(0);
        if len < self.offset {
            self.offset = 0;
            self.partial.clear();
        }
        if len == self.offset || file.seek(SeekFrom::Start(self.offset)).is_err() {
            return Vec::new();
        }
        let mut bytes = Vec::new();
        let Ok(read) = file.take(len - self.offset).read_to_end(&mut bytes) else {
            return Vec::new();
        };
        self.offset += read as u64;
        self.partial.extend_from_slice(&bytes);
        let Some(end) = self.partial.iter().rposition(|b| *b == b'\n') else {
            return Vec::new();
        };
        let complete: Vec<u8> = self.partial.drain(..=end).collect();
        String::from_utf8_lossy(&complete)
            .lines()
            .map(str::to_string)
            .collect()
    }
}
