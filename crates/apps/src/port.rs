use regex::Regex;
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::path::Path;
use std::sync::LazyLock;
use std::time::Duration;

static IN_COMMAND: LazyLock<[Regex; 3]> = LazyLock::new(|| {
    [
        Regex::new(r"--port[= ](\d{2,5})").unwrap(),
        Regex::new(r"\b[A-Z_]*PORT=(\d{2,5})").unwrap(),
        Regex::new(r"\s-p[= ](\d{2,5})\b").unwrap(),
    ]
});

static IN_CODE: LazyLock<[Regex; 3]> = LazyLock::new(|| {
    [
        Regex::new(r"\bport\s*:\s*(\d{2,5})\b").unwrap(),
        Regex::new(r#"PORT["'`]?\s*\)?\s*(?:\?\?|\|\|)\s*["'`]?(\d{2,5})"#).unwrap(),
        Regex::new(r"\.listen\(\s*(\d{2,5})").unwrap(),
    ]
});

static VITE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bvite\b").unwrap());
static VITE_BUILD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bvite\S*\s+build\b").unwrap());
static VITE_PORT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bport\s*:\s*(\d{2,5})").unwrap());
static SCRIPT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\.m?[jt]sx?$").unwrap());
static DENO_TASK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"deno task ([\w:.-]+)").unwrap());
static NOT_SERVING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"build|check|test|lint|fmt|install").unwrap());

const VITE_CONFIGS: [&str; 4] = [
    "vite.config.ts",
    "vite.config.js",
    "vite.config.mts",
    "vite.config.mjs",
];

pub fn detect(dir: &Path, command: &str) -> Option<u16> {
    let tasks = deno_tasks(dir);
    detect_in(dir, command, &tasks, &mut HashSet::new())
}

fn detect_in(
    dir: &Path,
    command: &str,
    tasks: &Map<String, Value>,
    seen: &mut HashSet<String>,
) -> Option<u16> {
    if let Some(port) = first(command, IN_COMMAND.as_slice()) {
        return Some(port);
    }
    if VITE.is_match(command) && !VITE_BUILD.is_match(command) {
        let configured = VITE_CONFIGS.iter().find_map(|file| {
            let text = std::fs::read_to_string(dir.join(file)).ok()?;
            first(&text, std::slice::from_ref(&*VITE_PORT))
        });
        return Some(configured.unwrap_or(5173));
    }
    let in_code = command
        .split_whitespace()
        .filter(|token| SCRIPT.is_match(token) && !token.starts_with('-'))
        .find_map(|token| {
            let text = std::fs::read_to_string(dir.join(token)).ok()?;
            first(&text, IN_CODE.as_slice())
        });
    if in_code.is_some() {
        return in_code;
    }
    for name in DENO_TASK
        .captures_iter(command)
        .map(|c| c[1].to_string())
        .collect::<Vec<_>>()
    {
        if seen.contains(&name) || NOT_SERVING.is_match(&name) {
            continue;
        }
        let Some(task) = tasks.get(&name).map(task_command) else {
            continue;
        };
        seen.insert(name);
        if let Some(port) = detect_in(dir, &task, tasks, seen) {
            return Some(port);
        }
    }
    None
}

fn first(text: &str, patterns: &[Regex]) -> Option<u16> {
    patterns.iter().find_map(|pattern| {
        pattern
            .captures(text)
            .and_then(|c| c[1].parse::<u16>().ok())
            .filter(|port| *port > 0)
    })
}

fn task_command(task: &Value) -> String {
    match task {
        Value::String(command) => command.clone(),
        other => other["command"].as_str().unwrap_or_default().to_string(),
    }
}

fn deno_tasks(dir: &Path) -> Map<String, Value> {
    ["deno.json", "deno.jsonc"]
        .iter()
        .find_map(|file| std::fs::read_to_string(dir.join(file)).ok())
        .and_then(|text| serde_json::from_str::<Value>(&strip_jsonc(&text)).ok())
        .and_then(|config| config.get("tasks").and_then(Value::as_object).cloned())
        .unwrap_or_default()
}

pub fn strip_jsonc(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut in_string = false;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(n) = next {
                    out.push(n);
                    i += 1;
                }
            } else if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        match (c, next) {
            ('"', _) => {
                in_string = true;
                out.push(c);
                i += 1;
            }
            ('/', Some('/')) => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            ('/', Some('*')) => {
                i += 2;
                while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                    i += 1;
                }
                i += 2;
            }
            (',', _) => {
                let rest = chars[i + 1..].iter().find(|c| !c.is_whitespace());
                if !matches!(rest, Some('}') | Some(']')) {
                    out.push(c);
                }
                i += 1;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

pub fn listening(port: u16) -> bool {
    let timeout = Duration::from_millis(200);
    [
        SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
        SocketAddr::from((Ipv6Addr::LOCALHOST, port)),
    ]
    .iter()
    .any(|addr| TcpStream::connect_timeout(addr, timeout).is_ok())
}
