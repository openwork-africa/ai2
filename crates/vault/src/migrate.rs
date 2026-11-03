use crate::cli::{code, exit, Admin};
use crate::policy::encode_percent;
use anyhow::Result;
use base64::Engine;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

const BIGGEST: u64 = 1 << 20;

const SHELL_FILES: [&str; 7] = [
    ".zshrc",
    ".zshenv",
    ".zprofile",
    ".bashrc",
    ".bash_profile",
    ".bash_login",
    ".profile",
];

const DEPENDENCY_FOLDERS: [&str; 9] = [
    "node_modules",
    "vendor",
    "target",
    "venv",
    "__pycache__",
    "bower_components",
    "Pods",
    "site-packages",
    "jspm_packages",
];

const EXAMPLES: [&str; 5] = ["example", "sample", "template", "dist", "defaults"];

const PREFIXES: [&str; 25] = [
    "sk-ant-",
    "sk-proj-",
    "sk-or-",
    "sk-",
    "github_pat_",
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "glpat-",
    "xoxb-",
    "xoxp-",
    "xapp-",
    "AKIA",
    "ASIA",
    "npm_",
    "sk_live_",
    "sk_test_",
    "rk_live_",
    "rk_test_",
    "AIza",
    "hf_",
    "gsk_",
    "dckr_pat_",
    "vt_",
];

const SECRET_WORDS: [&str; 8] = [
    "KEY",
    "TOKEN",
    "SECRET",
    "PASSWORD",
    "PASSWD",
    "PASS",
    "AUTH",
    "CREDENTIAL",
];

struct Known {
    variable: &'static str,
    prefix: &'static str,
    url: &'static str,
    inject: Option<&'static str>,
}

const KNOWN: [Known; 9] = [
    Known {
        variable: "ANTHROPIC",
        prefix: "sk-ant-",
        url: "https://api.anthropic.com/",
        inject: Some("header:x-api-key"),
    },
    Known {
        variable: "OPENROUTER",
        prefix: "sk-or-",
        url: "https://openrouter.ai/api/",
        inject: None,
    },
    Known {
        variable: "OPENAI",
        prefix: "sk-proj-",
        url: "https://api.openai.com/",
        inject: None,
    },
    Known {
        variable: "GITHUB",
        prefix: "ghp_",
        url: "https://api.github.com/",
        inject: None,
    },
    Known {
        variable: "GH_",
        prefix: "github_pat_",
        url: "https://api.github.com/",
        inject: None,
    },
    Known {
        variable: "STRIPE",
        prefix: "sk_live_",
        url: "https://api.stripe.com/",
        inject: None,
    },
    Known {
        variable: "GROQ",
        prefix: "gsk_",
        url: "https://api.groq.com/",
        inject: None,
    },
    Known {
        variable: "HUGGING",
        prefix: "hf_",
        url: "https://huggingface.co/",
        inject: None,
    },
    Known {
        variable: "SLACK",
        prefix: "xoxb-",
        url: "https://slack.com/api/",
        inject: None,
    },
];

#[derive(Serialize, Debug)]
pub struct Found {
    pub file: PathBuf,
    pub line: usize,
    pub variable: String,
    pub preview: String,
    pub name: String,
    pub approved_urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inject: Option<String>,
    #[serde(skip)]
    secret: Zeroizing<String>,
}

pub struct Migrate {
    pub home: Option<PathBuf>,
    pub path: PathBuf,
    pub take: Vec<String>,
    pub all: bool,
}

fn say(what: &str) {
    eprintln!("ai2 vault: {what}");
}

pub fn preview(secret: &str) -> String {
    let prefix = PREFIXES
        .iter()
        .find(|prefix| secret.starts_with(**prefix) && secret.len() > prefix.len() + 4)
        .copied()
        .unwrap_or_default();
    let chars: Vec<char> = secret.chars().collect();
    let tail: String = if chars.len() >= 12 {
        chars[chars.len() - 4..].iter().collect()
    } else {
        String::new()
    };
    format!("{prefix}…{tail}")
}

