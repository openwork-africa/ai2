use ai2::harness::Finding;
use ai2::report::{Card, Review};
use ai2::scan::{Language, Library, Spec};
use ai2::step::Item;
use ai2::steps::{Exploit, Factor, Rating, Technology};

fn keys(value: &serde_json::Value) -> Vec<String> {
    let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    keys
}

#[test]
fn review_json_has_the_shape_me_reads() {
    let review = Review {
        version: "0.1.1",
        folder: "/tmp/x".into(),
        objective: "do it".into(),
        generated_at: "2026-11-02T00:00:00Z".into(),
        harness: Some("pi".into()),
        ratings: vec![Card {
            key: "msr",
            name: "Modern Security Rating",
            short: "MSR",
            riskier: false,
            rating: Some(Rating {
                name: "Modern Security Rating".into(),
                short: "MSR".into(),
                score: 90.0,
                factors: vec![Factor { name: "Patched".into(), score: 90.0, why: "ok".into() }],
            }),
            missing: Some("note".into()),
            items: vec![Item { index: 0, text: "t".into(), score: 1.0, factors: vec![] }],
        }],
        technologies: vec![Technology { name: "Rust".into(), kind: "language", risk: 1.0 }],
        exploits: vec![Exploit { id: "CVE-1".into(), technology: "Rust".into(), severity: "high".into(), summary: "s".into(), fixed_in: "1.1".into() }],
        findings: vec![serde_json::from_value(serde_json::json!({"title":"t","technology":"Rust","file":"a.rs","severity":"high","fix":"fix it","status":"open"})).unwrap()],
        spec: Spec {
            os: "Linux".into(),
            languages: vec![Language { name: "Rust".into(), lines: 3, share: 100.0 }],
            libraries: vec![Library { name: "serde".into(), version: "1".into(), ecosystem: "crates.io".into() }],
        },
        notes: vec!["n".into()],
    };
    let _: &Finding = &review.findings[0];
    let json = serde_json::to_value(&review).unwrap();
    assert!(json.get("version").is_some() && json.get("notes").is_some());
    assert_eq!(
        keys(&json["ratings"][0]),
        ["items", "key", "missing", "name", "rating", "riskier", "short"]
    );
    assert_eq!(keys(&json["technologies"][0]), ["kind", "name", "risk"]);
    for field in ["title", "file", "severity", "status", "fix"] {
        assert!(json["findings"][0].get(field).is_some(), "{field}");
    }
    assert_eq!(keys(&json["spec"]), ["languages", "libraries", "os"]);
}
