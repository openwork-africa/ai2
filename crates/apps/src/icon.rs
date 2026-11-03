use crate::error::{Problem, Result};
use crate::model::IconUpload;
use base64::Engine;
use regex::Regex;
use std::sync::LazyLock;

pub const ICON_LIMIT: usize = 512 * 1024;

pub const TYPES: [(&str, &str); 5] = [
    ("image/png", "png"),
    ("image/svg+xml", "svg"),
    ("image/x-icon", "ico"),
    ("image/webp", "webp"),
    ("image/jpeg", "jpg"),
];

static HANDLER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\son[a-z]+\s*=").unwrap());
static EXTERNAL_URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)url\(\s*["']?\s*(?:[a-z]+:|//)|@import"#).unwrap());
static HREF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)\b(?:xlink:)?href\s*=\s*["']?\s*([^"'\s>]*)"#).unwrap());

pub fn extension(content_type: &str) -> &'static str {
    TYPES
        .iter()
        .find(|(t, _)| *t == content_type)
        .map(|(_, ext)| *ext)
        .unwrap_or("bin")
}

pub fn decode(upload: &IconUpload) -> Result<(String, Vec<u8>)> {
    let bytes = match (&upload.base64, &upload.path) {
        (Some(text), _) => base64::engine::general_purpose::STANDARD
            .decode(text.trim())
            .map_err(|error| {
                Problem::new(
                    400,
                    "icon_invalid",
                    format!("The icon is not base64: {error}"),
                )
            })?,
        (None, Some(path)) => std::fs::read(path).map_err(|error| {
            Problem::new(
                400,
                "icon_invalid",
                format!("Could not read the icon {path}: {error}"),
            )
        })?,
        (None, None) => {
            return Err(Problem::new(
                422,
                "icon_required",
                "Send the app's favicon: a png, svg, ico, webp or jpg image.",
            ))
        }
    };
    check(upload.content_type.as_deref(), bytes)
}

pub fn check(declared: Option<&str>, bytes: Vec<u8>) -> Result<(String, Vec<u8>)> {
    if bytes.len() > ICON_LIMIT {
        return Err(Problem::new(
            413,
            "icon_too_large",
            format!(
                "The icon is {} KiB; the limit is {} KiB.",
                bytes.len() / 1024,
                ICON_LIMIT / 1024
            ),
        ));
    }
    let Some(sniffed) = sniff(&bytes) else {
        return Err(Problem::new(
            400,
            "icon_type",
            "The icon must be a png, svg, ico, webp or jpg image.",
        ));
    };
    if let Some(declared) = declared {
        let declared = if declared == "image/vnd.microsoft.icon" {
            "image/x-icon"
        } else {
            declared
        };
        if declared != sniffed {
            return Err(Problem::new(
                400,
                "icon_type",
                format!("The icon says it is {declared} but it is {sniffed}."),
            ));
        }
    }
    if sniffed == "image/svg+xml" {
        unsafe_svg(&String::from_utf8_lossy(&bytes))?;
    }
    Ok((sniffed.to_string(), bytes))
}

fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("image/jpeg");
    }
    if bytes.starts_with(&[0, 0, 1, 0]) {
        return Some("image/x-icon");
    }
    if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    let text = std::str::from_utf8(bytes).ok()?;
    text.to_lowercase()
        .contains("<svg")
        .then_some("image/svg+xml")
}

fn unsafe_svg(text: &str) -> Result<()> {
    let lower = text.to_lowercase();
    let refuse = |why: &str| {
        Err(Problem::new(
            400,
            "icon_unsafe",
            format!("The svg icon {why}; send one without it."),
        ))
    };
    if lower.contains("<script") {
        return refuse("has a script");
    }
    if lower.contains("<foreignobject") {
        return refuse("has a foreignObject");
    }
    if HANDLER.is_match(text) {
        return refuse("has an event handler");
    }
    if HREF.captures_iter(text).any(|c| {
        let target = c[1].to_lowercase();
        !(target.is_empty() || target.starts_with('#') || target.starts_with("data:image/"))
    }) {
        return refuse("links outside itself");
    }
    if EXTERNAL_URL.is_match(text) {
        return refuse("loads something from outside");
    }
    if lower.contains("javascript:") {
        return refuse("has a javascript: link");
    }
    Ok(())
}
