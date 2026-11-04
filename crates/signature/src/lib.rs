use anyhow::{bail, Context, Result};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub const KEY_FILE: &str = "signature.key";
pub const META_NAME: &str = "ai2-signature";

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Manifest {
    pub alg: String,
    pub hash: String,
    pub sig: String,
    pub signed_at: u64,
}

pub struct Signed {
    pub path: PathBuf,
    pub html: String,
    pub manifest: Manifest,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn key(state: &Path) -> Result<[u8; 32]> {
    let path = state.join(KEY_FILE);
    if let Ok(bytes) = std::fs::read(&path) {
        if let Ok(key) = <[u8; 32]>::try_from(bytes) {
            return Ok(key);
        }
    }
    let mut key = [0u8; 32];
    getrandom(key.as_mut())?;
    std::fs::create_dir_all(state).with_context(|| format!("making {}", state.display()))?;
    let temp = state.join(format!(".{KEY_FILE}.{}", std::process::id()));
    let mut out = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)?;
    use std::io::Write;
    out.write_all(&key)?;
    out.sync_all()?;
    std::fs::rename(&temp, &path)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    Ok(key)
}

fn getrandom(into: &mut [u8]) -> Result<()> {
    getrandom::fill(into).map_err(|error| anyhow::anyhow!("no random bytes: {error}"))
}

pub fn strip(html: &str) -> String {
    let mut out = html.to_string();
    let mut from = 0;
    while let Some(found) = out[from..].find("<meta") {
        let at = from + found;
        let Some(found) = out[at..].find('>') else {
            break;
        };
        let end = at + found + 1;
        if out[at..end].contains(META_NAME) {
            out.replace_range(at..end, "");
        } else {
            from = end;
        }
    }
    out
}

pub fn sign(html: &str, secret: &[u8; 32]) -> Manifest {
    let clean = strip(html);
    let hash = hex(&Sha256::digest(clean.as_bytes()));
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("hmac takes any key size");
    mac.update(hash.as_bytes());
    Manifest {
        alg: "HMAC-SHA256".into(),
        hash,
        sig: hex(&mac.finalize().into_bytes()),
        signed_at: now(),
    }
}

pub fn embed(html: &str, manifest: &Manifest) -> Result<String> {
    let text = serde_json::to_string(manifest)?;
    let tag = format!(r#"<meta name="{META_NAME}" content='{text}' />"#);
    if let Some(at) = html.find("<head") {
        let rest = &html[at..];
        let Some(end) = rest.find('>') else {
            bail!("the document's <head> tag never closes");
        };
        let mut out = html.to_string();
        out.insert_str(at + end + 1, &tag);
        return Ok(out);
    }
    Ok(format!("{tag}{html}"))
}

pub fn extract(html: &str) -> Result<(Manifest, String)> {
    let Some(at) = html.find(META_NAME) else {
        bail!("the document carries no {META_NAME} signature");
    };
    let after = &html[at..];
    let quote = after
        .find("content=")
        .and_then(|at| after[at + "content=".len()..].chars().next())
        .filter(|c| *c == '\'' || *c == '"')
        .ok_or_else(|| anyhow::anyhow!("the signature tag has no readable content"))?;
    let start = at + after.find("content=").unwrap() + "content=".len() + 1;
    let rest = &html[start..];
    let Some(end) = rest.find(quote) else {
        bail!("the signature tag never closes");
    };
    let manifest: Manifest = serde_json::from_str(&rest[..end])
        .with_context(|| "the embedded signature is not valid JSON")?;
    let mut clean = html.to_string();
    let tag_start = html[..at].rfind('<').unwrap_or(at);
    let tag_end = start + end + rest[end..].find('>').map(|i| i + 1).unwrap_or(0);
    clean.replace_range(tag_start..tag_end, "");
    Ok((manifest, clean))
}

pub fn verify(html: &str, secret: &[u8; 32]) -> Result<Manifest> {
    let (manifest, clean) = extract(html)?;
    if manifest.alg != "HMAC-SHA256" {
        bail!("ai2 signs with {}, not {}", manifest.alg, "HMAC-SHA256");
    }
    let hash = hex(&Sha256::digest(clean.as_bytes()));
    if hash != manifest.hash {
        bail!("the document changed after it was signed");
    }
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("hmac takes any key size");
    mac.update(hash.as_bytes());
    if hex(&mac.finalize().into_bytes()) != manifest.sig {
        bail!("the signature was not made with this machine's key");
    }
    Ok(manifest)
}

pub fn sign_file(state: &Path, document: &Path) -> Result<Signed> {
    if document.extension().and_then(|ext| ext.to_str()) != Some("html")
        && document.extension().and_then(|ext| ext.to_str()) != Some("htm")
    {
        bail!("{} is not an HTML document", document.display());
    }
    let html = std::fs::read_to_string(document)
        .with_context(|| format!("reading {}", document.display()))?;
    if !html.to_ascii_lowercase().contains("<html") {
        bail!("{} has no HTML in it", document.display());
    }
    let secret = key(state)?;
    let manifest = sign(&html, &secret);
    let signed = embed(&strip(&html), &manifest)?;
    let path = document.with_extension("signed.html");
    std::fs::write(&path, &signed).with_context(|| format!("writing {}", path.display()))?;
    Ok(Signed {
        path,
        html: signed,
        manifest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str =
        "<!doctype html><html><head><title>t</title></head><body><p>hi</p></body></html>";

    fn secret() -> [u8; 32] {
        [9u8; 32]
    }

    #[test]
    fn a_signed_document_verifies_and_a_changed_one_does_not() {
        let manifest = sign(DOC, &secret());
        let signed = embed(DOC, &manifest).unwrap();
        assert_eq!(verify(&signed, &secret()).unwrap(), manifest);
        assert!(verify(&signed.replace("hi", "bye"), &secret()).is_err());
        assert!(verify(&signed, &[8u8; 32]).is_err());
        assert!(verify(DOC, &secret()).is_err());
    }

    #[test]
    fn signing_twice_replaces_the_old_signature() {
        let once = embed(DOC, &sign(DOC, &secret())).unwrap();
        let twice = embed(&strip(&once), &sign(&strip(&once), &secret())).unwrap();
        assert_eq!(twice.matches(META_NAME).count(), 1);
        assert!(verify(&twice, &secret()).is_ok());
    }

    #[test]
    fn a_document_without_a_head_still_signs() {
        let bare = "<html><body><p>hi</p></body></html>";
        let signed = embed(bare, &sign(bare, &secret())).unwrap();
        assert!(signed.starts_with("<meta"));
        assert!(verify(&signed, &secret()).is_ok());
    }

    #[test]
    fn sign_file_writes_a_signed_copy_next_to_the_document() {
        let dir = tempfile::tempdir().unwrap();
        let state = dir.path().join("state");
        let document = dir.path().join("page.html");
        std::fs::write(&document, DOC).unwrap();
        let signed = sign_file(&state, &document).unwrap();
        assert_eq!(signed.path, dir.path().join("page.signed.html"));
        assert!(verify(&signed.html, &key(&state).unwrap()).is_ok());
        let mode = std::fs::metadata(state.join(KEY_FILE))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        std::fs::write(dir.path().join("notes.txt"), DOC).unwrap();
        assert!(sign_file(&state, &dir.path().join("notes.txt")).is_err());
    }
}
