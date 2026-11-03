use crate::cli::{providers_file, Subcommand};
use crate::doctor::Row;
use crate::model::{Gateway, Model, Sent};
use anyhow::{anyhow, bail, Context, Result};
use clap::ArgMatches;
use serde_json::Value;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;
use vault::{KeyInfo, NoVetter, Outcome, Proposal, Resolver, Serve, Verdict, Vetter, Vetting};

pub struct ModelVetter(pub Model);

impl Vetter for ModelVetter {
    fn propose(&self, key: &KeyInfo) -> Result<Vec<String>> {
        Ok(self
            .0
            .ask::<Proposal>(vault::PROPOSE_SYSTEM, &key.input())?
            .prefixes)
    }
    fn vet(&self, call: &Vetting) -> Result<Verdict> {
        self.0.ask::<Verdict>(vault::VET_SYSTEM, &call.input())
    }
}

pub fn gateway(home: PathBuf) -> Gateway {
    Arc::new(move |sent: &Sent| {
        let token = vault::app::load(&home, sent.key).ok_or_else(|| {
            anyhow!(
                "there is no vault pass for {}: add the provider again on the Tools page",
                sent.key
            )
        })?;
        let body = serde_json::to_vec(sent.body)?;
        let mut headers: Vec<(&str, &str)> = sent
            .headers
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect();
        headers.push(("Content-Type", "application/json"));
        let reply = token.call(
            sent.key,
            &vault::app::Call {
                method: "POST",
                url: sent.url,
                headers: &headers,
                body: &body,
                wait: sent.timeout,
            },
        )?;
        Ok((
            reply.status,
            serde_json::from_slice(&reply.body).unwrap_or(Value::Null),
        ))
    })
}

#[cfg(feature = "harness")]
pub struct HarnessVetter {
    pub setup: ::harness::Setup,
    pub home: PathBuf,
    pub model: Option<String>,
    pub limit: Duration,
}

#[cfg(feature = "harness")]
pub const ANSWER_LINE: &str = "Write the JSON object alone to this file:";

#[cfg(feature = "harness")]
impl HarnessVetter {
    fn ask<T: serde::de::DeserializeOwned>(&self, system: &str, input: &str) -> Result<T> {
        use std::os::unix::fs::PermissionsExt;
        use std::process::{Command, Stdio};
        let dir = tempfile::Builder::new()
            .prefix("ai2-vet-")
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir()?;
        let out = dir.path().join("answer.json");
        let prompt = format!("{system}\n\n{input}\n\n{ANSWER_LINE}\n{}", out.display());
        let line = ::harness::task_line(
            &self.setup,
            &prompt,
            &dir.path().display().to_string(),
            self.model.as_deref(),
            None,
        );
        let path = ::harness::state::workspace_path(
            &self.home,
            &std::env::var("PATH").unwrap_or_default(),
        );
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(line)
            .env("PATH", path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("starting {}", self.setup.label))?;
        crate::rrf::wait(&mut child, &self.setup.label, self.limit)?;
        let text = std::fs::read_to_string(&out)
            .with_context(|| format!("{} gave no answer", self.setup.label))?;
        crate::model::parse(&text)
    }
}

#[cfg(feature = "harness")]
impl Vetter for HarnessVetter {
    fn propose(&self, key: &KeyInfo) -> Result<Vec<String>> {
        Ok(self
            .ask::<Proposal>(vault::PROPOSE_SYSTEM, &key.input())?
            .prefixes)
    }
    fn vet(&self, call: &Vetting) -> Result<Verdict> {
        self.ask::<Verdict>(vault::VET_SYSTEM, &call.input())
    }
}

pub struct VaultSubcommand {
    pub name: &'static str,
}

impl VaultSubcommand {
    pub fn new(name: &'static str) -> Self {
        Self { name }
    }

    pub(crate) fn options(&self) -> vault::Options {
        let name = self.name;
        vault::Options::new(
            &crate::cli::state_dir(name),
            Box::new(move |serve| run_serve(name, serve)),
        )
    }
}

