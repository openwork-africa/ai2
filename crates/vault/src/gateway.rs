use crate::approvals::Pending;
use crate::crypto;
use crate::daemon::Vault;
use crate::http::{self, Request};
use crate::inject::Inject;
use crate::policy;
use crate::scrub::Scrubber;
use crate::store::{AuditLine, KeyRecord, Mode, Store, TokenRecord, Vetted};
use crate::time;
use crate::vet::Vetting;
use anyhow::{anyhow, bail, Result};
use serde_json::json;
use std::io::{Read, Write};
use std::sync::Arc;

const DROPPED: [&str; 13] = [
    "host",
    "authorization",
    "connection",
    "keep-alive",
    "proxy-connection",
    "proxy-authorization",
    "transfer-encoding",
    "content-length",
    "te",
    "trailer",
    "upgrade",
    "accept-encoding",
    "expect",
];

const DROPPED_BACK: [&str; 5] = [
    "connection",
    "keep-alive",
    "transfer-encoding",
    "content-length",
    "trailer",
];

const INTERNAL: &str = "internal";

struct Denial {
    reason: &'static str,
    intent: Option<String>,
}

fn deny(reason: &'static str) -> Denial {
    Denial {
        reason,
        intent: None,
    }
}

struct Allowed {
    token: TokenRecord,
    key: KeyRecord,
    url: String,
}

struct Call<'a> {
    sandbox: &'a str,
    method: &'a str,
    token: Option<&'a str>,
    key: Option<&'a str>,
    url: Option<&'a str>,
    request: Option<&'a str>,
}

pub fn handle(
    vault: &Arc<Vault>,
    sandbox: &str,
    peer_allowed: bool,
    request: Request,
    out: &mut impl Write,
) {
    let asked = match request
        .header("x-vault-request")
        .map(policy::decode_percent)
        .transpose()
    {
        Ok(asked) => asked,
        Err(_) => {
            let _ = http::write_error(
                out,
                400,
                "bad_request",
                "X-Vault-Request is not percent-encoded UTF-8",
            );
            return;
        }
    };
    let token = bearer(&request).map(crypto::hash);
    let (key, url) = target(&request.target, vault.config.allow_http);
    let call = Call {
        sandbox,
        method: &request.method,
        token: token.as_deref(),
        key: key.as_deref(),
        url: url.as_deref(),
        request: asked.as_deref(),
    };
    let decided = if peer_allowed {
        decide(vault, &call)
    } else {
        Err(deny("wrong_sandbox"))
    };
    let (token_id, status, reason) = match decided {
        Err(denial) if denial.reason == INTERNAL => {
            let status = refuse(out, 500, INTERNAL, "the token's use could not be saved");
            (known_id(vault, call.token), status, Some(INTERNAL))
        }
        Err(denial) => {
            let _ = http::write_json(
                out,
                403,
                &json!({ "error": "denied", "reason": denial.reason, "intent": denial.intent }),
            );
            (known_id(vault, call.token), 403, Some(denial.reason))
        }
        Ok(allowed) => {
            let status = forward(vault, &allowed, &request, out);
            (Some(allowed.token.id.clone()), status, None)
        }
    };
    let _ = vault.store().audit(&AuditLine {
        time: time::stamp(),
        token: token_id.as_deref(),
        sandbox,
        key: call.key,
        method: call.method,
        url: call.url,
        request: call.request,
        decision: if reason.is_some() { "deny" } else { "allow" },
        status,
        reason,
    });
}

fn bearer(request: &Request) -> Option<&str> {
    let value = request.header("authorization")?;
    let (scheme, token) = value.split_once(' ')?;
    scheme
        .eq_ignore_ascii_case("bearer")
        .then(|| token.trim())
        .filter(|t| !t.is_empty())
}

