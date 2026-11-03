use keys::{MasterKey, MemoryKey, MemoryPresence, Platform};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use vault::{Config, NoVetter, Options, Outcome, Running};

static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

struct Test {
    dir: tempfile::TempDir,
    admin: PathBuf,
    running: Option<Running>,
}

impl Drop for Test {
    fn drop(&mut self) {
        if let Some(running) = self.running.take() {
            running.stop();
        }
    }
}

fn admin(path: &Path, method: &str, target: &str, body: Value) -> (u16, Value) {
    let body = body.to_string();
    let mut stream = UnixStream::connect(path).unwrap();
    write!(
        stream,
        "{method} {target} HTTP/1.1\r\nHost: v\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    let mut all = String::new();
    stream.read_to_string(&mut all).unwrap();
    let (head, body) = all.split_once("\r\n\r\n").unwrap();
    let status = head.split(' ').nth(1).unwrap().parse().unwrap();
    (status, serde_json::from_str(body).unwrap())
}

fn start() -> Test {
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("v");
    let admin_path = state.join("admin.sock");
    let mut config = Config::new(state, admin_path.clone());
    config.sockets = dir.path().join("s");
    let master = MemoryKey::default();
    master.create().unwrap();
    let platform = Platform {
        master: Box::new(master),
        presence: Box::new(MemoryPresence { present: true }),
    };
    let running = vault::start(config, platform, |_| Box::new(NoVetter)).unwrap();
    let put = json!({ "secret": "s3cret", "approved_urls": ["https://h.example/"] });
    assert_eq!(admin(&admin_path, "PUT", "/keys/svc", put).0, 200);
    assert_eq!(
        admin(&admin_path, "POST", "/sandboxes", json!({ "name": "host" })).0,
        200
    );
    Test {
        dir,
        admin: admin_path,
        running: Some(running),
    }
}

fn with(test: &Test, args: &[&str]) -> i32 {
    let opts = Options {
        admin: test.admin.clone(),
        state: test.dir.path().join("v"),
        sockets: test.dir.path().join("s"),
        ask_passphrase: false,
        serve: Box::new(|_| Ok(())),
    };
    let matches = vault::command()
        .try_get_matches_from(["vault", "with", "--sandbox", "host"].iter().chain(args))
        .unwrap();
    match vault::outcome(&matches, &opts).unwrap() {
        Outcome::Code(code) => code,
        Outcome::Extra(name, _) => panic!("extra {name}"),
    }
}

fn tokens(test: &Test) -> usize {
    admin(&test.admin, "GET", "/tokens", json!({})).1["tokens"]
        .as_array()
        .unwrap()
        .len()
}

#[test]
fn the_command_gets_a_token_that_ends_with_it() {
    let _one = ONE_AT_A_TIME.lock().unwrap();
    let test = start();
    let out = test.dir.path().join("out");
    let script = format!(
        "printf '%s %s' \"$AI2_VAULT_TOKEN\" \"$AI2_VAULT_ENDPOINT\" > {}; exit 3",
        out.display()
    );
    let code = with(
        &test,
        &[
            "svc",
            "--intent",
            "read the test data",
            "--",
            "sh",
            "-c",
            &script,
        ],
    );
    assert_eq!(code, 3);
    let seen = std::fs::read_to_string(&out).unwrap();
    let (token, endpoint) = seen.split_once(' ').unwrap();
    assert!(token.starts_with("vt_"));
    assert!(endpoint.starts_with("unix:") && endpoint.ends_with("host/gateway.sock"));
    assert_eq!(tokens(&test), 0);
    assert_eq!(with(&test, &["svc", "--", "true"]), vault::exit::USAGE);
    assert_eq!(
        with(&test, &["nope", "--intent", "x", "--", "true"]),
        vault::exit::ERROR
    );
    assert_eq!(
        with(&test, &["svc", "--mode", "app", "--", "sh", "-c", "exit 0"]),
        vault::exit::OK
    );
    assert_eq!(tokens(&test), 0);
}

#[test]
fn a_term_signal_goes_to_the_command_and_the_token_is_still_revoked() {
    let _one = ONE_AT_A_TIME.lock().unwrap();
    let test = start();
    let ready = test.dir.path().join("ready");
    let script = format!(
        "trap 'exit 7' TERM; touch {}; while :; do sleep 0.05; done",
        ready.display()
    );
    let signal = {
        let ready = ready.clone();
        std::thread::spawn(move || {
            while !ready.exists() {
                std::thread::sleep(Duration::from_millis(10));
            }
            unsafe { libc::kill(libc::getpid(), libc::SIGTERM) };
        })
    };
    let code = with(&test, &["svc", "--intent", "x", "--", "sh", "-c", &script]);
    signal.join().unwrap();
    assert_eq!(code, 7);
    assert_eq!(tokens(&test), 0);
}