impl Subcommand for VaultSubcommand {
    fn command(&self) -> clap::Command {
        vault::command_with(ui::commands())
    }
    fn run(&self, matches: &ArgMatches) -> Result<ExitCode> {
        let options = self.options();
        match vault::outcome(matches, &options)? {
            Outcome::Code(code) => Ok(ExitCode::from(code as u8)),
            Outcome::Extra(name, sub) => ui::run(
                crate::cli::Build {
                    name: self.name,
                    ..crate::cli::Build::ai2()
                },
                &name,
                &sub,
                &options,
            ),
        }
    }
    fn usage_exit(&self) -> Option<u8> {
        Some(vault::exit::USAGE as u8)
    }
}

#[cfg(feature = "ui")]
mod ui {
    use crate::cli::{default_out, exit_code, Build};
    use anyhow::{anyhow, bail, Context, Result};
    use clap::{value_parser, Arg, ArgMatches, Command};
    use serde_json::{json, Map};
    use std::path::{Path, PathBuf};
    use std::process::ExitCode;

    pub fn commands() -> Vec<Command> {
        vec![Command::new("ui")
            .about("Opens ai2 ui on its Vault page: keys, tokens, sandboxes, approvals and the audit log")
            .arg(
                Arg::new("report")
                    .long("report")
                    .value_name("PATH")
                    .value_parser(value_parser!(PathBuf))
                    .help("A reviewed folder, its out folder or its report.html: opens that review in ai2 ui instead"),
            )]
    }

    pub fn run(
        build: Build,
        name: &str,
        sub: &ArgMatches,
        options: &vault::Options,
    ) -> Result<ExitCode> {
        if name != "ui" {
            return Err(anyhow!("unknown vault subcommand {name}"));
        }
        let review = sub
            .get_one::<PathBuf>("report")
            .map(|path| review_dir(build, path))
            .transpose()?;
        let page = match &review {
            Some(dir) => format!(
                "reviews/{}",
                dir.file_name().unwrap_or_default().to_string_lossy()
            ),
            None => "vault".to_string(),
        };
        let mut extra = Map::new();
        extra.insert(
            "report".into(),
            json!(review.map(|dir| dir.display().to_string())),
        );
        crate::ui::inject(build);
        let ui = ::ui::Options {
            home: crate::cli::state_dir(build.name),
            admin: options.admin.clone(),
            agents: crate::agents::local(build),
        };
        ::ui::serve_page(crate::ui::port(), &ui, Some(&page), true, extra).map(exit_code)
    }

    pub fn review_dir(build: Build, path: &Path) -> Result<PathBuf> {
        let given = path
            .canonicalize()
            .with_context(|| format!("{} is not a folder or a report", path.display()))?;
        let dir = if given.is_file() {
            given.parent().map(Path::to_path_buf).unwrap_or_default()
        } else {
            given.clone()
        };
        let named = dir
            .file_name()
            .is_some_and(|hash| ::ui::reviews::valid_hash(&hash.to_string_lossy()));
        if named && dir.join("review.json").is_file() {
            return Ok(dir);
        }
        let reviewed = default_out(build, &dir);
        if reviewed.join("review.json").is_file() {
            return Ok(reviewed);
        }
        bail!(
            "There is no review of {}: run ai2 on it first.",
            given.display()
        )
    }
}

#[cfg(not(feature = "ui"))]
mod ui {
    use anyhow::{anyhow, Result};
    use clap::ArgMatches;
    use std::process::ExitCode;

    pub fn commands() -> Vec<clap::Command> {
        Vec::new()
    }

    pub fn run(
        _build: crate::cli::Build,
        name: &str,
        _sub: &ArgMatches,
        _options: &vault::Options,
    ) -> Result<ExitCode> {
        Err(anyhow!("unknown vault subcommand {name}"))
    }
}

fn run_serve(name: &str, serve: Serve) -> Result<()> {
    let platform = keys::platform()?;
    let mut config = serve.config;
    config
        .owners
        .extend(owners(std::env::var("AI2_VAULT_OWNERS").ok().as_deref()));
    let explicit = serve.providers.is_some() || serve.model.is_some();
    let chosen = vetting(name, explicit);
    let file = providers_file(name, serve.providers);
    let model = serve.model.or(chosen.model_name);
    let harness = chosen.harness;
    vault::start(config, platform, move |resolver: Resolver| {
        if let Some(harness) = harness {
            return harness;
        }
        let Some(file) = file else {
            eprintln!("{name} vault: no providers file, so calls that need vetting are refused");
            return Box::new(NoVetter);
        };
        match Model::load(&file, model.as_deref()) {
            Ok(m) => Box::new(ModelVetter(m.with_vault(Arc::new(move |key: &str| {
                resolver(key).map(|secret| secret.as_str().to_string())
            })))),
            Err(error) => {
                eprintln!("{name} vault: no vetting model: {error:#}");
                Box::new(NoVetter)
            }
        }
    })?
    .wait();
    Ok(())
}

