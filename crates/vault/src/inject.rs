use crate::policy::encode_percent;
use anyhow::{bail, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use zeroize::Zeroizing;

pub const DEFAULT: &str = "bearer";

#[derive(Debug, PartialEq)]
pub enum Inject {
    Header { name: String, template: String },
    Basic,
    Query(String),
}

impl Inject {
    pub fn parse(text: &str) -> Result<Inject> {
        let text = text.trim();
        if text == "bearer" {
            return Ok(Inject::Header {
                name: "Authorization".into(),
                template: "Bearer {secret}".into(),
            });
        }
        if text == "basic" {
            return Ok(Inject::Basic);
        }
        if let Some(param) = text.strip_prefix("query:") {
            if param.is_empty() || encode_percent(param) != param {
                bail!("query:{param} needs a plain parameter name");
            }
            return Ok(Inject::Query(param.into()));
        }
        if let Some(name) = text.strip_prefix("header:") {
            return header(name, "{secret}");
        }
        match text.split_once(':') {
            Some((name, template)) => header(name, template.trim_start()),
            None => bail!(
                "{text} is not an inject template (bearer, basic, query:<param>, header:<Name> or \"Name: ...{{secret}}...\")"
            ),
        }
    }

    pub fn apply(
        &self,
        secret: &str,
        url: &str,
        headers: &mut Vec<(String, String)>,
    ) -> Result<Zeroizing<String>> {
        if secret.chars().any(|c| c == '\r' || c == '\n' || c == '\0') {
            bail!("the secret cannot go in a request: it has a line break");
        }
        let (name, value) = match self {
            Inject::Query(param) => return Ok(with_query(url, param, secret)),
            Inject::Basic => (
                "Authorization".to_string(),
                Zeroizing::new(format!("Basic {}", STANDARD.encode(secret))),
            ),
            Inject::Header { name, template } => (
                name.clone(),
                Zeroizing::new(template.replace("{secret}", secret)),
            ),
        };
        headers.retain(|(n, _)| !n.eq_ignore_ascii_case(&name));
        headers.push((name, value.to_string()));
        Ok(Zeroizing::new(url.to_string()))
    }
}

fn header(name: &str, template: &str) -> Result<Inject> {
    let name = name.trim();
    if name.is_empty() || !name.bytes().all(is_token_byte) {
        bail!("{name} is not a header name");
    }
    if !template.contains("{secret}") || template.contains(['\r', '\n']) {
        bail!("the inject template for {name} needs {{secret}} and no line breaks");
    }
    Ok(Inject::Header {
        name: name.into(),
        template: template.into(),
    })
}

fn is_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}

fn with_query(url: &str, param: &str, secret: &str) -> Zeroizing<String> {
    let (base, query) = url.split_once('?').unwrap_or((url, ""));
    let mut kept: Vec<&str> = query
        .split('&')
        .filter(|pair| !pair.is_empty() && pair.split('=').next() != Some(param))
        .collect();
    let pair = Zeroizing::new(format!("{param}={}", encode_percent(secret)));
    kept.push(&pair);
    Zeroizing::new(format!("{base}?{}", kept.join("&")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn applied(template: &str) -> (String, Vec<(String, String)>) {
        let mut headers = vec![("authorization".to_string(), "Bearer fake".to_string())];
        let url = Inject::parse(template)
            .unwrap()
            .apply("u:p w", "https://h.io/x?key=agent&a=1", &mut headers)
            .unwrap();
        (url.to_string(), headers)
    }

    #[test]
    fn each_template_puts_the_secret_where_it_says() {
        let (url, headers) = applied("bearer");
        assert_eq!(url, "https://h.io/x?key=agent&a=1");
        assert_eq!(
            headers,
            [("Authorization".to_string(), "Bearer u:p w".to_string())]
        );
        let (_, headers) = applied("basic");
        assert_eq!(headers[0].1, format!("Basic {}", STANDARD.encode("u:p w")));
        let (url, _) = applied("query:key");
        assert_eq!(url, "https://h.io/x?a=1&key=u%3Ap%20w");
        let (_, headers) = applied("header:X-Api-Key");
        assert_eq!(headers[1], ("X-Api-Key".into(), "u:p w".into()));
        let (_, headers) = applied("X-Token: tok={secret};");
        assert_eq!(headers[1], ("X-Token".into(), "tok=u:p w;".into()));
    }

    #[test]
    fn bad_templates_are_refused() {
        for bad in [
            "nothing",
            "X-Token: plain",
            "Bad Name: {secret}",
            "query:",
            "query:a b",
        ] {
            assert!(Inject::parse(bad).is_err(), "{bad}");
        }
        let mut headers = Vec::new();
        assert!(Inject::parse("bearer")
            .unwrap()
            .apply("a\r\nb", "https://h.io/", &mut headers)
            .is_err());
    }
}