fn target(path: &str, allow_http: bool) -> (Option<String>, Option<String>) {
    let Some(rest) = path.strip_prefix('/') else {
        return (None, None);
    };
    let (key, rest) = rest.split_once('/').unwrap_or((rest, ""));
    let key = (!key.is_empty()).then(|| key.to_string());
    let host_end = rest.find(['/', '?']).unwrap_or(rest.len());
    if host_end == 0 {
        return (key, None);
    }
    let (host, tail) = rest.split_at(host_end);
    let tail = if tail.starts_with('/') {
        tail.to_string()
    } else {
        format!("/{tail}")
    };
    let scheme = if allow_http { "http" } else { "https" };
    (key, Some(format!("{scheme}://{host}{tail}")))
}

fn known_id(vault: &Vault, hash: Option<&str>) -> Option<String> {
    let hash = hash?;
    vault.store().token_by_hash(hash).map(|t| t.id.clone())
}

fn lookup(store: &Store, call: &Call) -> Result<(TokenRecord, KeyRecord), Denial> {
    if store.sandbox(call.sandbox).is_none() {
        return Err(deny("wrong_sandbox"));
    }
    let token = call
        .token
        .and_then(|hash| store.token_by_hash(hash))
        .cloned()
        .ok_or_else(|| deny("token_unknown"))?;
    let with_intent = |reason| Denial {
        reason,
        intent: token.intent.clone(),
    };
    if token.expires <= time::now() {
        return Err(with_intent("token_expired"));
    }
    if token.uses == Some(0) {
        return Err(with_intent("token_used_up"));
    }
    if token.sandbox != call.sandbox {
        return Err(with_intent("wrong_sandbox"));
    }
    let key = call
        .key
        .filter(|key| token.keys.iter().any(|k| k == key))
        .and_then(|key| store.key(key))
        .cloned()
        .ok_or_else(|| with_intent("key_not_allowed"))?;
    Ok((token, key))
}

fn decide(vault: &Vault, call: &Call) -> Result<Allowed, Denial> {
    let (token, key) = lookup(&vault.store(), call)?;
    let with_intent = |reason| Denial {
        reason,
        intent: token.intent.clone(),
    };
    let method_ok = token.methods.iter().any(|m| m == call.method);
    if !method_ok && token.mode == Mode::Agent {
        return Err(with_intent("method_not_allowed"));
    }
    let url_text = call.url.ok_or_else(|| with_intent("url_not_allowed"))?;
    let allow_http = vault.config.allow_http;
    let url = policy::parse(url_text, allow_http).map_err(|_| with_intent("url_not_allowed"))?;
    match token.mode {
        Mode::App => app_scope(vault, &token, &key, call, (url_text, &url), method_ok),
        Mode::Agent => agent_scope(vault, &token, &key, call, (url_text, &url)),
    }
    .map_err(with_intent)?;
    use_once(vault, &token.id).map_err(with_intent)?;
    Ok(Allowed {
        token,
        key,
        url: url_text.to_string(),
    })
}

fn app_scope(
    vault: &Vault,
    token: &TokenRecord,
    key: &KeyRecord,
    call: &Call,
    (url_text, url): (&str, &policy::Url),
    method_ok: bool,
) -> Result<(), &'static str> {
    let prefixes = if token.url_prefixes.is_empty() {
        &key.approved_urls
    } else {
        &token.url_prefixes
    };
    if method_ok && policy::allowed(url, prefixes, vault.config.allow_http) {
        return Ok(());
    }
    let line = format!(
        "app token {} wants {} {} with key {}",
        token.id,
        call.method,
        url_text
            .split_once("://")
            .map_or(url_text, |(_, rest)| rest),
        key.name
    );
    approve(vault, token, key, call, url_text, line)
}