fn key_name(raw: &str) -> String {
    let name: String = raw
        .trim_matches(|c: char| !c.is_ascii_alphanumeric())
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let name = name.trim_matches('-').to_string();
    let name: String = name.chars().take(60).collect();
    if name.is_empty() {
        "migrated".into()
    } else {
        name
    }
}

fn unquote(value: &str) -> &str {
    let value = value.trim();
    for quote in ['"', '\''] {
        if let Some(inner) = value
            .strip_prefix(quote)
            .and_then(|rest| rest.split(quote).next())
        {
            return inner;
        }
    }
    value.split(" #").next().unwrap_or(value).trim()
}

fn real_value(value: &str) -> bool {
    value.len() >= 8
        && !value.contains('$')
        && !value.contains('`')
        && !value.contains(char::is_whitespace)
        && !value.starts_with('<')
        && !value.chars().all(|c| c == 'x' || c == 'X' || c == '*')
}

fn secret_variable(variable: &str) -> bool {
    let upper = variable.to_ascii_uppercase();
    SECRET_WORDS.iter().any(|word| upper.contains(word))
}

fn known_prefix(value: &str) -> bool {
    PREFIXES.iter().any(|prefix| value.starts_with(prefix))
}

fn known(variable: &str, value: &str) -> (Vec<String>, Option<String>) {
    let upper = variable.to_ascii_uppercase();
    KNOWN
        .iter()
        .find(|k| value.starts_with(k.prefix))
        .or_else(|| KNOWN.iter().find(|k| upper.starts_with(k.variable)))
        .map(|k| (vec![k.url.to_string()], k.inject.map(str::to_string)))
        .unwrap_or_default()
}

fn found(
    file: &Path,
    line: usize,
    variable: &str,
    name: String,
    secret: &str,
    urls: Vec<String>,
    inject: Option<String>,
) -> Found {
    let shown = match (inject.as_deref(), secret.split_once(':')) {
        (Some("basic"), Some((_, password))) => password,
        _ => secret,
    };
    Found {
        file: file.to_path_buf(),
        line,
        variable: variable.to_string(),
        preview: preview(shown),
        name,
        approved_urls: urls,
        inject,
        secret: Zeroizing::new(secret.to_string()),
    }
}

fn read(file: &Path) -> Option<Zeroizing<String>> {
    let meta = std::fs::symlink_metadata(file).ok()?;
    if !meta.is_file() || meta.len() > BIGGEST {
        return None;
    }
    std::fs::read_to_string(file).ok().map(Zeroizing::new)
}

fn assignments(file: &Path, text: &str) -> Vec<Found> {
    let mut out = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
        let Some((variable, value)) = line.split_once('=') else {
            continue;
        };
        let variable = variable.trim();
        if variable.is_empty()
            || variable.starts_with('#')
            || !variable
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            continue;
        }
        let value = unquote(value);
        if !real_value(value) || !(secret_variable(variable) || known_prefix(value)) {
            continue;
        }
        let (urls, inject) = known(variable, value);
        out.push(found(
            file,
            index + 1,
            variable,
            key_name(variable),
            value,
            urls,
            inject,
        ));
    }
    out
}

fn npmrc(file: &Path, text: &str) -> Vec<Found> {
    let mut out = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let Some((left, value)) = line.trim().split_once('=') else {
            continue;
        };
        let left = left.trim();
        let value = unquote(value);
        let (scope, field) = left.rsplit_once(':').unwrap_or(("", left));
        if !matches!(field, "_authToken" | "_auth" | "_password") || !real_value(value) {
            continue;
        }
        let host = scope
            .trim_start_matches('/')
            .split('/')
            .next()
            .filter(|h| !h.is_empty())
            .unwrap_or("registry.npmjs.org");
        let (secret, inject) = if field == "_auth" {
            let Some(plain) = basic(value) else {
                continue;
            };
            (plain, Some("basic".to_string()))
        } else {
            (Zeroizing::new(value.to_string()), None)
        };
        let item = found(
            file,
            index + 1,
            left,
            key_name(&format!("npm-{host}")),
            &secret,
            vec![format!("https://{host}/")],
            inject,
        );
        out.push(item);
    }
    out
}

