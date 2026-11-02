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
    setup.headless().is_some_and(|headless| {
        headless.personality.is_some() || headless.personality_env.is_some()
    })
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
        given.and_then(|words| {
            headless
                .personality_env
                .as_ref()
                .map(|name| format!("{name}={}", sh_quote(words)))
        }),
    ]
    .into_iter()
    .flatten()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join(" ");
    let flag = match (given, &headless.personality) {
        (Some(words), Some(Personality::Flag(name))) => format!(" {name} {}", sh_quote(words)),
        (Some(words), Some(Personality::Given(given))) => format!(" {}", given(words)),
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
