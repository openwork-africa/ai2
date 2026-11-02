use crate::model::Model;
use crate::prompts;
use crate::scan::{percent, Spec};
use crate::sources::{self, Advisory};
use crate::step::{Context, Found, Rated, Step};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub struct Scale {
    pub key: &'static str,
    pub name: &'static str,
    pub short: &'static str,
    pub riskier: bool,
    pub missing: &'static str,
    pub factors: &'static [(&'static str, &'static str)],
}

pub const MSR: Scale = Scale {
    key: "msr",
    name: "Modern Security Rating",
    short: "MSR",
    riskier: false,
    missing: "Could not be rated: see the notes.",
    factors: &[
        ("unexposed", "Unexposed"),
        ("low_severity", "Low severity"),
        ("patched", "Patched"),
    ],
};
pub const RRF: Scale = Scale {
    key: "rrf",
    name: "Run Risk Factor",
    short: "RRF",
    riskier: true,
    missing: "Needs a coding harness: Muse Code (logged in) or pi.",
    factors: &[
        ("impact", "Impact"),
        ("likelihood", "Likelihood"),
        ("share", "Share of the run"),
    ],
};
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Scored {
    pub score: f64,
    #[serde(default)]
    pub why: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Factor {
    pub name: String,
    pub score: f64,
    pub why: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Rating {
    pub name: String,
    pub short: String,
    pub score: f64,
    pub factors: Vec<Factor>,
}

impl Scale {
    pub fn rate(&self, scored: &BTreeMap<String, Scored>) -> Rating {
        let factors: Vec<Factor> = self
            .factors
            .iter()
            .map(|(key, name)| {
                let given = scored.get(*key).cloned().unwrap_or(Scored {
                    score: 0.0,
                    why: "Not scored.".into(),
                });
                Factor {
                    name: name.to_string(),
                    score: clamp(given.score),
                    why: given.why,
                }
            })
            .collect();
        Rating {
            name: self.name.into(),
            short: self.short.into(),
            score: mean(factors.iter().map(|f| f.score)),
            factors,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Exploit {
    pub id: String,
    pub technology: String,
    pub severity: String,
    pub summary: String,
    #[serde(default)]
    pub fixed_in: String,
}

#[derive(Deserialize)]
struct MsrAnswer {
    exploits: Vec<Exploit>,
    factors: BTreeMap<String, Scored>,
}

pub struct Msr;

impl Step for Msr {
    fn id(&self) -> &'static str {
        "rating.msr"
    }
    fn scale(&self) -> &'static Scale {
        &MSR
    }

    fn rate(&self, ctx: &Context, _found: &Found) -> Result<Rated> {
        let mut notes = Vec::new();
        let advisories = sources::recent(&ctx.spec, ctx.days, &mut notes);
        let (rating, exploits) = msr(ctx.model.as_ref(), &ctx.spec, &advisories, ctx.days)?;
        Ok(Rated {
            rating: Some(rating),
            exploits,
            notes,
            ..Rated::default()
        })
    }
}

pub fn msr(
    model: Option<&Model>,
    spec: &Spec,
    advisories: &[Advisory],
    days: u64,
) -> Result<(Rating, Vec<Exploit>)> {
    if advisories.is_empty() {
        let why = format!("No exploit published in the last {days} days touches this technology.");
        let all = MSR
            .factors
            .iter()
            .map(|(key, _)| {
                (
                    key.to_string(),
                    Scored {
                        score: 100.0,
                        why: why.clone(),
                    },
                )
            })
            .collect();
        return Ok((MSR.rate(&all), Vec::new()));
    }
    let model = model.ok_or_else(|| {
        anyhow::anyhow!("a model is needed to match the advisories to the technology")
    })?;
    let capped = capped_spec(spec, advisories);
    let input = serde_json::to_string_pretty(
        &serde_json::json!({ "technology": capped, "advisories": advisories, "days": days }),
    )?;
    let answer: MsrAnswer = model.ask(prompts::MSR, &input)?;
    Ok((MSR.rate(&answer.factors), answer.exploits))
}

#[derive(Serialize, Clone)]
pub struct Technology {
    pub name: String,
    pub kind: &'static str,
    pub risk: f64,
}

pub fn weight(severity: &str) -> f64 {
    match severity.to_ascii_lowercase().as_str() {
        "critical" => 10.0,
        "high" => 7.0,
        "medium" => 4.0,
        "low" => 1.0,
        _ => 0.0,
    }
}

pub fn capped_spec(spec: &Spec, advisories: &[Advisory]) -> Spec {
    let named: Vec<String> = advisories
        .iter()
        .map(|a| a.technology.to_lowercase())
        .collect();
    let mut capped = spec.clone();
    capped.libraries.retain(|lib| {
        let name = lib.name.to_lowercase();
        !name.is_empty()
            && named.iter().any(|tech| {
                tech.split(|c: char| c.is_whitespace() || c == ',' || c == ';')
                    .any(|token| token == name)
            })
    });
    capped
}

pub fn technologies(spec: &Spec, risks: &[(String, f64)]) -> Vec<Technology> {
    let mut weights: BTreeMap<String, (String, f64)> = BTreeMap::new();
    for (name, w) in risks {
        weights
            .entry(name.to_lowercase())
            .or_insert((name.clone(), 0.0))
            .1 += w;
    }
    let total: f64 = weights.values().map(|(_, w)| w).sum();
    let mut take = |name: &str| {
        weights
            .remove(&name.to_lowercase())
            .map_or(0.0, |(_, w)| percent(w, total))
    };
    let mut list: Vec<Technology> = spec
        .languages
        .iter()
        .map(|l| Technology {
            name: l.name.clone(),
            kind: "language",
            risk: take(&l.name),
        })
        .collect();
    list.push(Technology {
        name: spec.os.clone(),
        kind: "os",
        risk: take(&spec.os),
    });
    let rest: Vec<(String, f64)> = weights.into_values().collect();
    list.extend(rest.into_iter().map(|(name, w)| Technology {
        name,
        kind: "library",
        risk: percent(w, total),
    }));
    list.sort_by(|a, b| b.risk.total_cmp(&a.risk));
    list
}

pub fn clamp(score: f64) -> f64 {
    score.clamp(0.0, 100.0)
}

pub fn mean(scores: impl Iterator<Item = f64>) -> f64 {
    let (sum, n) = scores.fold((0.0, 0), |(sum, n), s| (sum + s, n + 1));
    if n == 0 {
        0.0
    } else {
        (sum / n as f64).round()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rating_is_the_mean_of_its_factors() {
        let scored = BTreeMap::from([
            (
                "impact".to_string(),
                Scored {
                    score: 90.0,
                    why: String::new(),
                },
            ),
            (
                "likelihood".to_string(),
                Scored {
                    score: 150.0,
                    why: String::new(),
                },
            ),
        ]);
        let rating = RRF.rate(&scored);
        assert_eq!(
            rating.factors.iter().map(|f| f.score).collect::<Vec<_>>(),
            vec![90.0, 100.0, 0.0]
        );
        assert_eq!(rating.score, 63.0);
    }

    #[test]
    fn no_recent_advisory_is_a_full_rating_without_a_model() {
        let (rating, exploits) = msr(None, &Spec::default(), &[], 7).unwrap();
        assert_eq!(rating.score, 100.0);
        assert!(exploits.is_empty());
    }

    #[test]
    fn msr_input_for_a_large_spec_lists_only_advised_libraries() {
        let libraries = (0..3000)
            .map(|i| crate::scan::Library {
                name: format!("lib-{i}"),
                version: "1".into(),
                ecosystem: "npm".into(),
            })
            .collect::<Vec<_>>();
        let spec = Spec {
            os: "Linux".into(),
            languages: vec![crate::scan::Language {
                name: "Rust".into(),
                lines: 1,
                share: 100.0,
            }],
            libraries,
        };
        let advisories = vec![
            Advisory {
                source: "x".into(),
                id: "1".into(),
                technology: "lib-2 1.0".into(),
                published: "2026-01-01".into(),
                summary: String::new(),
            },
            Advisory {
                source: "x".into(),
                id: "2".into(),
                technology: "lib-2999".into(),
                published: "2026-01-01".into(),
                summary: String::new(),
            },
        ];
        let capped = capped_spec(&spec, &advisories);
        assert_eq!(
            capped
                .libraries
                .iter()
                .map(|l| l.name.as_str())
                .collect::<Vec<_>>(),
            ["lib-2", "lib-2999"]
        );
        assert_eq!(capped.languages.len(), 1);
        assert_eq!(capped.os, "Linux");
    }

    #[test]
    fn risk_is_shared_by_severity() {
        let spec = Spec {
            os: "Linux".into(),
            languages: vec![crate::scan::Language {
                name: "Rust".into(),
                lines: 10,
                share: 100.0,
            }],
            libraries: vec![],
        };
        let list = technologies(
            &spec,
            &[
                ("rust".into(), weight("high")),
                ("openssl 3.0".into(), weight("high")),
                ("openssl 3.0".into(), weight("informational")),
            ],
        );
        let risk: BTreeMap<_, _> = list.iter().map(|t| (t.name.as_str(), t.risk)).collect();
        assert_eq!(risk["Rust"], 50.0);
        assert_eq!(risk["openssl 3.0"], 50.0);
        assert_eq!(risk["Linux"], 0.0);
    }
}
