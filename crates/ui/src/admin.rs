use anyhow::{Context, Result};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;
use vault::http::{self, Response};

const WAIT: Duration = Duration::from_secs(180);

pub fn send(
    socket: &Path,
    method: &str,
    target: &str,
    body: &[u8],
    passphrase: Option<&str>,
) -> Result<Response> {
    let mut stream = UnixStream::connect(socket)
        .with_context(|| format!("connecting to {}", socket.display()))?;
    stream.set_read_timeout(Some(WAIT))?;
    stream.set_write_timeout(Some(WAIT))?;
    let mut headers = vec![("Accept", "application/json")];
    if !body.is_empty() {
        headers.push(("Content-Type", "application/json"));
    }
    if let Some(passphrase) = passphrase {
        headers.push(("X-Vault-Passphrase", passphrase));
    }
    http::call(&mut stream, method, target, &headers, body)
}