fn aws(file: &Path, text: &str) -> Vec<Found> {
    let mut out = Vec::new();
    let mut profile = "default".to_string();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            profile = name.trim().trim_start_matches("profile ").to_string();
            continue;
        }
        let Some((field, value)) = line.split_once('=') else {
            continue;
        };
        let field = field.trim();
        let value = unquote(value);
        if !matches!(
            field,
            "aws_access_key_id" | "aws_secret_access_key" | "aws_session_token"
        ) || !real_value(value)
        {
            continue;
        }
        let short = field.trim_start_matches("aws_").replace('_', "-");
        out.push(found(
            file,
            index + 1,
            &format!("{profile}.{field}"),
            key_name(&format!("aws-{profile}-{short}")),
            value,
            Vec::new(),
            None,
        ));
    }
    out
}

fn netrc(file: &Path, text: &str) -> Vec<Found> {
    let mut out = Vec::new();
    let mut machine = None::<String>;
    for (index, line) in text.lines().enumerate() {
        let words: Vec<&str> = line.split_whitespace().collect();
        let mut at = 0;
        while at < words.len() {
            match words[at] {
                "machine" if at + 1 < words.len() => {
                    machine = Some(words[at + 1].to_string());
                    at += 1;
                }
                "default" => machine = None,
                "password" if at + 1 < words.len() => {
                    let value = words[at + 1];
                    if let Some(host) = machine.as_deref().filter(|_| real_value(value)) {
                        out.push(found(
                            file,
                            index + 1,
                            &format!("{host}.password"),
                            key_name(&format!("netrc-{host}")),
                            value,
                            vec![format!("https://{host}/")],
                            None,
                        ));
                    }
                    at += 1;
                }
                _ => {}
            }
            at += 1;
        }
    }
    out
}

fn git_credentials(file: &Path, text: &str) -> Vec<Found> {
    let mut out = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let Some((scheme, rest)) = line.trim().split_once("://") else {
            continue;
        };
        let Some((userinfo, place)) = rest.rsplit_once('@') else {
            continue;
        };
        let Some((user, password)) = userinfo.split_once(':') else {
            continue;
        };
        let host = place.split('/').next().unwrap_or(place);
        let password = percent_decoded(password);
        if host.is_empty() || !real_value(&password) {
            continue;
        }
        let url = if host == "github.com" {
            "https://api.github.com/".to_string()
        } else {
            format!("{scheme}://{host}/")
        };
        out.push(found(
            file,
            index + 1,
            &format!("{user}@{host}"),
            key_name(&format!("git-{host}")),
            &password,
            vec![url],
            None,
        ));
    }
    out
}

fn percent_decoded(text: &str) -> Zeroizing<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let hex = bytes
            .get(at + 1..at + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[at], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                at += 3;
            }
            (byte, _) => {
                out.push(byte);
                at += 1;
            }
        }
    }
    Zeroizing::new(String::from_utf8_lossy(&out).into_owned())
}

fn gh_hosts(file: &Path, text: &str) -> Vec<Found> {
    let mut out = Vec::new();
    let mut host = None::<String>;
    for (index, line) in text.lines().enumerate() {
        if !line.starts_with(char::is_whitespace) {
            host = line
                .trim()
                .strip_suffix(':')
                .map(|h| h.trim_matches('"').to_string());
            continue;
        }
        let Some((field, value)) = line.trim().split_once(':') else {
            continue;
        };
        let value = unquote(value);
        let Some(host) = host.as_deref() else {
            continue;
        };
        if field.trim() != "oauth_token" || !real_value(value) {
            continue;
        }
        let url = if host == "github.com" {
            "https://api.github.com/".to_string()
        } else {
            format!("https://{host}/api/")
        };
        out.push(found(
            file,
            index + 1,
            &format!("{host}.oauth_token"),
            key_name(&format!("gh-{host}")),
            value,
            vec![url],
            None,
        ));
    }
    out
}

