use anyhow::{bail, Result};

const READ: [&str; 3] = ["GET", "HEAD", "OPTIONS"];
const WRITE: [&str; 4] = ["POST", "PUT", "PATCH", "DELETE"];

#[derive(Debug, PartialEq)]
pub struct Url {
    pub scheme: String,
    pub authority: String,
    pub rest: String,
}

pub fn parse(text: &str, allow_http: bool) -> Result<Url> {
    let Some((scheme, after)) = text.split_once("://") else {
        bail!("{text} has no scheme");
    };
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "https" && !(allow_http && scheme == "http") {
        bail!("{text} is not https");
    }
    if text
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || c == '\\' || c == '#')
    {
        bail!("{text} has characters a URL cannot carry");
    }
    let end = after.find(['/', '?']).unwrap_or(after.len());
    let (authority, rest) = after.split_at(end);
    let authority = authority.to_ascii_lowercase();
    if authority.is_empty() || authority.contains('@') || authority.starts_with(':') {
        bail!("{text} has no plain host");
    }
    let default_port = if scheme == "https" { ":443" } else { ":80" };
    let authority = authority
        .strip_suffix(default_port)
        .unwrap_or(&authority)
        .to_string();
    let path = rest.split('?').next().unwrap_or("");
    if path.split('/').any(unsafe_segment) {
        bail!("{text} has a dot or encoded slash in its path");
    }
    Ok(Url {
        scheme,
        authority,
        rest: rest.to_string(),
    })
}

fn unsafe_segment(segment: &str) -> bool {
    let lower = segment.to_ascii_lowercase();
    let decoded = lower.replace("%2e", ".");
    decoded == "." || decoded == ".." || lower.contains("%2f") || lower.contains("%5c")
}

pub fn within(url: &Url, prefix: &Url) -> bool {
    url.scheme == prefix.scheme
        && url.authority == prefix.authority
        && url.rest.starts_with(&prefix.rest)
}

pub fn allowed(url: &Url, prefixes: &[String], allow_http: bool) -> bool {
    prefixes
        .iter()
        .filter_map(|prefix| parse(prefix, allow_http).ok())
        .any(|prefix| within(url, &prefix))
}

pub fn accepted_prefix(prefix: &str, url: &str, allow_http: bool) -> bool {
    match (parse(prefix, allow_http), parse(url, allow_http)) {
        (Ok(prefix), Ok(url)) => within(&url, &prefix),
        _ => false,
    }
}

pub fn methods(spec: &str) -> Result<Vec<String>> {
    let named: Vec<&str> = match spec.trim().to_ascii_lowercase().as_str() {
        "read" => READ.to_vec(),
        "write" => WRITE.to_vec(),
        "all" => READ.iter().chain(WRITE.iter()).copied().collect(),
        _ => {
            let listed: Vec<String> = spec
                .split(',')
                .map(|m| m.trim().to_ascii_uppercase())
                .collect();
            for method in &listed {
                if !READ.contains(&method.as_str()) && !WRITE.contains(&method.as_str()) {
                    bail!("{method} is not a method a token can allow (read, write, all, or GET,POST,...)");
                }
            }
            return Ok(listed);
        }
    };
    Ok(named.into_iter().map(str::to_string).collect())
}

pub fn is_write(method: &str) -> bool {
    !READ.contains(&method)
}

pub fn decode_percent(text: &str) -> Result<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = text.get(i + 1..i + 3).unwrap_or("");
            let Ok(byte) = u8::from_str_radix(hex, 16) else {
                bail!("a bad percent escape");
            };
            out.push(byte);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Ok(String::from_utf8(out)?)
}

pub fn encode_percent(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Url {
        parse(text, false).unwrap()
    }

    #[test]
    fn a_prefix_holds_its_own_host_and_path_only() {
        let prefix = url("https://API.stripe.com:443/v1/");
        assert!(within(&url("https://api.stripe.com/v1/charges"), &prefix));
        assert!(!within(&url("https://api.stripe.com/v2/charges"), &prefix));
        assert!(!within(&url("https://api.stripe.com.evil.io/v1/"), &prefix));
        let host = url("https://api.stripe.com");
        assert!(within(&url("https://api.stripe.com/anything?x=1"), &host));
        assert!(!within(&url("https://api.stripe.com:8443/"), &host));
    }

    #[test]
    fn tricky_urls_are_refused() {
        for bad in [
            "http://api.stripe.com/",
            "https://user@evil.io/",
            "https://api.stripe.com/v1/../admin",
            "https://api.stripe.com/v1/%2E%2e/admin",
            "https://api.stripe.com/v1%2fadmin",
            "https://api.stripe.com/v1#x",
            "https://api.stripe.com/a b",
            "https:///v1",
            "api.stripe.com/v1",
        ] {
            assert!(parse(bad, false).is_err(), "{bad}");
        }
        assert!(parse("http://127.0.0.1:9/", true).is_ok());
    }

    #[test]
    fn the_vetter_prefix_must_hold_the_url() {
        let asked = "https://api.stripe.com/v1/refunds";
        assert!(accepted_prefix("https://api.stripe.com/v1/", asked, false));
        assert!(accepted_prefix("https://api.stripe.com", asked, false));
        assert!(!accepted_prefix("https://", asked, false));
        assert!(!accepted_prefix("https://api.stripe.com/v2/", asked, false));
        assert!(!accepted_prefix("*", asked, false));
    }

    #[test]
    fn method_sets_and_percent_text() {
        assert_eq!(methods("read").unwrap(), ["GET", "HEAD", "OPTIONS"]);
        assert_eq!(methods("all").unwrap().len(), 7);
        assert_eq!(methods("get, post").unwrap(), ["GET", "POST"]);
        assert!(methods("TRACE").is_err());
        assert!(is_write("POST") && !is_write("HEAD"));
        assert_eq!(decode_percent("refund%20%C3%A9 1").unwrap(), "refund é 1");
        assert!(decode_percent("%zz").is_err() && decode_percent("%ff").is_err());
        assert_eq!(encode_percent("a b/é"), "a%20b%2F%C3%A9");
    }
}
