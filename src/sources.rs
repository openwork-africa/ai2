use crate::scan::Spec;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

const OSV_BATCH: &str = "https://api.osv.dev/v1/querybatch";
const OSV_VULN: &str = "https://api.osv.dev/v1/vulns/";
const KEV: &str =
    "https://www.cisa.gov/sites/default/files/feeds/known_exploited_vulnerabilities.json";
const OSV_QUERIES_PER_CALL: usize = 500;
pub const OSV_MAX_FETCH: usize = 200;
const OSV_WORKERS: usize = 8;

#[derive(Clone)]
pub struct Endpoints {
    pub osv_batch: String,
    pub osv_vuln: String,
    pub kev: String,
}
impl Default for Endpoints {
    fn default() -> Self {
        Self {
            osv_batch: OSV_BATCH.into(),
            osv_vuln: OSV_VULN.into(),
            kev: KEV.into(),
        }
    }
}
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
    recent_with(&Endpoints::default(), spec, days, notes)
}
pub fn recent_with(
    endpoints: &Endpoints,
    spec: &Spec,
    days: u64,
    notes: &mut Vec<String>,
) -> Vec<Advisory> {
    let since = date_days_ago(days);
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .build()
        .new_agent();
    let mut found = Vec::new();
    match osv(&agent, endpoints, spec, &since, notes) {
        Ok(advisories) => found.extend(advisories),
        Err(error) => notes.push(format!("OSV.dev could not be searched: {error:#}")),
    }
    match kev(&agent, endpoints, spec, &since) {
        Ok(advisories) => found.extend(advisories),
        Err(error) => notes.push(format!("CISA KEV could not be searched: {error:#}")),
    }
    found
}

fn osv(
    agent: &ureq::Agent,
    endpoints: &Endpoints,
    spec: &Spec,
    since: &str,
    notes: &mut Vec<String>,
) -> Result<Vec<Advisory>> {
    let known: Vec<_> = spec
        .libraries
        .iter()
        .filter(|l| !l.version.is_empty() && !l.ecosystem.is_empty())
        .collect();
    let mut wanted: Vec<(String, String)> = Vec::new();
    for chunk in known.chunks(OSV_QUERIES_PER_CALL) {
        let queries: Vec<Value> = chunk.iter().map(|l| json!({ "package": { "name": l.name, "ecosystem": l.ecosystem }, "version": l.version })).collect();
        let answer: Value = agent
            .post(&endpoints.osv_batch)
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
                wanted.push((
                    format!("{} {}", library.name, library.version),
                    vuln["id"].as_str().unwrap_or_default().to_string(),
                ));
            }
        }
    }
    if wanted.len() > OSV_MAX_FETCH {
        notes.push(format!(
            "OSV.dev: only the first {OSV_MAX_FETCH} of {} recent vulnerabilities were fetched.",
            wanted.len()
        ));
        wanted.truncate(OSV_MAX_FETCH);
    }
    let fetched = fetch_osv(agent, endpoints, &wanted, since, notes);
    Ok(fetched)
}

fn fetch_osv(
    agent: &ureq::Agent,
    endpoints: &Endpoints,
    wanted: &[(String, String)],
    since: &str,
    notes: &mut Vec<String>,
) -> Vec<Advisory> {
    if wanted.is_empty() {
        return Vec::new();
    }
    let workers = OSV_WORKERS.min(wanted.len());
    let chunk_size = wanted.len().div_ceil(workers);
    let results: Vec<(Vec<Advisory>, Vec<String>)> = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for group in wanted.chunks(chunk_size) {
            let agent = agent.clone();
            let base = endpoints.osv_vuln.clone();
            handles.push(scope.spawn(move || {
                let mut found = Vec::new();
                let mut errors = Vec::new();
                for (technology, id) in group {
                    let fetched = (|| -> Result<Option<Advisory>> {
                        let full: Value = agent
                            .get(&format!("{base}{id}"))
                            .call()
                            .map_err(|e| anyhow::anyhow!("{e}"))?
                            .body_mut()
                            .read_json()
                            .context(id.to_string())?;
                        let published = full["published"].as_str().unwrap_or_default();
                        if published < since {
                            return Ok(None);
                        }
                        let summary = full["summary"]
                            .as_str()
                            .or(full["details"].as_str())
                            .unwrap_or_default();
                        Ok(Some(Advisory {
                            source: "OSV.dev".into(),
                            id: id.clone(),
                            technology: technology.clone(),
                            published: published.into(),
                            summary: summary.chars().take(400).collect(),
                        }))
                    })();
                    match fetched {
                        Ok(Some(a)) => found.push(a),
                        Ok(None) => {}
                        Err(e) => errors.push(format!("OSV.dev could not fetch {id}: {e:#}")),
                    }
                }
                (found, errors)
            }));
        }
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_default())
            .collect()
    });
    let mut found = Vec::new();
    for (mut f, errors) in results {
        found.append(&mut f);
        notes.extend(errors);
    }
    found
}