fn basic(encoded: &str) -> Option<Zeroizing<String>> {
    let plain = Zeroizing::new(
        base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .ok()?,
    );
    let text = std::str::from_utf8(&plain).ok()?;
    text.contains(':').then(|| Zeroizing::new(text.to_string()))
}

fn docker(file: &Path, text: &str) -> Vec<Found> {
    let Ok(config) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (registry, entry) in config["auths"].as_object().into_iter().flatten() {
        let Some(auth) = entry["auth"].as_str().filter(|a| real_value(a)) else {
            continue;
        };
        let line = text
            .lines()
            .enumerate()
            .skip_while(|(_, l)| !l.contains(&format!("\"{registry}\"")))
            .find(|(_, l)| l.contains("\"auth\""))
            .map(|(index, _)| index + 1)
            .unwrap_or(1);
        let host = registry
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .split('/')
            .next()
            .unwrap_or(registry);
        let host = if host == "index.docker.io" {
            "registry-1.docker.io"
        } else {
            host
        };
        let Some(plain) = basic(auth) else {
            continue;
        };
        out.push(found(
            file,
            line,
            &format!("auths.{registry}.auth"),
            key_name(&format!("docker-{host}")),
            &plain,
            vec![format!("https://{host}/")],
            Some("basic".into()),
        ));
    }
    out
}

fn env_file(name: &str) -> bool {
    (name == ".env" || name.starts_with(".env."))
        && !EXAMPLES
            .iter()
            .any(|word| name.to_ascii_lowercase().contains(word))
}

fn walk(folder: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            if !name.starts_with('.') && !DEPENDENCY_FOLDERS.contains(&name.as_str()) {
                walk(&entry.path(), out);
            }
        } else if kind.is_file() && env_file(&name) {
            out.push(entry.path());
        }
    }
}

pub fn scan(home: Option<&Path>, folder: &Path) -> Vec<Found> {
    let mut found = Vec::new();
    let mut seen = HashSet::new();
    let mut files: Vec<PathBuf> = Vec::new();
    walk(folder, &mut files);
    for file in files {
        let file = std::fs::canonicalize(&file).unwrap_or(file);
        if seen.insert(file.clone()) {
            if let Some(text) = read(&file) {
                found.extend(assignments(&file, &text));
            }
        }
    }
    let Some(home) = home else {
        return found;
    };
    type Parse = fn(&Path, &str) -> Vec<Found>;
    let mut homes: Vec<(PathBuf, Parse)> = SHELL_FILES
        .iter()
        .map(|name| (home.join(name), assignments as Parse))
        .collect();
    homes.extend([
        (home.join(".npmrc"), npmrc as Parse),
        (home.join(".aws/credentials"), aws as Parse),
        (home.join(".netrc"), netrc as Parse),
        (home.join(".git-credentials"), git_credentials as Parse),
        (home.join(".config/gh/hosts.yml"), gh_hosts as Parse),
        (home.join(".docker/config.json"), docker as Parse),
    ]);
    for (file, parse) in homes {
        let file = std::fs::canonicalize(&file).unwrap_or(file);
        if !seen.insert(file.clone()) {
            continue;
        }
        if let Some(text) = read(&file) {
            found.extend(parse(&file, &text));
        }
    }
    found
}

#[derive(Debug, PartialEq)]
pub struct Take {
    pub file: PathBuf,
    pub line: usize,
    pub name: Option<String>,
}

pub fn parse_take(spec: &str) -> Option<Take> {
    let (file, rest) = spec.rsplit_once(':')?;
    let (line, name) = match rest.split_once('=') {
        Some((line, name)) => (line, Some(name.to_string())),
        None => (rest, None),
    };
    let line = line.trim().parse().ok().filter(|&line: &usize| line > 0)?;
    let file = PathBuf::from(file);
    let file = std::fs::canonicalize(&file).unwrap_or(file);
    (!file.as_os_str().is_empty()).then_some(Take {
        file,
        line,
        name: name.filter(|n| !n.is_empty()),
    })
}

