use super::{inject, valid_hash, Launched, NewReview};
use crate::tests::ui_without_daemon;
use std::path::Path;
use std::sync::Arc;

const WORKSPOT: &str = include_str!("fixtures/workspot.json");
const HARNESS: &str = include_str!("fixtures/harness.json");
const WORKSPOT_HASH: &str = "3f9a0c2b7d14e865";
const HARNESS_HASH: &str = "a01b2c3d4e5f6071";
const JSON: &[(&str, &str)] = &[("Content-Type", "application/json")];

fn home_with_reviews(home: &Path) {
    for (hash, review) in [(WORKSPOT_HASH, WORKSPOT), (HARNESS_HASH, HARNESS)] {
        let dir = home.join("reviews").join(hash);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("review.json"), review).unwrap();
    }
    std::fs::write(
        home.join("reviews").join(WORKSPOT_HASH).join("report.html"),
        "<!doctype html>",
    )
    .unwrap();
    let broken = home.join("reviews").join("0000000000000000");
    std::fs::create_dir_all(&broken).unwrap();
    std::fs::write(broken.join("review.json"), "not json").unwrap();
    let stray = home.join("reviews").join("notes");
    std::fs::create_dir_all(&stray).unwrap();
    std::fs::write(stray.join("review.json"), WORKSPOT).unwrap();
}

#[test]
fn the_list_has_every_review_newest_first_with_its_ratings() {
    let dir = tempfile::tempdir().unwrap();
    home_with_reviews(dir.path());
    let ui = ui_without_daemon(dir.path());
    let response = ui.api("GET", "/api/reviews", &[], "");
    assert_eq!(response.status, 200);
    let body = response.json();
    let reviews = body["reviews"].as_array().unwrap();
    assert_eq!(reviews.len(), 2);
    let first = &reviews[0];
    assert_eq!(first["hash"], WORKSPOT_HASH);
    assert_eq!(
        first["folder"],
        "/Users/rawk/Documents/openwork/example/workspot"
    );
    assert_eq!(
        first["objective"],
        "Add a pricing page with three plans and a Stripe checkout button"
    );
    assert_eq!(first["generated_at"], "2026-11-03T09:41:12Z");
    assert_eq!(first["harness"], "muse");
    assert_eq!(first["findings"], 3);
    assert_eq!(first["open_findings"], 1);
    assert_eq!(first["exploits"], 3);
    assert_eq!(first["report"], true);
    assert_eq!(
        first["ratings"],
        serde_json::json!([
            { "key": "msr", "name": "Modern Security Rating", "short": "MSR", "riskier": false, "score": 82.0 },
            { "key": "rrf", "name": "Run Risk Factor", "short": "RRF", "riskier": true, "score": 9.0 }
        ])
    );
    let second = &reviews[1];
    assert_eq!(second["hash"], HARNESS_HASH);
    assert_eq!(second["objective"], "");
    assert_eq!(second["harness"], serde_json::Value::Null);
    assert_eq!(second["ratings"][1]["score"], serde_json::Value::Null);
    assert_eq!(second["report"], false);
}

#[test]
fn an_empty_home_has_no_reviews() {
    let dir = tempfile::tempdir().unwrap();
    let ui = ui_without_daemon(dir.path());
    let response = ui.api("GET", "/api/reviews", &[], "");
    assert_eq!(response.status, 200);
    assert_eq!(response.json()["reviews"], serde_json::json!([]));
}

#[test]
fn one_review_is_its_review_json_as_it_is() {
    let dir = tempfile::tempdir().unwrap();
    home_with_reviews(dir.path());
    let ui = ui_without_daemon(dir.path());
    let response = ui.api("GET", &format!("/api/reviews/{WORKSPOT_HASH}"), &[], "");
    assert_eq!(response.status, 200);
    let expected: serde_json::Value = serde_json::from_str(WORKSPOT).unwrap();
    assert_eq!(response.json(), expected);
    let missing = ui.api("GET", "/api/reviews/ffffffffffffffff", &[], "");
    assert_eq!(missing.status, 404);
    assert_eq!(missing.json()["error"], "not_found");
    let broken = ui.api("GET", "/api/reviews/0000000000000000", &[], "");
    assert_eq!(broken.status, 404);
    for bad in ["notes", "..%2F..%2Fetc", "3F9A0C2B7D14E865"] {
        let response = ui.api("GET", &format!("/api/reviews/{bad}"), &[], "");
        assert_eq!(response.status, 400, "{bad}");
    }
}

