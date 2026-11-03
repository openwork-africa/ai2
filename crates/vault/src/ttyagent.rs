use keys::polkit::{find, process_start, subject, tty_agent_args, PKTTYAGENT};
use std::ffi::OsStr;
use std::io::Read;
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::channel;
use std::time::Duration;

const NOTIFY_FD: i32 = 3;
const REGISTER_LIMIT: Duration = Duration::from_secs(5);

pub struct TtyAgent {
    child: Child,
}

impl Drop for TtyAgent {
    fn drop(&mut self) {
        if let Ok(pid) = libc::pid_t::try_from(self.child.id()) {
            unsafe { libc::kill(pid, libc::SIGTERM) };
        }
        let _ = self.child.wait();
    }
}

fn has_terminal() -> bool {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .is_ok()
}

pub fn start(path: Option<&OsStr>, proc: &Path) -> Option<TtyAgent> {
    if !has_terminal() {
        return None;
    }
    let program = find(PKTTYAGENT, path)?;
    let pid = std::process::id();
    let started = process_start(proc, pid)?;
    let (mut reader, writer) = std::io::pipe().ok()?;
    let out = std::io::stderr().as_fd().try_clone_to_owned().ok()?;
    let fd = writer.as_raw_fd();
    let mut command = Command::new(program);
    command
        .args(tty_agent_args(&subject(pid, started, None), NOTIFY_FD))
        .stdin(Stdio::inherit())
        .stdout(Stdio::from(out))
        .stderr(Stdio::inherit());
    unsafe {
        command.pre_exec(move || {
            let moved = if fd == NOTIFY_FD {
                libc::fcntl(fd, libc::F_SETFD, 0)
            } else {
                libc::dup2(fd, NOTIFY_FD)
            };
            if moved < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let child = command.spawn().ok()?;
    drop(writer);
    let (to, from) = channel();
    std::thread::spawn(move || {
        let mut sink = Vec::new();
        let _ = reader.read_to_end(&mut sink);
        let _ = to.send(());
    });
    let registered = from.recv_timeout(REGISTER_LIMIT).is_ok();
    let mut agent = TtyAgent { child };
    if !registered || !matches!(agent.child.try_wait(), Ok(None)) {
        return None;
    }
    Some(agent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_agent_is_started_without_pkttyagent_or_a_readable_process() {
        let dir = tempfile::tempdir().unwrap();
        assert!(start(Some(dir.path().as_os_str()), dir.path()).is_none());
        std::fs::write(dir.path().join(PKTTYAGENT), "").unwrap();
        assert!(start(Some(dir.path().as_os_str()), dir.path()).is_none());
    }
}