fn kev(
    agent: &ureq::Agent,
    endpoints: &Endpoints,
    spec: &Spec,
    since: &str,
) -> Result<Vec<Advisory>> {
    let feed: Value = agent.get(&endpoints.kev).call()?.body_mut().read_json()?;
    Ok(filter_kev(&feed, spec, since))
}

pub fn kev_matches(vendor: &str, product: &str, spec: &Spec) -> bool {
    let needles: Vec<String> = [vendor, product]
        .iter()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    if needles.is_empty() {
        return false;
    }
    let mut names: Vec<String> = spec
        .libraries
        .iter()
        .map(|l| l.name.to_lowercase())
        .collect();
    names.extend(spec.languages.iter().map(|l| l.name.to_lowercase()));
    names.push(spec.os.to_lowercase());
    names.iter().any(|name| {
        needles
            .iter()
            .any(|n| name.contains(n.as_str()) || n.contains(name.as_str()))
    })
}

pub fn filter_kev(feed: &Value, spec: &Spec, since: &str) -> Vec<Advisory> {
    feed["vulnerabilities"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|v| v["dateAdded"].as_str().unwrap_or("") >= &since[..10.min(since.len())])
        .filter(|v| {
            kev_matches(
                v["vendorProject"].as_str().unwrap_or_default(),
                v["product"].as_str().unwrap_or_default(),
                spec,
            )
        })
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
        .collect()
}

fn date_days_ago(days: u64) -> String {
    crate::time::rfc3339((crate::time::now() / 86_400).saturating_sub(days) * 86_400)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kev_is_filtered_by_the_spec() {
        let spec = crate::scan::Spec {
            os: "Linux".into(),
            languages: vec![crate::scan::Language {
                name: "Rust".into(),
                lines: 1,
                share: 100.0,
            }],
            libraries: vec![crate::scan::Library {
                name: "openssl".into(),
                version: "3".into(),
                ecosystem: String::new(),
            }],
        };
        let feed = serde_json::json!({"vulnerabilities":[{"cveID":"CVE-1","vendorProject":"OpenSSL","product":"openssl","dateAdded":"2026-11-01","shortDescription":"a"},{"cveID":"CVE-2","vendorProject":"Other","product":"unrelated","dateAdded":"2026-11-01","shortDescription":"b"}]});
        let found = filter_kev(&feed, &spec, "2026-10-26T00:00:00Z");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "CVE-1");
    }

    #[test]
    fn osv_a_failed_fetch_is_a_note_not_a_failure() {
        use std::io::{Read as _, Write as _};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buf = [0u8; 8192];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let (status, body) = if req.contains("/v1/querybatch") {
                    ("200 OK", r#"{"results":[{"vulns":[{"id":"GOOD-1","modified":"2999-01-01"},{"id":"BAD-1","modified":"2999-01-01"}]}]}"#.to_string())
                } else if req.contains("BAD-1") {
                    ("500 Internal Server Error", "{}".to_string())
                } else {
                    (
                        "200 OK",
                        r#"{"published":"2999-01-01","summary":"bad thing"}"#.to_string(),
                    )
                };
                let _ = stream.write_all(format!("HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).as_bytes());
            }
        });
        let endpoints = Endpoints {
            osv_batch: format!("http://{addr}/v1/querybatch"),
            osv_vuln: format!("http://{addr}/v1/vulns/"),
            kev: format!("http://{addr}/kev"),
        };
        let spec = crate::scan::Spec {
            os: "Linux".into(),
            languages: vec![],
            libraries: vec![crate::scan::Library {
                name: "lib".into(),
                version: "1".into(),
                ecosystem: "npm".into(),
            }],
        };
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .build()
            .new_agent();
        let mut notes = Vec::new();
        let found = osv(
            &agent,
            &endpoints,
            &spec,
            "2000-01-01T00:00:00Z",
            &mut notes,
        )
        .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "GOOD-1");
        assert!(notes.iter().any(|n| n.contains("BAD-1")), "{notes:?}");
        let _ = server.join();
    }

    #[test]
    fn dates_compare_as_text() {
        assert_eq!(date_days_ago(0).len(), 20);
        assert_eq!(
            crate::time::rfc3339(20_759 * 86_400),
            "2026-11-02T00:00:00Z"
        );
        assert!(date_days_ago(7) < date_days_ago(0));
        assert!("2026-11-01T10:00:00Z" >= "2026-10-26T00:00:00Z");
    }
}