fn agent_scope(
    vault: &Vault,
    token: &TokenRecord,
    key: &KeyRecord,
    call: &Call,
    (url_text, url): (&str, &policy::Url),
) -> Result<(), &'static str> {
    if !policy::allowed(url, &key.approved_urls, vault.config.allow_http) {
        return Err("url_not_allowed");
    }
    let intent = token.intent.clone().unwrap_or_default();
    vet(vault, key, &intent, call, url_text, url)?;
    if !policy::is_write(call.method) {
        return Ok(());
    }
    let line = format!(
        "agent token {} wants {} {} with key {}, saying: \"{}\" (intent: \"{intent}\")",
        token.id,
        call.method,
        url_text,
        key.name,
        call.request.unwrap_or_default()
    );
    approve(vault, token, key, call, url_text, line)
}

fn vet(
    vault: &Vault,
    key: &KeyRecord,
    intent: &str,
    call: &Call,
    url_text: &str,
    url: &policy::Url,
) -> Result<(), &'static str> {
    let allow_http = vault.config.allow_http;
    let cached: Vec<String> = vault
        .store()
        .vetted
        .iter()
        .filter(|v| v.key == key.name && v.intent == intent)
        .map(|v| v.prefix.clone())
        .collect();
    if policy::allowed(url, &cached, allow_http) {
        return Ok(());
    }
    let vetting = Vetting {
        key: key.info(),
        intent: intent.to_string(),
        method: call.method.to_string(),
        url: url_text.to_string(),
        request: call.request.unwrap_or_default().to_string(),
    };
    let verdict = vault
        .vetter()
        .vet(&vetting)
        .map_err(|_| "vetting_unavailable")?;
    if !verdict.allow {
        return Err("vetting_denied");
    }
    if !policy::accepted_prefix(&verdict.prefix, url_text, allow_http) {
        return Err("vetting_unavailable");
    }
    let mut store = vault.store();
    store.vetted.push(Vetted {
        key: key.name.clone(),
        intent: intent.to_string(),
        prefix: verdict.prefix,
        why: verdict.why,
        created: time::stamp(),
    });
    let _ = store.save("vetted", &store.vetted);
    Ok(())
}

fn approve(
    vault: &Vault,
    token: &TokenRecord,
    key: &KeyRecord,
    call: &Call,
    url_text: &str,
    line: String,
) -> Result<(), &'static str> {
    let pending = Pending {
        id: crypto::id("call_").map_err(|_| "approval_denied")?,
        token: token.id.clone(),
        sandbox: call.sandbox.to_string(),
        key: key.name.clone(),
        method: call.method.to_string(),
        url: url_text.to_string(),
        intent: token.intent.clone().unwrap_or_default(),
        request: call.request.unwrap_or_default().to_string(),
        line,
        created: time::stamp(),
    };
    match vault.approvals.ask(pending, vault.config.approval_timeout) {
        Some(true) => Ok(()),
        Some(false) => Err("approval_denied"),
        None => Err("approval_timeout"),
    }
}

fn use_once(vault: &Vault, id: &str) -> Result<(), &'static str> {
    let mut store = vault.store();
    let Some(token) = store.tokens.iter_mut().find(|t| t.id == id) else {
        return Err("token_unknown");
    };
    match token.uses {
        Some(0) => Err("token_used_up"),
        Some(left) => {
            token.uses = Some(left - 1);
            store.save("tokens", &store.tokens).map_err(|_| INTERNAL)
        }
        None => Ok(()),
    }
}

fn upstream_headers(request: &Request) -> Vec<(String, String)> {
    request
        .headers
        .iter()
        .filter(|(name, _)| {
            let lower = name.to_ascii_lowercase();
            !lower.starts_with("x-vault-") && !DROPPED.contains(&lower.as_str())
        })
        .cloned()
        .collect()
}

fn forward(vault: &Vault, allowed: &Allowed, request: &Request, out: &mut impl Write) -> u16 {
    let secret = match vault.secret(&allowed.key.name) {
        Ok(secret) => secret,
        Err(_) => return refuse(out, 500, "internal", "the key's secret could not be opened"),
    };
    let scrubber = Scrubber::new(&secret);
    match send(vault, allowed, request, &secret) {
        Ok(response) => stream(response, request, scrubber, out),
        Err(error) => {
            let message = scrubber.text(&format!("{error:#}"));
            refuse(out, 502, "upstream", &message)
        }
    }
}