pub fn free_name(wanted: &str, taken: &HashSet<String>) -> String {
    if !taken.contains(wanted) {
        return wanted.to_string();
    }
    (2..)
        .map(|n| format!("{wanted}-{n}"))
        .find(|name| !taken.contains(name))
        .unwrap_or_default()
}

fn print(value: &Value) {
    println!("{value}");
}

fn usage(message: &str) -> i32 {
    print(&json!({ "error": "usage", "message": message }));
    exit::USAGE
}

fn chosen(found: Vec<Found>, take: &[String], all: bool) -> Result<Vec<Found>, String> {
    if all {
        return Ok(found);
    }
    let mut picks = Vec::new();
    let mut found: Vec<Option<Found>> = found.into_iter().map(Some).collect();
    for spec in take {
        let Some(want) = parse_take(spec) else {
            return Err(format!("{spec} is not <file>:<line>[=name]"));
        };
        let Some(mut pick) = found
            .iter_mut()
            .find(|slot| {
                slot.as_ref()
                    .is_some_and(|f| f.file == want.file && f.line == want.line)
            })
            .and_then(Option::take)
        else {
            return Err(format!("no secret was found at {spec}"));
        };
        if let Some(name) = want.name {
            pick.name = name;
        }
        picks.push(pick);
    }
    Ok(picks)
}

fn proposed(admin: &Admin, pick: &Found) -> Result<Vec<String>, String> {
    let description = description(pick);
    let (status, value) = admin
        .call(
            "POST",
            &format!("/keys/{}/propose", encode_percent(&pick.name)),
            Some(json!({ "description": description })),
            false,
        )
        .map_err(|error| format!("{error:#}"))?;
    if status != 200 {
        return Err(value["message"]
            .as_str()
            .unwrap_or("the vetting model proposed nothing")
            .to_string());
    }
    let urls: Vec<String> =
        serde_json::from_value(value["approved_urls"].clone()).unwrap_or_default();
    if urls.is_empty() {
        return Err("no approved URLs are known for it and the vetting model proposed none".into());
    }
    Ok(urls)
}

fn description(pick: &Found) -> String {
    format!(
        "{} from {}:{}",
        pick.variable,
        pick.file.display(),
        pick.line
    )
}

fn stored_names(admin: &Admin) -> Result<Result<HashSet<String>, i32>> {
    let (status, value) = admin.call("GET", "/keys", None, false)?;
    if status != 200 {
        print(&value);
        return Ok(Err(code(status)));
    }
    Ok(Ok(value["keys"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|key| key["name"].as_str().map(str::to_string))
        .collect()))
}

fn spot(pick: &Found) -> Value {
    json!({ "file": pick.file, "line": pick.line, "name": pick.name })
}

pub fn run(admin: &Admin, migrate: Migrate) -> Result<i32> {
    let found = scan(migrate.home.as_deref(), &migrate.path);
    if migrate.take.is_empty() && !migrate.all {
        print(&json!({ "found": found }));
        return Ok(exit::OK);
    }
    let mut picks = match chosen(found, &migrate.take, migrate.all) {
        Ok(picks) => picks,
        Err(message) => return Ok(usage(&message)),
    };
    let mut taken = match stored_names(admin)? {
        Ok(names) => names,
        Err(code) => return Ok(code),
    };
    let mut skipped = Vec::new();
    let mut ready = Vec::new();
    for mut pick in picks.drain(..) {
        pick.name = free_name(&pick.name, &taken);
        taken.insert(pick.name.clone());
        if pick.approved_urls.is_empty() {
            match proposed(admin, &pick) {
                Ok(urls) => pick.approved_urls = urls,
                Err(reason) => {
                    let mut item = spot(&pick);
                    item["reason"] = json!(reason);
                    skipped.push(item);
                    continue;
                }
            }
        }
        ready.push(pick);
    }
    if ready.is_empty() {
        print(&json!({ "stored": [], "skipped": skipped }));
        return Ok(if skipped.is_empty() {
            exit::OK
        } else {
            exit::ERROR
        });
    }
    let (stored, failed, worst) = store(admin, &ready)?;
    say("the source files were not changed; remove the secrets from them once your apps use the vault");
    print(&json!({ "stored": stored, "failed": failed, "skipped": skipped }));
    Ok(match (worst, skipped.is_empty()) {
        (exit::OK, false) => exit::ERROR,
        (worst, _) => worst,
    })
}

