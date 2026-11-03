use ai2::harness::Finding;
use ai2::report::{html, Card, Review};
use ai2::scan::{self, Spec};
use ai2::steps::{technologies, weight, Factor, Rating, MSR, RRF};
use std::path::{Path, PathBuf};

const DOCUMENTS: &[&str] = &["clean", "script", "hidden"];

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn spec_of(name: &str) -> Spec {
    let mut notes = Vec::new();
    scan::scan(&fixture(name), None, &mut notes)
}

fn finding(technology: &str, severity: &str) -> Finding {
    serde_json::from_value(
        serde_json::json!({ "title": "x", "technology": technology, "severity": severity }),
    )
    .unwrap()
}

fn card(key: &'static str, rating: Option<Rating>, missing: Option<&str>) -> Card {
    let scale = if key == "msr" { &MSR } else { &RRF };
    Card {
        key,
        name: scale.name,
        short: scale.short,
        riskier: scale.riskier,
        rating,
        missing: missing.map(String::from),
        items: Vec::new(),
    }
}

fn review(spec: Spec, ratings: Vec<Card>, findings: Vec<Finding>) -> Review {
    let risks: Vec<_> = findings
        .iter()
        .map(|f| (f.technology.clone(), weight(&f.severity)))
        .collect();
    Review {
        version: "test",
        folder: "<fixtures>".into(),
        objective: "Summarise the report".into(),
        generated_at: String::new(),
        harness: None,
        ratings,
        technologies: technologies(&spec, &risks),
        exploits: Vec::new(),
        findings,
        spec,
        notes: Vec::new(),
    }
}

#[test]
fn every_document_is_read_as_html_and_css() {
    for name in DOCUMENTS {
        let spec = spec_of(name);
        let names: Vec<_> = spec.languages.iter().map(|l| l.name.as_str()).collect();
        assert!(names.contains(&"HTML"), "{name}: {names:?}");
        assert!(names.contains(&"CSS"), "{name}: {names:?}");
        let total: f64 = spec.languages.iter().map(|l| l.share).sum();
        assert!((total - 100.0).abs() < 0.2, "{name}: {total}");
        assert!(spec.libraries.is_empty(), "{name}");
    }
}

#[test]
fn risk_goes_to_the_technology_its_findings_are_in() {
    let list = technologies(
        &spec_of("hidden"),
        &[
            ("HTML".into(), weight("high")),
            ("CSS".into(), weight("high")),
            ("CSS".into(), weight("low")),
        ],
    );
    let risk = |name: &str| list.iter().find(|t| t.name == name).map(|t| t.risk);
    assert_eq!(risk("HTML"), Some(46.7));
    assert_eq!(risk("CSS"), Some(53.3));
    assert_eq!(list[0].name, "CSS");
    assert!(list
        .iter()
        .filter(|t| t.kind == "os")
        .all(|t| t.risk == 0.0));
}

#[test]
fn a_clean_document_has_no_risk_to_share() {
    let list = technologies(&spec_of("clean"), &[]);
    assert!(list.iter().all(|t| t.risk == 0.0));
}

#[test]
fn the_dashboard_shows_only_the_steps_it_ran() {
    let rating = RRF.rate(
        &serde_json::from_value(serde_json::json!({
            "impact": { "score": 80, "why": "<script> runs" },
            "likelihood": { "score": 60 },
            "share": { "score": 100 }
        }))
        .unwrap(),
    );
    let page = html(&review(
        spec_of("script"),
        vec![
            card("msr", None, Some("OSV.dev could not be searched")),
            card("rrf", Some(rating), None),
        ],
        vec![finding("HTML", "high")],
    ));
    assert!(page.contains("Modern Security Rating") && page.contains("Run Risk Factor"));
    assert!(page.contains("OSV.dev could not be searched"));
    assert!(page.contains("80%") && page.contains("&lt;script&gt; runs"));
    assert!(page.contains("&lt;fixtures&gt;") && !page.contains("<fixtures>"));
    assert_eq!(page.matches("<article class=\"rating\">").count(), 2);
    assert!(page.contains("--w:100%"));
}

#[test]
fn the_report_opened_as_a_file_says_to_run_ai2_ui() {
    let page = html(&review(
        spec_of("clean"),
        vec![card("msr", None, None)],
        Vec::new(),
    ));
    assert!(page.contains("<label for=\"tab-vault\">Vault</label>"));
    assert!(page.contains("<section id=\"vault\""));
    assert!(page.contains("Run <code>ai2 ui</code>"));
    assert!(!page.contains("<script"));
    assert!(page.contains("Security review") && page.contains("Modern Security Rating"));
}

#[test]
fn a_review_lists_its_ratings_in_step_order() {
    let json = serde_json::to_value(review(
        spec_of("clean"),
        vec![card("msr", None, None), card("rrf", None, None)],
        Vec::new(),
    ))
    .unwrap();
    let keys: Vec<_> = json["ratings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["key"].as_str().unwrap())
        .collect();
    assert_eq!(keys, ["msr", "rrf"]);
    let factor: Factor =
        serde_json::from_value(serde_json::json!({ "name": "x", "score": 1, "why": "" })).unwrap();
    assert_eq!(factor.score, 1.0);
}

#[test]
fn without_the_dashboard_only_the_review_is_written() {
    let out = std::env::temp_dir().join(format!("ai2-out-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let make = || review(spec_of("clean"), vec![card("msr", None, None)], Vec::new());
    let written = ai2::cli::write(&make(), &out, false).unwrap();
    assert_eq!(written, out.join("review.json"));
    assert!(!out.join("report.html").exists());
    assert_eq!(
        ai2::cli::write(&make(), &out, true).unwrap(),
        out.join("report.html")
    );
    assert!(out.join("report.html").exists());
    std::fs::remove_dir_all(&out).unwrap();
}
