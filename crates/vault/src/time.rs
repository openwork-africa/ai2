use anyhow::{bail, Result};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn stamp() -> String {
    rfc3339(now())
}

pub fn rfc3339(s: u64) -> String {
    let (y, m, d) = civil((s / 86_400) as i64);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        s % 86_400 / 3_600,
        s % 3_600 / 60,
        s % 60
    )
}

fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

pub fn span(secs: u64) -> String {
    match secs {
        s if s % 86_400 == 0 => format!("{}d", s / 86_400),
        s if s % 3_600 == 0 => format!("{}h", s / 3_600),
        s if s % 60 == 0 => format!("{}m", s / 60),
        s => format!("{s}s"),
    }
}

pub fn ttl(text: &str) -> Result<u64> {
    let text = text.trim();
    let split = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let Ok(number) = number.parse::<u64>() else {
        bail!("{text} is not a duration like 15m, 8h or 30d");
    };
    let scale = match unit {
        "" | "s" => 1,
        "m" => 60,
        "h" => 3_600,
        "d" => 86_400,
        _ => bail!("{text} is not a duration like 15m, 8h or 30d"),
    };
    if number == 0 {
        bail!("a token's ttl must be more than zero");
    }
    Ok(number.saturating_mul(scale))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_and_stamps_read_as_people_write_them() {
        assert_eq!(ttl("8h").unwrap(), 28_800);
        assert_eq!(ttl("30d").unwrap(), 2_592_000);
        assert_eq!(ttl("90").unwrap(), 90);
        assert!(ttl("0h").is_err() && ttl("8 hours").is_err() && ttl("h").is_err());
        assert_eq!(
            (span(28_800), span(2_592_000), span(90)),
            ("8h".into(), "30d".into(), "90s".into())
        );
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(1_700_000_000), "2023-11-14T22:13:20Z");
    }
}