fn refuse(out: &mut impl Write, status: u16, code: &str, message: &str) -> u16 {
    let _ = http::write_error(out, status, code, message);
    status
}

fn send(
    vault: &Vault,
    allowed: &Allowed,
    request: &Request,
    secret: &str,
) -> Result<ureq::http::Response<ureq::Body>> {
    let mut headers = upstream_headers(request);
    let inject = Inject::parse(&allowed.key.inject)?;
    let url = inject.apply(secret, &allowed.url, &mut headers)?;
    let mut builder = ureq::http::Request::builder()
        .method(request.method.as_str())
        .uri(url.as_str());
    for (name, value) in &headers {
        builder = builder.header(name.as_str(), value.as_str());
    }
    let sent = if request.body.is_empty() {
        vault.agent.run(builder.body(())?)
    } else {
        vault.agent.run(builder.body(request.body.clone())?)
    };
    sent.map_err(|error| anyhow!("{error}"))
}

fn stream(
    response: ureq::http::Response<ureq::Body>,
    request: &Request,
    mut scrubber: Scrubber,
    out: &mut impl Write,
) -> u16 {
    let status = response.status().as_u16();
    let (parts, body) = response.into_parts();
    let mut headers = Vec::new();
    for (name, value) in &parts.headers {
        let lower = name.as_str();
        if DROPPED_BACK.contains(&lower) {
            continue;
        }
        if lower == "content-encoding" && value.as_bytes() != b"identity" {
            return refuse(
                out,
                502,
                "upstream",
                "the upstream answer is encoded in a way the vault cannot read",
            );
        }
        let text = String::from_utf8_lossy(value.as_bytes());
        headers.push((name.to_string(), scrubber.text(&text)));
    }
    headers.push(("Connection".into(), "close".into()));
    let bodyless = request.method == "HEAD" || status == 204 || status == 304 || status < 200;
    if bodyless {
        headers.push(("Content-Length".into(), "0".into()));
        let _ = http::write_head(out, status, &headers);
        return status;
    }
    headers.push(("Transfer-Encoding".into(), "chunked".into()));
    if http::write_head(out, status, &headers).is_err() {
        return status;
    }
    let _ = pipe(body.into_reader(), &mut scrubber, out);
    status
}

fn pipe(mut reader: impl Read, scrubber: &mut Scrubber, out: &mut impl Write) -> Result<()> {
    let mut buf = vec![0u8; 16 * 1024];
    loop {
        let read = match reader.read(&mut buf) {
            Ok(read) => read,
            Err(_) => {
                http::write_chunk(out, &scrubber.finish())?;
                bail!("the upstream answer broke off");
            }
        };
        if read == 0 {
            http::write_chunk(out, &scrubber.finish())?;
            http::end_chunks(out)?;
            return Ok(());
        }
        http::write_chunk(out, &scrubber.push(&buf[..read]))?;
    }
}

#[cfg(test)]
mod tests {
    use super::target;

    #[test]
    fn the_path_names_the_key_and_where_the_call_goes() {
        let pair = |key: &str, url: &str| (Some(key.to_string()), Some(url.to_string()));
        assert_eq!(
            target("/stripe/api.stripe.com/v1/charges?limit=3", false),
            pair("stripe", "https://api.stripe.com/v1/charges?limit=3")
        );
        assert_eq!(
            target("/k/127.0.0.1:9/x", true),
            pair("k", "http://127.0.0.1:9/x")
        );
        assert_eq!(
            target("/k/h.example?q=1", false),
            pair("k", "https://h.example/?q=1")
        );
        assert_eq!(target("/k", false), (Some("k".into()), None));
        assert_eq!(target("/", false), (None, None));
    }
}