#[test]
fn reviews_need_the_nonce() {
    let dir = tempfile::tempdir().unwrap();
    home_with_reviews(dir.path());
    let ui = ui_without_daemon(dir.path());
    let response = ui.api("GET", "/api/reviews", &[("X-Ai2-UI-Nonce", "wrong")], "");
    assert_eq!(response.status, 403);
}

#[test]
fn hashes_are_sixteen_lowercase_hex_digits() {
    assert!(valid_hash("3f9a0c2b7d14e865"));
    assert!(!valid_hash("3f9a0c2b7d14e86"));
    assert!(!valid_hash("3F9A0C2B7D14E865"));
    assert!(!valid_hash("3f9a0c2b7d14e86g"));
    assert!(!valid_hash("../../etc/passwd"));
}

#[test]
fn the_document_describes_the_review_routes() {
    let dir = tempfile::tempdir().unwrap();
    let ui = ui_without_daemon(dir.path());
    let document = ui.api("GET", "/openapi.json", &[], "").json();
    let paths = &document["paths"];
    assert!(paths["/api/reviews"]["get"].is_object());
    assert!(paths["/api/reviews"]["post"].is_object());
    assert!(paths["/api/reviews/{hash}"]["get"].is_object());
    assert!(paths["/api/reviews/jobs/{id}/events"]["get"].is_object());
    let schemas = &document["components"]["schemas"];
    for name in [
        "Reviews",
        "Summary",
        "Score",
        "NewReview",
        "Started",
        "JobEvent",
    ] {
        assert!(schemas[name].is_object(), "{name}");
    }
}

#[test]
fn a_review_starts_only_through_the_injected_launcher_and_streams_its_progress() {
    let dir = tempfile::tempdir().unwrap();
    let ui = ui_without_daemon(dir.path());
    let body =
        serde_json::json!({ "folder": dir.path(), "objective": "Add a pricing page" }).to_string();
    let refused = ui.api("POST", "/api/reviews", JSON, &body);
    assert_eq!(refused.status, 501);
    assert_eq!(refused.json()["error"], "not_available");
    assert_eq!(
        ui.api("GET", "/api/reviews", &[], "").json()["can_start"],
        false
    );

    let fixture = dir.path().join("fixture.json");
    std::fs::write(&fixture, WORKSPOT).unwrap();
    let fixture = fixture.display().to_string();
    assert!(inject(Arc::new(move |request: &NewReview| {
        let out = Path::new(&request.folder)
            .join("reviews")
            .join(WORKSPOT_HASH);
        let mut command = std::process::Command::new("sh");
        command
            .arg("-c")
            .arg("echo \"ai2: $3\" >&2; echo 'ai2: rating the Modern Security Rating' >&2; mkdir -p \"$1\" && cp \"$2\" \"$1/review.json\"")
            .arg("sh")
            .arg(&out)
            .arg(&fixture)
            .arg(request.objective.clone().unwrap_or_default());
        Ok(Launched {
            command,
            hash: WORKSPOT_HASH.to_string(),
        })
    })));
    assert_eq!(
        ui.api("GET", "/api/reviews", &[], "").json()["can_start"],
        true
    );

    let empty = ui.api("POST", "/api/reviews", JSON, r#"{"folder":"  "}"#);
    assert_eq!(empty.status, 400);

    let started = ui.api("POST", "/api/reviews", JSON, &body);
    assert_eq!(started.status, 202);
    let started = started.json();
    assert_eq!(started["hash"], WORKSPOT_HASH);
    let job = started["job"].as_str().unwrap();

    let events = ui.api("GET", &format!("/api/reviews/jobs/{job}/events"), &[], "");
    assert_eq!(events.status, 200);
    let text = events.text();
    assert!(text.contains("event: log"), "{text}");
    assert!(text.contains("ai2: Add a pricing page"), "{text}");
    assert!(
        text.contains("ai2: rating the Modern Security Rating"),
        "{text}"
    );
    assert!(text.contains("event: done"), "{text}");
    assert!(text.contains(r#""ok":true"#), "{text}");

    let replay = ui.api(
        "GET",
        &format!("/api/reviews/jobs/{job}/events"),
        &[("Last-Event-ID", "1")],
        "",
    );
    let replay = replay.text();
    assert!(!replay.contains("event: log"), "{replay}");
    assert!(replay.contains("event: done"), "{replay}");

    let list = ui.api("GET", "/api/reviews", &[], "").json();
    assert_eq!(list["reviews"][0]["hash"], WORKSPOT_HASH);
    let unknown = ui.api("GET", "/api/reviews/jobs/zz/events", &[], "");
    assert_eq!(unknown.status, 404);
}