#[derive(Default)]
struct VetterChoice {
    model_name: Option<String>,
    harness: Option<Box<dyn Vetter>>,
}

#[cfg(feature = "harness")]
const VET_LIMIT: Duration = Duration::from_secs(300);

#[cfg(feature = "harness")]
fn vetting(name: &str, explicit: bool) -> VetterChoice {
    use ::harness::settings::{model_name, Settings, VaultUse, MODEL};
    let home = crate::cli::state_dir(name);
    let chosen = match VaultUse::load(&home) {
        Ok(Some(chosen)) => chosen,
        Ok(None) => return VetterChoice::default(),
        Err(error) => {
            eprintln!("{name} vault: the vault settings were left out: {error:#}");
            return VetterChoice::default();
        }
    };
    if explicit || chosen.vetter == MODEL {
        return VetterChoice {
            model_name: model_name(chosen.provider.as_deref(), chosen.model.as_deref()),
            harness: None,
        };
    }
    let registry = ::harness::Registry::public();
    let Some(setup) = registry
        .agent(&chosen.vetter)
        .filter(|s| s.headless().is_some())
    else {
        eprintln!(
            "{name} vault: there is no harness called {}, so calls that need vetting are refused",
            chosen.vetter
        );
        return VetterChoice {
            model_name: None,
            harness: Some(Box::new(NoVetter)),
        };
    };
    VetterChoice {
        model_name: None,
        harness: Some(Box::new(HarnessVetter {
            setup: setup.clone(),
            home: std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default(),
            model: chosen.model,
            limit: VET_LIMIT,
        })),
    }
}

#[cfg(not(feature = "harness"))]
fn vetting(_name: &str, _explicit: bool) -> VetterChoice {
    VetterChoice::default()
}

fn owners(listed: Option<&str>) -> Vec<u32> {
    listed
        .unwrap_or_default()
        .split(',')
        .filter_map(|uid| uid.trim().parse().ok())
        .collect()
}

fn admin_get(name: &'static str, path: &str) -> Result<Value> {
    let options = VaultSubcommand::new(name).options();
    let (status, value) = vault::cli::Admin::new(&options).call("GET", path, None, false)?;
    if status != 200 {
        bail!("{path} answered {status}: {value}");
    }
    Ok(value)
}

pub fn daemon_version(name: &'static str) -> Result<String> {
    admin_get(name, "/status")?["version"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("/status gave no version"))
}

pub fn rows(name: &'static str) -> Vec<Row> {
    vec![
        sandboxes_row(name, admin_get(name, "/sandboxes")),
        vetting_model(name),
    ]
}

pub fn sandboxes_row(name: &str, listed: Result<Value>) -> Row {
    const CHECK: &str = "sandboxes";
    let listed = match listed {
        Ok(listed) => listed,
        Err(error) => {
            return Row::fail(CHECK, format!("{error:#}")).fix(format!("run {name} setup"))
        }
    };
    let names: Vec<&str> = listed["sandboxes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|sandbox| sandbox["name"].as_str())
        .collect();
    if names.is_empty() {
        return Row::ok(CHECK, "none");
    }
    let detail = names.join(", ");
    if names.contains(&"host") {
        return Row::warn(
            CHECK,
            format!("{detail}; host is not safe: its tokens work for any process on this machine, outside a container"),
        )
        .fix(format!(
            "give agents a container sandbox, and remove host with {name} vault sandbox remove --name host"
        ));
    }
    Row::ok(CHECK, detail)
}

const VETTING: &str = "vetting-model";

pub fn vetting_model(name: &str) -> Row {
    let Some(file) = providers_file(name, None) else {
        return Row::warn(
            VETTING,
            "no providers file, so the vault refuses calls that need vetting",
        )
        .fix(format!(
            "write ~/.{name}/providers.json or set {}_PROVIDERS",
            name.to_uppercase().replace('-', "_")
        ));
    };
    match Model::load(&file, None) {
        Ok(model) => vetting_row(&model.name(), model.base_url(), reach(model.base_url())),
        Err(error) => {
            Row::fail(VETTING, format!("{error:#}")).fix(format!("fix {}", file.display()))
        }
    }
}

fn reach(url: &str) -> Result<u16> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .http_status_as_error(false)
        .build()
        .new_agent();
    Ok(agent
        .get(url)
        .call()
        .with_context(|| format!("calling {url}"))?
        .status()
        .as_u16())
}

