use crate::cli::exit;
use std::io::Write;

pub const GUIDE: &str = include_str!("skill.md");

pub fn run() -> i32 {
    match std::io::stdout().lock().write_all(GUIDE.as_bytes()) {
        Ok(()) => exit::OK,
        Err(_) => exit::ERROR,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_guide_covers_the_gateway_refusals_and_exit_codes() {
        for needed in [
            "<endpoint>/<key>/<host>/<path>",
            "Authorization: Bearer $AI2_VAULT_TOKEN",
            "X-Vault-Request",
            "approval_timeout",
            "Never retry a denial in a loop",
            "ai2 vault with",
            "`64` usage",
        ] {
            assert!(GUIDE.contains(needed), "the guide does not say {needed}");
        }
        for reason in [
            "token_unknown",
            "token_expired",
            "token_used_up",
            "wrong_sandbox",
            "key_not_allowed",
            "method_not_allowed",
            "url_not_allowed",
            "vetting_denied",
            "vetting_unavailable",
            "approval_denied",
            "wrong_user",
        ] {
            assert!(GUIDE.contains(reason), "the guide leaves out {reason}");
        }
    }
}
