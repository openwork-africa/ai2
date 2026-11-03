use crate::model::{ApiState, IconUpload, NewApp, Restart};
use crate::port::{detect, strip_jsonc};
use crate::store::Store;
use std::net::TcpListener;
use std::path::Path;
use tempfile::TempDir;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
const DOCUMENT: &str = r#"{"openapi":"3.1.0","info":{"title":"Fake","version":"0.3.0"},"paths":{"/a":{"get":{},"post":{}},"/b":{"delete":{}}}}"#;

struct Home {
    _temp: TempDir,
    store: Store,
    app_dir: std::path::PathBuf,
}

fn home() -> Home {
    let temp = TempDir::new().unwrap();
    let store = Store::open(&temp.path().join("apps")).unwrap();
    let app_dir = temp.path().join("app");
    std::fs::create_dir_all(&app_dir).unwrap();
    std::fs::write(app_dir.join("openapi.json"), DOCUMENT).unwrap();
    std::fs::write(app_dir.join("icon.png"), PNG).unwrap();
    Home {
        _temp: temp,
        store,
        app_dir,
    }
}

fn new_app(home: &Home, name: &str, command: &str, port: u16) -> NewApp {
    NewApp {
        name: name.to_string(),
        description: "A fake app for tests".to_string(),
        command: command.to_string(),
        openapi: "/openapi.json".to_string(),
        icon: Some(IconUpload {
            path: Some(home.app_dir.join("icon.png").to_string_lossy().into_owned()),
            ..IconUpload::default()
        }),
        cwd: Some(home.app_dir.to_string_lossy().into_owned()),
        port: Some(port),
        ..NewApp::default()
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[test]
fn register_stores_the_app_its_icon_and_a_unique_id() {
    let home = home();
    let (app, created) = home
        .store
        .register(new_app(&home, "Work spot", "deno task start", 7791))
        .unwrap();
    assert!(created);
    assert_eq!(app.id, "work-spot");
    assert_eq!(app.url, "http://localhost:7791");
    assert!(app.set_port);
    assert_eq!(app.restart, Restart::Never);
    let icon = app.icon.clone().unwrap();
    assert_eq!(icon.content_type, "image/png");
    assert_eq!(icon.file, "icons/work-spot.png");
    assert_eq!(icon.bytes, PNG.len() as u64);
    assert_eq!(
        home.store.icon("work-spot").unwrap().unwrap(),
        ("image/png".to_string(), PNG.to_vec())
    );

    let mut again = new_app(&home, "Work spot", "deno task serve", 7792);
    again.icon = None;
    let (updated, created) = home.store.register(again).unwrap();
    assert!(!created);
    assert_eq!(updated.id, "work-spot");
    assert_eq!(updated.command, "deno task serve");
    assert_eq!(updated.added, app.added);
    assert_eq!(updated.icon, app.icon);

    let (other, _) = home
        .store
        .register(new_app(&home, "work-spot", "x", 7793))
        .unwrap();
    assert_eq!(other.id, "work-spot-2");

    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(home.store.dir().join("apps.json")).unwrap())
            .unwrap();
    assert_eq!(saved.as_array().unwrap().len(), 2);

    home.store.remove("work-spot").unwrap();
    assert!(!home.store.dir().join("icons/work-spot.png").exists());
    assert_eq!(home.store.apps().unwrap().len(), 1);
    assert_eq!(home.store.app("work-spot").unwrap_err().status, 404);
}

#[test]
fn update_changes_only_what_is_sent() {
    let home = home();
    home.store
        .register(new_app(&home, "Patch", "run", 7000))
        .unwrap();
    let app = home
        .store
        .update(
            "patch",
            crate::AppPatch {
                port: Some(7001),
                restart: Some(Restart::Always),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(app.port, 7001);
    assert_eq!(app.url, "http://localhost:7001");
    assert_eq!(app.restart, Restart::Always);
    assert_eq!(app.command, "run");
    let refused = home
        .store
        .update(
            "patch",
            crate::AppPatch {
                openapi: Some(" ".into()),
                ..Default::default()
            },
        )
        .unwrap_err();
    assert_eq!(refused.error, "openapi_required");
}

#[test]
fn register_refuses_an_app_without_openapi_icon_or_description() {
    let home = home();
    let mut app = new_app(&home, "A", "run", 7000);
    app.openapi = "  ".into();
    let refused = home.store.register(app).unwrap_err();
    assert_eq!(
        (refused.status, refused.error.as_str()),
        (422, "openapi_required")
    );

    let mut app = new_app(&home, "A", "run", 7000);
    app.icon = None;
    let refused = home.store.register(app).unwrap_err();
    assert_eq!(
        (refused.status, refused.error.as_str()),
        (422, "icon_required")
    );

    let mut app = new_app(&home, "A", "run", 7000);
    app.description = String::new();
    assert_eq!(
        home.store.register(app).unwrap_err().error,
        "description_required"
    );

    let mut app = new_app(&home, "A", "run", 7000);
    app.cwd = Some("relative/dir".into());
    assert_eq!(home.store.register(app).unwrap_err().error, "cwd_required");

    let mut app = new_app(&home, "A", "run-something", 7000);
    app.port = None;
    assert_eq!(home.store.register(app).unwrap_err().error, "port_required");

    assert!(home.store.apps().unwrap().is_empty());

    let parsed = NewApp::parse(serde_json::json!({
        "name": "A", "description": "d", "command": "c"
    }))
    .unwrap_err();
    assert_eq!(
        (parsed.status, parsed.error.as_str()),
        (422, "openapi_required")
    );
    let parsed = NewApp::parse(serde_json::json!({
        "name": "A", "description": "d", "command": "c", "openapi": "/openapi.json",
        "icon": { "type": "image/png", "base64": "iVBORw0KGgo=" }
    }))
    .unwrap();
    assert_eq!(parsed.openapi, "/openapi.json");
}

#[test]
fn icons_are_sniffed_capped_and_svgs_sanitised() {
    use crate::icon::check;
    assert_eq!(check(None, PNG.to_vec()).unwrap().0, "image/png");
    assert_eq!(
        check(None, vec![0xff, 0xd8, 0xff, 0xe0]).unwrap().0,
        "image/jpeg"
    );
    assert_eq!(check(None, vec![0, 0, 1, 0, 1]).unwrap().0, "image/x-icon");
    let webp = b"RIFF\0\0\0\0WEBPVP8 ".to_vec();
    assert_eq!(check(None, webp).unwrap().0, "image/webp");
    let svg =
        br##"<svg xmlns="http://www.w3.org/2000/svg"><use href="#a"/><rect fill="red"/></svg>"##;
    assert_eq!(check(None, svg.to_vec()).unwrap().0, "image/svg+xml");

    assert_eq!(
        check(None, b"hello".to_vec()).unwrap_err().error,
        "icon_type"
    );
    assert_eq!(
        check(Some("image/jpeg"), PNG.to_vec()).unwrap_err().error,
        "icon_type"
    );
    let mut big = PNG.to_vec();
    big.resize(600 * 1024, 0);
    assert_eq!(check(None, big).unwrap_err().status, 413);
    for bad in [
        r#"<svg><script>alert(1)</script></svg>"#,
        r#"<svg onload="alert(1)"></svg>"#,
        r#"<svg><image href="https://evil.example/x.png"/></svg>"#,
        r#"<svg><style>rect{fill:url(https://evil.example/x)}</style></svg>"#,
        r#"<svg><a xlink:href="javascript:alert(1)"/></svg>"#,
    ] {
        let refused = check(None, bad.as_bytes().to_vec()).unwrap_err();
        assert_eq!(
            (refused.status, refused.error.as_str()),
            (400, "icon_unsafe"),
            "{bad}"
        );
    }
}

#[test]
fn ports_are_detected_as_the_launcher_did() {
    let temp = TempDir::new().unwrap();
    let dir = temp.path();
    assert_eq!(detect(dir, "deno run -A main.ts --port 8000"), Some(8000));
    assert_eq!(detect(dir, "deno run --port=8001 main.ts"), Some(8001));
    assert_eq!(
        detect(dir, "ZAZI_PORT=8790 cargo run --release"),
        Some(8790)
    );
    assert_eq!(detect(dir, "python3 -m http.server -p 8002"), Some(8002));
    assert_eq!(detect(dir, "npx vite"), Some(5173));
    assert_eq!(detect(dir, "npx vite build"), None);
    std::fs::write(
        dir.join("vite.config.ts"),
        "export default { server: { port: 5199 } }",
    )
    .unwrap();
    assert_eq!(detect(dir, "deno run -A npm:vite"), Some(5199));

    std::fs::write(dir.join("a.ts"), "Deno.serve({ port: 8123 }, h)").unwrap();
    std::fs::write(
        dir.join("b.ts"),
        r#"const p = Number(Deno.env.get("PORT") ?? 8124)"#,
    )
    .unwrap();
    std::fs::write(dir.join("c.js"), "app.listen( 8125 )").unwrap();
    assert_eq!(detect(dir, "deno run -A a.ts"), Some(8123));
    assert_eq!(detect(dir, "deno run -A b.ts start"), Some(8124));
    assert_eq!(detect(dir, "node c.js"), Some(8125));
    assert_eq!(detect(dir, "deno run -A missing.ts"), None);

    std::fs::write(
        dir.join("deno.jsonc"),
        r#"{
            // the tasks
            "tasks": {
                "build": "deno run --port 9999 build.ts",
                "start": "deno task build && deno task serve", /* serve it */
                "serve": { "command": "deno run -A a.ts" },
            },
        }"#,
    )
    .unwrap();
    assert_eq!(detect(dir, "deno task start"), Some(8123));
    assert_eq!(detect(dir, "deno task build"), None);
}

#[test]
fn jsonc_comments_and_trailing_commas_are_stripped() {
    let text = r#"{ "a": "x // not a comment", /* c */ "b": [1, 2,], } // end"#;
    let value: serde_json::Value = serde_json::from_str(&strip_jsonc(text)).unwrap();
    assert_eq!(
        value,
        serde_json::json!({ "a": "x // not a comment", "b": [1, 2] })
    );
}

#[test]
fn the_openapi_check_marks_valid_unreachable_and_invalid() {
    let home = home();
    let file = home.app_dir.join("openapi.json");
    let mut app = new_app(&home, "Doc", "run", free_port());
    app.openapi = file.to_string_lossy().into_owned();
    let (app, _) = home.store.register(app).unwrap();
    assert_eq!(home.store.api(&app).state, ApiState::Unchecked);
    let api = crate::openapi::check(&home.store, &app).unwrap();
    assert_eq!(api.state, ApiState::Valid);
    assert_eq!(api.title.as_deref(), Some("Fake"));
    assert_eq!(api.version.as_deref(), Some("0.3.0"));
    assert_eq!(api.operations, Some(3));
    assert_eq!(api.href, "/api/apps/doc/openapi.json");
    assert_eq!(home.store.api(&app), api);
    assert_eq!(home.store.document("doc").unwrap()["info"]["title"], "Fake");

    let (closed, _) = home
        .store
        .register(new_app(&home, "Closed", "run", free_port()))
        .unwrap();
    let api = crate::openapi::check(&home.store, &closed).unwrap();
    assert_eq!(api.state, ApiState::Unreachable);
    assert!(api.error.is_some());
    assert!(home.store.document("closed").is_none());

    std::fs::write(&file, r#"{"openapi":"2.0","paths":{}}"#).unwrap();
    let api = crate::openapi::check(&home.store, &app).unwrap();
    assert_eq!(api.state, ApiState::Invalid);
    assert!(api.error.unwrap().contains("3.x"));

    std::fs::write(&file, "openapi: 3.1.0\npaths: {}\n").unwrap();
    let api = crate::openapi::check(&home.store, &app).unwrap();
    assert_eq!(api.state, ApiState::Invalid);
    assert!(api.error.unwrap().contains("serve it as JSON"));

    std::fs::write(&file, r#"{"openapi":"3.0.3"}"#).unwrap();
    let api = crate::openapi::check(&home.store, &app).unwrap();
    assert_eq!(
        api.error.as_deref(),
        Some("the document has no paths object")
    );
}

#[test]
fn the_store_uses_ai2_home_apps() {
    let temp = TempDir::new().unwrap();
    let store = Store::at_home(temp.path()).unwrap();
    assert_eq!(store.dir(), temp.path().join("apps"));
    for sub in ["icons", "logs", "openapi"] {
        assert!(Path::new(&store.dir().join(sub)).is_dir());
    }
}