fn store(admin: &Admin, ready: &[Found]) -> Result<(Vec<Value>, Vec<Value>, i32)> {
    let mut stored = Vec::new();
    let mut failed = Vec::new();
    let mut worst = exit::OK;
    let _held = admin.hold();
    for pick in ready {
        let body = json!({
            "secret": pick.secret.as_str(),
            "description": description(pick),
            "approved_urls": pick.approved_urls,
            "inject": pick.inject,
        });
        let (status, value) = admin.call(
            "PUT",
            &format!("/keys/{}", encode_percent(&pick.name)),
            Some(body),
            true,
        )?;
        let mut item = spot(pick);
        if status == 200 {
            item["approved_urls"] = json!(pick.approved_urls);
            stored.push(item);
            continue;
        }
        item["error"] = value["message"].clone();
        failed.push(item);
        if worst == exit::OK {
            worst = code(status);
        }
        if status == 401 {
            break;
        }
    }
    Ok((stored, failed, worst))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing;

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn a_preview_shows_the_known_prefix_and_the_last_four_only() {
        assert_eq!(preview("sk-ant-api03-abcdefghijklmnop"), "sk-ant-…mnop");
        assert_eq!(preview("plainsecretvalue1234"), "…1234");
        assert_eq!(preview("short123"), "…");
    }

    #[test]
    fn env_files_are_found_and_hidden_and_dependency_folders_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            &root.join("app/.env"),
            "# a comment\nexport OPENAI_API_KEY=\"sk-proj-0123456789abcdef\"\nPORT=8080\nDB_PASSWORD=${FROM_ELSEWHERE}\nSTRIPE_SECRET_KEY=sk_live_aaaaaaaaaaaa9999 # live\n",
        );
        write(
            &root.join("app/.env.example"),
            "OPENAI_API_KEY=sk-proj-example0000\n",
        );
        write(
            &root.join("node_modules/x/.env"),
            "TOKEN=abcdefghijklmnop\n",
        );
        write(&root.join(".hidden/.env"), "TOKEN=abcdefghijklmnop\n");
        write(&root.join("app/target/.env"), "TOKEN=abcdefghijklmnop\n");
        let found = scan(None, root);
        let seen: Vec<_> = found
            .iter()
            .map(|f| {
                (
                    f.line,
                    f.variable.as_str(),
                    f.name.as_str(),
                    f.preview.as_str(),
                )
            })
            .collect();
        assert_eq!(
            seen,
            [
                (2, "OPENAI_API_KEY", "openai-api-key", "sk-proj-…cdef"),
                (5, "STRIPE_SECRET_KEY", "stripe-secret-key", "sk_live_…9999"),
            ]
        );
        assert_eq!(found[0].approved_urls, ["https://api.openai.com/"]);
        assert_eq!(found[1].approved_urls, ["https://api.stripe.com/"]);
        let printed = serde_json::to_string(&found).unwrap();
        assert!(!printed.contains("0123456789abcdef"));
        assert!(!printed.contains("aaaaaaaaaaaa9999"));
    }

    #[test]
    fn home_files_are_read_with_their_own_formats() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        write(
            &home.join(".zshrc"),
            "alias ll='ls -l'\nexport ANTHROPIC_API_KEY=sk-ant-api03-zzzzzzzzzzzz1111\n",
        );
        write(
            &home.join(".npmrc"),
            "//registry.npmjs.org/:_authToken=npm_bbbbbbbbbbbbbb2222\n",
        );
        write(
            &home.join(".aws/credentials"),
            "[default]\naws_access_key_id = AKIAAAAAAAAAAAAA3333\naws_secret_access_key = cccccccccccccccccccc4444\n",
        );
        write(
            &home.join(".netrc"),
            "machine api.example.com login ada password dddddddddddd5555\n",
        );
        write(
            &home.join(".git-credentials"),
            "https://ada:ghp_eeeeeeeeeeeeeeee6666@github.com\n",
        );
        write(
            &home.join(".config/gh/hosts.yml"),
            "github.com:\n    user: ada\n    oauth_token: gho_ffffffffffffffff7777\n",
        );
        let auth = base64::engine::general_purpose::STANDARD.encode("ada:gggggggggggg8888");
        write(
            &home.join(".docker/config.json"),
            &format!("{{\n  \"auths\": {{\n    \"ghcr.io\": {{\n      \"auth\": \"{auth}\"\n    }}\n  }}\n}}\n"),
        );
        let empty = dir.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        let found = scan(Some(&home), &empty);
        let seen: Vec<_> = found
            .iter()
            .map(|f| (f.name.as_str(), f.line, f.preview.as_str()))
            .collect();
        assert_eq!(
            seen,
            [
                ("anthropic-api-key", 2, "sk-ant-…1111"),
                ("npm-registry.npmjs.org", 1, "npm_…2222"),
                ("aws-default-access-key-id", 2, "AKIA…3333"),
                ("aws-default-secret-access-key", 3, "…4444"),
                ("netrc-api.example.com", 1, "…5555"),
                ("git-github.com", 1, "ghp_…6666"),
                ("gh-github.com", 3, "gho_…7777"),
                ("docker-ghcr.io", 4, "…8888"),
            ]
        );
        assert_eq!(found[0].inject.as_deref(), Some("header:x-api-key"));
        assert_eq!(found[5].approved_urls, ["https://api.github.com/"]);
        assert_eq!(found[7].inject.as_deref(), Some("basic"));
        assert!(found[2].approved_urls.is_empty());
    }

    #[test]
    fn takes_and_free_names_are_parsed() {
        assert_eq!(
            parse_take("/no/such/.env:3=stripe"),
            Some(Take {
                file: "/no/such/.env".into(),
                line: 3,
                name: Some("stripe".into())
            })
        );
        assert_eq!(parse_take("/no/such/.env:0"), None);
        assert_eq!(parse_take("nofile"), None);
        let taken: HashSet<String> = ["a".into(), "a-2".into()].into();
        assert_eq!(free_name("a", &taken), "a-3");
        assert_eq!(free_name("b", &taken), "b");
    }

    #[test]
    fn chosen_secrets_are_stored_without_overwriting_and_the_files_stay_as_they_were() {
        let daemon = testing::Daemon::start();
        let dir = tempfile::tempdir().unwrap();
        let env = dir.path().join(".env");
        let text = "OPENAI_API_KEY=sk-proj-0123456789abcdef\nGITHUB_TOKEN=ghp_0123456789abcdef\nAWS_SECRET=zzzzzzzzzzzzzzzz\n";
        write(&env, text);
        daemon.put("openai-api-key");
        let opts = daemon.options();
        let admin = Admin::new(&opts);
        let file = std::fs::canonicalize(&env).unwrap();
        let take = |spec: &str| format!("{}:{spec}", file.display());
        let migrate = Migrate {
            home: None,
            path: dir.path().into(),
            take: vec![take("1"), take("2=gh")],
            all: false,
        };
        assert_eq!(run(&admin, migrate).unwrap(), exit::OK);
        let names = daemon.key_names();
        assert_eq!(names, ["openai-api-key", "openai-api-key-2", "gh"]);
        assert_eq!(std::fs::read_to_string(&env).unwrap(), text);
        let missing = Migrate {
            home: None,
            path: dir.path().into(),
            take: vec![take("9")],
            all: false,
        };
        assert_eq!(run(&admin, missing).unwrap(), exit::USAGE);
        let all = Migrate {
            home: None,
            path: dir.path().into(),
            take: Vec::new(),
            all: true,
        };
        assert_eq!(run(&admin, all).unwrap(), exit::ERROR);
        assert_eq!(
            daemon.key_names(),
            [
                "openai-api-key",
                "openai-api-key-2",
                "gh",
                "openai-api-key-3",
                "github-token"
            ]
        );
    }
}