pub fn vetting_row(model: &str, url: &str, reached: Result<u16>) -> Row {
    match reached {
        Ok(status) => Row::ok(VETTING, format!("{model} at {url} answered {status}")),
        Err(error) => Row::fail(VETTING, format!("{model}: {error:#}"))
            .fix(format!("check the network and {url}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::State;
    use serde_json::json;

    #[test]
    fn a_host_sandbox_is_a_warning_and_an_unreachable_daemon_a_failure() {
        assert_eq!(
            sandboxes_row("ai2", Ok(json!({ "sandboxes": [] }))).state,
            State::Ok
        );
        let boxed = sandboxes_row("ai2", Ok(json!({ "sandboxes": [{ "name": "box" }] })));
        assert_eq!((boxed.state, boxed.detail.as_str()), (State::Ok, "box"));
        let host = sandboxes_row(
            "ai2",
            Ok(json!({ "sandboxes": [{ "name": "box" }, { "name": "host" }] })),
        );
        assert_eq!(host.state, State::Warn);
        assert!(host.detail.starts_with("box, host; host is not safe"));
        assert_eq!(
            sandboxes_row("ai2", Err(anyhow!("not running"))).state,
            State::Fail
        );
    }

    #[test]
    fn the_vetting_model_is_reached_without_a_key() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let answer = std::thread::spawn(move || {
            let (mut conn, _) = listener.accept().unwrap();
            let mut asked = [0u8; 1024];
            let read = conn.read(&mut asked).unwrap();
            conn.write_all(
                b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
            )
            .unwrap();
            String::from_utf8_lossy(&asked[..read]).to_lowercase()
        });
        let row = vetting_row("a/one", &url, reach(&url));
        assert_eq!(row.state, State::Ok);
        assert!(row.detail.ends_with("answered 404"));
        let asked = answer.join().unwrap();
        assert!(!asked.contains("authorization") && !asked.contains("x-api-key"));
        assert_eq!(
            vetting_row("a/one", &url, Err(anyhow!("refused"))).state,
            State::Fail
        );
    }

    #[cfg(feature = "harness")]
    fn fake_harness(dir: &std::path::Path, answer: &str) -> HarnessVetter {
        use std::os::unix::fs::PermissionsExt;
        let script = dir.join("fake-harness");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$1\" > \"$(dirname \"$0\")/asked\"\nout=$(printf '%s\\n' \"$1\" | tail -n 1)\nprintf '%s' '{answer}' > \"$out\"\n"
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let run = script.display().to_string();
        HarnessVetter {
            setup: ::harness::Setup {
                id: "fake".into(),
                label: "Fake".into(),
                command: run.clone(),
                agent: Some(::harness::Agent {
                    models: Vec::new(),
                    headless: Some(::harness::list::headless(&run, "")),
                }),
                ..::harness::Setup::default()
            },
            home: dir.to_path_buf(),
            model: None,
            limit: Duration::from_secs(20),
        }
    }

    #[cfg(feature = "harness")]
    fn a_call() -> Vetting {
        Vetting {
            key: KeyInfo {
                name: "openai".into(),
                description: "OpenAI".into(),
                approved_urls: vec!["https://api.openai.com/".into()],
            },
            intent: "list models".into(),
            method: "GET".into(),
            url: "https://api.openai.com/v1/models".into(),
            request: "listing".into(),
        }
    }

    #[cfg(feature = "harness")]
    #[test]
    fn a_harness_vetter_asks_with_the_vetting_rules_and_reads_its_answer() {
        let dir = tempfile::tempdir().unwrap();
        let vetter = fake_harness(
            dir.path(),
            r#"{"allow": true, "prefix": "https://api.openai.com/v1/", "why": "fits"}"#,
        );
        let verdict = vetter.vet(&a_call()).unwrap();
        assert!(verdict.allow);
        assert_eq!(verdict.prefix, "https://api.openai.com/v1/");
        let asked = std::fs::read_to_string(dir.path().join("asked")).unwrap();
        assert!(asked.starts_with(vault::VET_SYSTEM));
        assert!(asked.contains("untrusted_request_from_the_agent"));
        assert!(asked.contains(ANSWER_LINE));
        let proposer = fake_harness(
            dir.path(),
            r#"{"prefixes": ["https://api.openai.com/v1/"]}"#,
        );
        let info = a_call().key;
        assert_eq!(
            proposer.propose(&info).unwrap(),
            ["https://api.openai.com/v1/"]
        );
    }

    #[cfg(feature = "harness")]
    #[test]
    fn a_harness_vetter_refuses_when_the_answer_is_not_a_verdict() {
        let dir = tempfile::tempdir().unwrap();
        assert!(fake_harness(dir.path(), "no").vet(&a_call()).is_err());
        assert!(fake_harness(dir.path(), r#"{"why": "no allow"}"#)
            .vet(&a_call())
            .is_err());
    }

    #[test]
    fn a_vault_key_goes_through_the_gateway_with_the_saved_pass() {
        let home = tempfile::tempdir().unwrap();
        let socket = home.path().join("gw.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        let seen = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request = vault::http::read_request(&mut stream).unwrap();
            vault::http::Response::json(
                200,
                &json!({ "choices": [{ "message": { "content": "{\"ok\": 1}" } }] }),
            )
            .write_to(&mut stream)
            .unwrap();
            request
        });
        vault::app::save(
            home.path(),
            "openai",
            &vault::app::AppToken {
                id: "t".into(),
                token: "vt_pass".into(),
                revoke: "r".into(),
                socket,
            },
        )
        .unwrap();
        let file = home.path().join("providers.json");
        std::fs::write(
            &file,
            r#"{"providers":{"openai":{"api":"openai-completions","baseUrl":"https://api.openai.com/v1","apiKey":"vault:openai","models":[{"id":"gpt-x"}]}}}"#,
        )
        .unwrap();
        let model = Model::load(&file, None)
            .unwrap()
            .with_gateway(gateway(home.path().to_path_buf()));
        let text = model.text("s", "i", &crate::model::ASKING).unwrap();
        assert_eq!(text, "{\"ok\": 1}");
        let request = seen.join().unwrap();
        assert_eq!(request.target, "/openai/api.openai.com/v1/chat/completions");
        assert_eq!(request.header("authorization"), Some("Bearer vt_pass"));
        let missing = Model::load(&file, None)
            .unwrap()
            .with_gateway(gateway(home.path().join("elsewhere")));
        assert!(format!(
            "{:#}",
            missing.text("s", "i", &crate::model::ASKING).unwrap_err()
        )
        .contains("Tools page"));
    }

    #[test]
    fn owners_come_from_a_comma_list_of_uids() {
        assert_eq!(super::owners(Some("501, 1000,x")), vec![501, 1000]);
        assert!(super::owners(None).is_empty());
    }

    #[cfg(feature = "ui")]
    #[test]
    fn vault_ui_report_finds_the_review_folder_from_its_report_or_itself() {
        let home = tempfile::tempdir().unwrap();
        let review = home.path().join("0123456789abcdef");
        std::fs::create_dir(&review).unwrap();
        std::fs::write(review.join("review.json"), "{}").unwrap();
        std::fs::write(review.join("report.html"), "<html>").unwrap();
        let real = review.canonicalize().unwrap();
        let build = crate::cli::Build::ai2();
        assert_eq!(
            super::ui::review_dir(build, &review.join("report.html")).unwrap(),
            real
        );
        assert_eq!(super::ui::review_dir(build, &review).unwrap(), real);
        let unreviewed = home.path().join("plain");
        std::fs::create_dir(&unreviewed).unwrap();
        assert!(super::ui::review_dir(build, &unreviewed).is_err());
        assert!(super::ui::review_dir(build, &home.path().join("nope")).is_err());
    }
}
