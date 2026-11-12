use crate::setup::{Personality, Setup};

pub fn sh_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r#"'"'"'"#))
}

pub fn run_line(setup: &Setup, prompt: &str) -> String {
    let run = setup
        .headless()
        .map_or(setup.command.as_str(), |headless| headless.run.as_str());
    format!("{run} {}", sh_quote(prompt))
}

pub fn takes_personality(setup: &Setup) -> bool {
    setup
        .headless()
        .is_some_and(|headless| headless.personality.is_some())
}

/// `words` as a TOML basic string, quotes included: what a `-c key=value` override parses as a string. Backslash, the double
/// quote and every control character (and DEL) are escaped; everything else, unicode too, is kept as it is.
pub fn toml_string(words: &str) -> String {
    let mut out = String::with_capacity(words.len() + 2);
    out.push('"');
    for c in words.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 || c == '\u{7f}' => {
                out.push_str(&format!("\\u{:04X}", c as u32))
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn task_line(
    setup: &Setup,
    prompt: &str,
    folder: &str,
    model: Option<&str>,
    personality: Option<&str>,
) -> String {
    let start = format!("cd {} && exec ", sh_quote(folder));
    let Some(headless) = setup.headless() else {
        return format!("{start}{}", run_line(setup, prompt));
    };
    let given = personality.filter(|words| !words.is_empty() && takes_personality(setup));
    let env = [
        headless.env.clone(),
        given.and_then(|words| match &headless.personality {
            Some(Personality::Env(name)) => Some(format!("{name}={}", sh_quote(words))),
            _ => None,
        }),
    ]
    .into_iter()
    .flatten()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join(" ");
    let flag = match (given, &headless.personality) {
        (Some(words), Some(Personality::Flag(name))) => format!(" {name} {}", sh_quote(words)),
        (Some(words), Some(Personality::Config(key))) => {
            format!(" -c {}", sh_quote(&format!("{key}={}", toml_string(words))))
        }
        _ => String::new(),
    };
    let named = match model {
        Some(model) if !model.is_empty() && !headless.no_model => {
            format!(" --model {}", sh_quote(model))
        }
        _ => String::new(),
    };
    let env = if env.is_empty() {
        env
    } else {
        format!("env {env} ")
    };
    format!(
        "{start}{env}{} {}{named}{flag}",
        run_line(setup, prompt),
        headless.flags
    )
}
