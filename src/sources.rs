use crate::scan::Spec;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const OSV_BATCH: &str = "https://api.osv.dev/v1/querybatch";
const OSV_VULN: &str = "https://api.osv.dev/v1/vulns/";
const KEV: &str =
    "https://www.cisa.gov/sites/default/files/feeds/known_exploited_vulnerabilities.json";
const OSV_QUERIES_PER_CALL: usize = 500;
const TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Serialize, Deserialize, Clone)]
pub struct Advisory {
    pub source: String,
    pub id: String,
    pub technology: String,
    pub published: String,
    pub summary: String,
}

pub fn recent(spec: &Spec, days: u64, notes: &mut Vec<String>) -> Vec<Advisory> {
    let since = date_days_ago(days);
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .build()
        .new_agent();
    let mut found = Vec::new();
    for (name, search) in [("OSV.dev", osv as Search), ("CISA KEV", kev as Search)] {
        match search(&agent, spec, &since) {
            Ok(advisories) => found.extend(advisories),
            Err(error) => notes.push(format!("{name} could not be searched: {error:#}")),
        }
    }
    found
}

type Search = fn(&ureq::Agent, &Spec, &str) -> Result<Vec<Advisory>>;

fn osv(agent: &ureq::Agent, spec: &Spec, since: &str) -> Result<Vec<Advisory>> {
    let known: Vec<_> = spec
        .libraries
        .iter()
        .filter(|l| !l.version.is_empty() && !l.ecosystem.is_empty())
        .collect();
    let mut found = Vec::new();
    for chunk in known.chunks(OSV_QUERIES_PER_CALL) {
        let queries: Vec<Value> = chunk.iter().map(|l| json!({ "package": { "name": l.name, "ecosystem": l.ecosystem }, "version": l.version })).collect();
        let answer: Value = agent
            .post(OSV_BATCH)
            .send_json(json!({ "queries": queries }))?
            .body_mut()
            .read_json()?;
        for (library, result) in chunk
            .iter()
            .zip(answer["results"].as_array().into_iter().flatten())
        {
            for vuln in result["vulns"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|v| v["modified"].as_str().unwrap_or("") >= since)
            {
                let id = vuln["id"].as_str().unwrap_or_default();
                let full: Value = agent
                    .get(&format!("{OSV_VULN}{id}"))
                    .call()?
                    .body_mut()
                    .read_json()
                    .context(id.to_string())?;
                let published = full["published"].as_str().unwrap_or_default();
                if published >= since {
                    let summary = full["summary"]
                        .as_str()
                        .or(full["details"].as_str())
                        .unwrap_or_default();
                    found.push(Advisory {
                        source: "OSV.dev".into(),
                        id: id.into(),
                        technology: format!("{} {}", library.name, library.version),
                        published: published.into(),
                        summary: summary.chars().take(400).collect(),
                    });
                }
            }
        }
    }
    Ok(found)
}

fn kev(agent: &ureq::Agent, _spec: &Spec, since: &str) -> Result<Vec<Advisory>> {
    let feed: Value = agent.get(KEV).call()?.body_mut().read_json()?;
    Ok(feed["vulnerabilities"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|v| v["dateAdded"].as_str().unwrap_or("") >= &since[..10])
        .map(|v| Advisory {
            source: "CISA KEV".into(),
            id: v["cveID"].as_str().unwrap_or_default().into(),
            technology: format!(
                "{} {}",
                v["vendorProject"].as_str().unwrap_or_default(),
                v["product"].as_str().unwrap_or_default()
            ),
            published: v["dateAdded"].as_str().unwrap_or_default().into(),
            summary: v["shortDescription"].as_str().unwrap_or_default().into(),
        })
        .collect())
}

fn date_days_ago(days: u64) -> String {
    let (y, m, d) = civil((seconds() / 86_400).saturating_sub(days) as i64);
    format!("{y:04}-{m:02}-{d:02}T00:00:00Z")
}

pub fn now() -> String {
    let s = seconds();
    let (y, m, d) = civil((s / 86_400) as i64);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        s % 86_400 / 3_600,
        s % 3_600 / 60,
        s % 60
    )
}

fn seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
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
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_compare_as_text() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(20_759), (2026, 11, 2));
        assert!(date_days_ago(7) < date_days_ago(0));
        assert!("2026-11-01T10:00:00Z" >= "2026-10-26T00:00:00Z");
    }
}
