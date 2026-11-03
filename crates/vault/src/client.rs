use crate::http;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::os::unix::net::UnixStream;
use std::path::Path;

pub fn call(
    admin: &Path,
    method: &str,
    path: &str,
    body: Option<&Value>,
    headers: &[(&str, &str)],
) -> Result<(u16, Value)> {
    let mut stream = UnixStream::connect(admin).map_err(|error| {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            return anyhow!(
                "this user may not reach the vault daemon at {}; after ai2 setup log out and back in so you are in its group",
                admin.display()
            );
        }
        anyhow!(
            "the vault daemon is not running at {} (start it with ai2 vault serve, or run ai2 setup)",
            admin.display()
        )
    })?;
    let body = body
        .map(serde_json::to_vec)
        .transpose()?
        .unwrap_or_default();
    let headers = [&[("Content-Type", "application/json")][..], headers].concat();
    let response = http::call(&mut stream, method, path, &headers, &body)?;
    let value = serde_json::from_slice(&response.body).unwrap_or(Value::Null);
    Ok((response.status, value))
}
