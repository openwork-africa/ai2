use anyhow::{bail, Context, Result};
use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub fn fresh() -> Result<String> {
    Ok(vault::crypto::hex(&vault::crypto::random::<32>()?))
}

pub fn path(home: &Path) -> PathBuf {
    home.join("ui").join("nonce")
}

pub fn load(home: &Path) -> Result<String> {
    let file = path(home);
    match fs::read_to_string(&file) {
        Ok(text) => {
            let nonce = text.trim().to_string();
            if nonce.len() != 64 || !nonce.bytes().all(|b| b.is_ascii_hexdigit()) {
                bail!("{} is not a nonce; run ai2 ui rotate-nonce", file.display());
            }
            let mode = fs::metadata(&file)?.permissions().mode() & 0o777;
            if mode & 0o077 != 0 {
                fs::set_permissions(&file, fs::Permissions::from_mode(0o600))?;
            }
            Ok(nonce)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => rotate(home),
        Err(error) => Err(error).with_context(|| format!("reading {}", file.display())),
    }
}

pub fn rotate(home: &Path) -> Result<String> {
    let file = path(home);
    let dir = file.parent().unwrap_or(home);
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .with_context(|| format!("creating {}", dir.display()))?;
    let nonce = fresh()?;
    let staged = file.with_extension("new");
    let _ = fs::remove_file(&staged);
    let mut out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&staged)
        .with_context(|| format!("writing {}", staged.display()))?;
    out.write_all(nonce.as_bytes())?;
    out.sync_all()?;
    fs::rename(&staged, &file).with_context(|| format!("writing {}", file.display()))?;
    Ok(nonce)
}

pub fn same(given: &str, nonce: &str) -> bool {
    given.len() == nonce.len()
        && given
            .bytes()
            .zip(nonce.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}
