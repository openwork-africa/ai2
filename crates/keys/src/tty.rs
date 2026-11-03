use anyhow::{bail, Context, Result};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::io::AsRawFd;
use zeroize::Zeroizing;

fn open() -> Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .context("there is no terminal to ask on")
}

fn read_line(tty: &File) -> Result<Zeroizing<String>> {
    let mut line = Zeroizing::new(String::new());
    BufReader::new(tty).read_line(&mut line)?;
    let trimmed = Zeroizing::new(line.trim_end_matches(['\r', '\n']).to_string());
    Ok(trimmed)
}

pub fn ask(prompt: &str) -> Result<String> {
    let mut tty = open()?;
    write!(tty, "{prompt}")?;
    tty.flush()?;
    Ok(read_line(&tty)?.to_string())
}

pub fn hidden(prompt: &str) -> Result<Zeroizing<String>> {
    let mut tty = open()?;
    write!(tty, "{prompt}")?;
    tty.flush()?;
    let fd = tty.as_raw_fd();
    let mut before: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut before) } != 0 {
        bail!("could not read the terminal settings");
    }
    let mut quiet = before;
    quiet.c_lflag &= !libc::ECHO;
    quiet.c_lflag |= libc::ECHONL;
    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &quiet) } != 0 {
        bail!("could not turn off the terminal echo");
    }
    let line = read_line(&tty);
    unsafe { libc::tcsetattr(fd, libc::TCSANOW, &before) };
    line
}

pub fn confirm(prompt: &str) -> Result<bool> {
    let answer = ask(&format!("{prompt} [y/N] "))?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}
