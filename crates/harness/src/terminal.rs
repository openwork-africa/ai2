use std::sync::LazyLock;

use regex::Regex;

const TERMINAL: &str = "stty cols 1000 rows 50 2>/dev/null; ";

fn pattern(source: &str) -> Regex {
    Regex::new(source).expect("a valid pattern")
}

static OSC: LazyLock<Regex> = LazyLock::new(|| pattern(r"\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)"));
static OSC_LINK: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"\x1b\]8;[^;\x07\x1b]*;([^\x07\x1b]+)(?:\x07|\x1b\\)"));
static CSI: LazyLock<Regex> = LazyLock::new(|| pattern(r"\x1b\[[0-?]*[ -/]*[@-~]"));
static ESCAPE: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"\x1b[@-Z\\-_]|[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]"));
static LINE_RESET: LazyLock<Regex> = LazyLock::new(|| pattern(r"\x1b\[[0-9;]*[GFAK]"));
static URL_IN: LazyLock<Regex> = LazyLock::new(|| pattern(r#"https?://[^\s"'<>`)\]]+"#));
static DEVICE_CODE: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"\b[A-Z0-9]{4,5}(?:-[A-Z0-9]{4,5}){1,2}\b"));
static CALLED_CODE: LazyLock<Regex> = LazyLock::new(|| pattern(r"(?i)\bcode\b"));
static TRAILING: LazyLock<Regex> = LazyLock::new(|| pattern(r"[.,;:]+$"));

pub fn in_terminal(line: &str, os: &str) -> Vec<String> {
    let line = format!("{TERMINAL}{line}");
    let args: Vec<&str> = if os == "macos" || os == "darwin" {
        vec!["script", "-q", "/dev/null", "sh", "-c", &line]
    } else {
        vec!["script", "-qfec", &line, "/dev/null"]
    };
    args.into_iter().map(String::from).collect()
}

pub fn plain_text(raw: &str) -> String {
    let text = OSC.replace_all(raw, "").replace("\r\n", "\n");
    let text = LINE_RESET.replace_all(&text, "\r");
    let text = CSI.replace_all(&text, "");
    let text = ESCAPE.replace_all(&text, "");
    text.split('\n')
        .map(|line| {
            line.split('\r')
                .rfind(|part| !part.trim().is_empty())
                .unwrap_or("")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn links_in(raw: &str) -> Vec<String> {
    let hyperlinks = OSC_LINK
        .captures_iter(raw)
        .map(|found| found[1].to_string());
    let plain = plain_text(raw);
    let plain = URL_IN
        .find_iter(&plain)
        .map(|found| found.as_str().to_string());
    let mut links: Vec<String> = Vec::new();
    for url in hyperlinks
        .chain(plain)
        .map(|url| TRAILING.replace(&url, "").into_owned())
    {
        if !links.contains(&url) {
            links.push(url);
        }
    }
    links
}

pub fn code_in(line: &str) -> Option<String> {
    let bare = URL_IN.replace_all(line, "");
    let found = DEVICE_CODE.find(&bare)?.as_str();
    if !found.chars().any(|c| c.is_ascii_uppercase()) {
        return None;
    }
    let called = CALLED_CODE.is_match(&line.replacen(found, "", 1));
    (found.chars().any(|c| c.is_ascii_digit()) || called).then(|| found.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_output_is_read_as_plain_lines_its_links_and_its_sign_in_codes() {
        let raw = "\x1b[1mOpen\x1b[0m \x1b]8;;https://claude.ai/oauth/authorize?code=true&x=1\x07this link\x1b]8;;\x07.\r\nThen enter ABCD-12345\r\n";
        assert_eq!(plain_text(raw), "Open this link.\nThen enter ABCD-12345\n");
        assert_eq!(
            plain_text("│  ▪_\x1b[2K\x1b[G│  ▪▪_\x1b[2K\x1b[G│  ▪▪▪\n"),
            "│  ▪▪▪\n"
        );
        assert_eq!(plain_text("10%\r50%\r100%\r\n"), "100%\n");
        assert_eq!(
            links_in(raw),
            ["https://claude.ai/oauth/authorize?code=true&x=1"]
        );
        assert_eq!(
            links_in("Go to https://auth.openai.com/codex/device."),
            ["https://auth.openai.com/codex/device"]
        );
        assert_eq!(
            code_in("Then enter ABCD-12345").as_deref(),
            Some("ABCD-12345")
        );
        assert_eq!(code_in("Installing CLAUDE-CODE"), None);
        assert_eq!(code_in("see https://x.io/ABCD-12345"), None);
        assert_eq!(
            code_in("! First copy your one-time code: WDJB-MJHT").as_deref(),
            Some("WDJB-MJHT")
        );
        assert_eq!(code_in("Building WDJB-MJHT"), None);
    }

    #[test]
    fn a_line_runs_in_a_terminal() {
        assert_eq!(
            in_terminal("claude auth login", "linux"),
            [
                "script",
                "-qfec",
                "stty cols 1000 rows 50 2>/dev/null; claude auth login",
                "/dev/null"
            ]
        );
        assert_eq!(
            in_terminal("true", "macos"),
            [
                "script",
                "-q",
                "/dev/null",
                "sh",
                "-c",
                "stty cols 1000 rows 50 2>/dev/null; true"
            ]
        );
    }
}
