use crate::client;
use crate::daemon::Config;
use crate::policy::encode_percent;
use crate::ttyagent;
use crate::with::{self, With};
use anyhow::{bail, Result};
use clap::{ArgMatches, FromArgMatches, Subcommand};
use keys::tty;
use serde::Serialize;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::io::{Read, Write};
use std::net::IpAddr;
use std::path::{Path, PathBuf};

const LINUX_RUN: &str = "/run/ai2-vault";
use zeroize::Zeroizing;

pub mod exit {
    pub const OK: i32 = 0;
    pub const ERROR: i32 = 1;
    pub const DENIED: i32 = 2;
    pub const USAGE: i32 = 64;
}

#[derive(Subcommand, Debug)]
enum VaultCommand {
    #[command(about = "Stores a key: its secret encrypted, where it may be sent and how")]
    Put {
        #[arg(long)]
        name: String,
        #[arg(
            long,
            help = "The secret, or - to read it from stdin [default: a hidden prompt]"
        )]
        secret: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(
            long = "approved-url",
            help = "A URL prefix the key may be sent to, repeatable"
        )]
        approved_url: Vec<String>,
        #[arg(
            long,
            help = "bearer, basic, query:<param>, header:<Name> or \"Name: ...{secret}...\" [default: bearer]"
        )]
        inject: Option<String>,
    },
    #[command(about = "Lists the keys, never their secrets")]
    List,
    #[command(about = "Says whether a key is stored")]
    Has {
        #[arg(long)]
        name: String,
    },
    #[command(about = "Deletes a key")]
    Delete {
        #[arg(long)]
        name: String,
    },
    #[command(about = "Replaces a key's secret")]
    Rotate {
        #[arg(long)]
        name: String,
        #[arg(long)]
        secret: Option<String>,
    },
    #[command(about = "Makes, lists and revokes tokens")]
    Token {
        #[command(subcommand)]
        command: TokenCommand,
    },
    #[command(about = "Adds, lists and removes sandbox endpoints")]
    Sandbox {
        #[command(subcommand)]
        command: SandboxCommand,
    },
    #[command(about = "Lists the calls waiting for you, or decides one")]
    Approvals {
        #[arg(long, conflicts_with = "deny")]
        approve: Option<String>,
        #[arg(long)]
        deny: Option<String>,
    },
    #[command(about = "Prints the last lines of the audit log")]
    Audit {
        #[arg(long, default_value_t = 50)]
        tail: usize,
    },
    #[command(about = "Says whether the daemon has its master key, and its version")]
    Status,
    #[command(about = "Runs the vault daemon")]
    Serve {
        #[arg(long, help = "A providers file for the vetting model")]
        providers: Option<PathBuf>,
        #[arg(long, help = "<provider>/<model-id> for the vetting model")]
        model: Option<String>,
    },
    #[command(
        about = "Runs a command with a token for some keys, and revokes the token when it exits"
    )]
    With {
        #[arg(
            value_delimiter = ',',
            required = true,
            help = "The keys, comma separated"
        )]
        keys: Vec<String>,
        #[arg(long, default_value = "host")]
        sandbox: String,
        #[arg(long, help = "What the command is for; needed for agent tokens")]
        intent: Option<String>,
        #[arg(long, default_value = "agent", value_parser = ["agent", "app"])]
        mode: String,
        #[arg(long, help = "read, write, all or GET,POST,... [default: read]")]
        methods: Option<String>,
        #[arg(long = "url-prefix")]
        url_prefix: Vec<String>,
        #[arg(long = "for", help = "How long the token lasts at most, e.g. 2h")]
        lasts: Option<String>,
        #[arg(last = true, required = true)]
        command: Vec<String>,
    },
}

#[derive(Subcommand, Debug)]
enum TokenCommand {
    Create {
        #[arg(long, value_delimiter = ',', required = true)]
        keys: Vec<String>,
        #[arg(long)]
        sandbox: String,
        #[arg(long, value_parser = ["app", "agent"])]
        mode: String,
        #[arg(long, help = "read, write, all or GET,POST,... [default: read]")]
        methods: Option<String>,
        #[arg(long = "url-prefix")]
        url_prefix: Vec<String>,
        #[arg(long)]
        intent: Option<String>,
        #[arg(long, help = "15m, 8h, 30d [default: 8h for agent, 30d for app]")]
        ttl: Option<String>,
        #[arg(long)]
        uses: Option<u64>,
    },
    List,
    Revoke {
        #[arg(long)]
        id: String,
    },
}

#[derive(Subcommand, Debug)]
enum SandboxCommand {
    Add {
        #[arg(long)]
        name: String,
        #[arg(long, help = "The gateway's Unix socket [default: one per sandbox]")]
        socket: Option<PathBuf>,
        #[arg(long, help = "Listen on TCP at this address instead of a Unix socket")]
        tcp: Option<String>,
        #[arg(long, help = "An address a TCP endpoint takes calls from, repeatable")]
        peer: Vec<IpAddr>,
    },
    List,
    Remove {
        #[arg(long)]
        name: String,
    },
}

pub struct Serve {
    pub config: Config,
    pub providers: Option<PathBuf>,
    pub model: Option<String>,
}

pub type ServeHook = Box<dyn Fn(Serve) -> Result<()>>;

pub struct Options {
    pub admin: PathBuf,
    pub state: PathBuf,
    pub sockets: PathBuf,
    pub ask_passphrase: bool,
    pub serve: ServeHook,
}

pub enum Outcome {
    Code(i32),
    Extra(String, ArgMatches),
}

impl Options {
    pub fn new(ai2_home: &Path, serve: ServeHook) -> Options {
        let (state, admin, sockets) = default_paths(ai2_home);
        Options {
            admin,
            state,
            sockets,
            ask_passphrase: cfg!(target_os = "linux"),
            serve,
        }
    }
}

pub fn default_paths(ai2_home: &Path) -> (PathBuf, PathBuf, PathBuf) {
    if cfg!(target_os = "linux") {
        let run = Path::new(LINUX_RUN);
        return (
            PathBuf::from(keys::LINUX_STATE),
            run.join("admin.sock"),
            run.join("sandboxes"),
        );
    }
    let state = ai2_home.join("vault");
    (
        state.clone(),
        state.join("admin.sock"),
        state.join("sandboxes"),
    )
}

pub fn command() -> clap::Command {
    command_with(Vec::new())
}

pub fn command_with(extra: Vec<clap::Command>) -> clap::Command {
    let command = VaultCommand::augment_subcommands(
        clap::Command::new("vault")
            .about("Keeps secrets away from agents: keys, tokens, sandboxes and the gateway")
            .subcommand_required(true),
    );
    extra
        .into_iter()
        .fold(command, |command, sub| command.subcommand(sub))
}

pub fn outcome(matches: &ArgMatches, opts: &Options) -> Result<Outcome> {
    if let Some((name, sub)) = matches.subcommand() {
        if !VaultCommand::has_subcommand(name) {
            return Ok(Outcome::Extra(name.to_string(), sub.clone()));
        }
    }
    let command = VaultCommand::from_arg_matches(matches)?;
    Ok(Outcome::Code(execute(command, opts)?))
}

pub(crate) fn code(status: u16) -> i32 {
    match status {
        200..=299 => exit::OK,
        400 => exit::USAGE,
        401 | 403 => exit::DENIED,
        _ => exit::ERROR,
    }
}

fn print(value: &impl Serialize) {
    let text = serde_json::to_string(value).unwrap_or_default();
    let _ = writeln!(std::io::stdout().lock(), "{text}");
}

fn usage(message: &str) -> i32 {
    print(&json!({ "error": "usage", "message": message }));
    exit::USAGE
}

pub struct Admin<'a> {
    opts: &'a Options,
    checked: Cell<bool>,
    presence: RefCell<Option<String>>,
}

impl<'a> Admin<'a> {
    pub fn new(opts: &'a Options) -> Admin<'a> {
        Admin {
            opts,
            checked: Cell::new(false),
            presence: RefCell::new(None),
        }
    }

    fn check_version(&self) {
        if self.checked.replace(true) {
            return;
        }
        let Ok((200, status)) = client::call(&self.opts.admin, "GET", "/status", None, &[]) else {
            return;
        };
        *self.presence.borrow_mut() = status["presence"].as_str().map(String::from);
        let theirs = status["version"].as_str().unwrap_or("an unknown version");
        let ours = env!("CARGO_PKG_VERSION");
        if theirs != ours {
            eprintln!("ai2 vault: the daemon is {theirs} and this ai2 is {ours}; restart the daemon (ai2 setup) so they match");
        }
    }

    pub fn call_with(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        present: bool,
        headers: &[(&str, &str)],
    ) -> Result<(u16, Value)> {
        self.check_version();
        let kind = self.presence.borrow().clone();
        let asks = present && self.opts.ask_passphrase;
        let _agent = (asks && kind.as_deref() == Some(keys::POLKIT))
            .then(|| ttyagent::start(std::env::var_os("PATH").as_deref(), Path::new("/proc")))
            .flatten();
        let passphrase = if asks && kind.as_deref() != Some(keys::POLKIT) {
            Some(tty::hidden("Vault passphrase: ")?)
        } else {
            None
        };
        let mut all: Vec<(&str, &str)> = headers.to_vec();
        if let Some(passphrase) = passphrase.as_deref() {
            all.push(("X-Vault-Passphrase", passphrase.as_str()));
        }
        client::call(&self.opts.admin, method, path, body.as_ref(), &all)
    }

    pub fn call(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        present: bool,
    ) -> Result<(u16, Value)> {
        self.call_with(method, path, body, present, &[])
    }

    fn print(&self, method: &str, path: &str, body: Option<Value>, present: bool) -> Result<i32> {
        let (status, value) = self.call(method, path, body, present)?;
        print(&value);
        Ok(code(status))
    }
}

fn read_secret(given: Option<String>, name: &str) -> Result<Zeroizing<String>> {
    match given {
        Some(given) if given == "-" => {
            let mut text = Zeroizing::new(String::new());
            std::io::stdin().read_to_string(&mut text)?;
            Ok(Zeroizing::new(
                text.trim_end_matches(['\r', '\n']).to_string(),
            ))
        }
        Some(given) => {
            eprintln!("ai2 vault: the secret is now in your shell history; next time leave --secret out to be asked for it");
            Ok(Zeroizing::new(given))
        }
        None => tty::hidden(&format!("Secret for {name}: ")),
    }
}

enum Urls {
    Chosen(Vec<String>),
    Refused(i32),
}

fn approved_urls(admin: &Admin, name: &str, given: Vec<String>, description: &str) -> Result<Urls> {
    if !given.is_empty() {
        return Ok(Urls::Chosen(given));
    }
    if description.trim().is_empty() {
        return Ok(Urls::Refused(usage(
            "give --approved-url, or a --description the vetting model can propose approved URLs from",
        )));
    }
    let (status, value) = admin.call(
        "POST",
        &format!("/keys/{}/propose", encode_percent(name)),
        Some(json!({ "description": description })),
        false,
    )?;
    if status != 200 {
        print(&value);
        return Ok(Urls::Refused(code(status)));
    }
    let proposed: Vec<String> = serde_json::from_value(value["approved_urls"].clone())?;
    if proposed.is_empty() {
        bail!("the vetting model proposed no approved URLs: give --approved-url");
    }
    let listed = proposed
        .iter()
        .map(|url| format!("  {url}\n"))
        .collect::<String>();
    if !tty::confirm(&format!(
        "The model proposes {name} may be sent to:\n{listed}Store these?"
    ))? {
        bail!("not stored: give --approved-url to choose them yourself");
    }
    Ok(Urls::Chosen(proposed))
}

fn decide(admin: &Admin, id: &str, approve: bool) -> Result<i32> {
    admin.print(
        "POST",
        &format!("/approvals/{}", encode_percent(id)),
        Some(json!({ "approve": approve })),
        true,
    )
}

fn execute(command: VaultCommand, opts: &Options) -> Result<i32> {
    let admin = Admin::new(opts);
    match command {
        VaultCommand::Put {
            name,
            secret,
            description,
            approved_url,
            inject,
        } => {
            let description = description.unwrap_or_default();
            let approved = match approved_urls(&admin, &name, approved_url, &description)? {
                Urls::Chosen(urls) => urls,
                Urls::Refused(code) => return Ok(code),
            };
            let secret = read_secret(secret, &name)?;
            let body = json!({
                "secret": secret.as_str(),
                "description": description,
                "approved_urls": approved,
                "inject": inject,
            });
            admin.print(
                "PUT",
                &format!("/keys/{}", encode_percent(&name)),
                Some(body),
                true,
            )
        }
        VaultCommand::List => admin.print("GET", "/keys", None, false),
        VaultCommand::Has { name } => {
            let (status, value) = admin.call(
                "GET",
                &format!("/keys/{}", encode_percent(&name)),
                None,
                false,
            )?;
            match status {
                200 | 404 => {
                    print(&json!({ "name": name, "exists": status == 200 }));
                    Ok(if status == 200 { exit::OK } else { exit::ERROR })
                }
                _ => {
                    print(&value);
                    Ok(code(status))
                }
            }
        }
        VaultCommand::Delete { name } => admin.print(
            "DELETE",
            &format!("/keys/{}", encode_percent(&name)),
            None,
            true,
        ),
        VaultCommand::Rotate { name, secret } => {
            let secret = read_secret(secret, &name)?;
            let body = json!({ "secret": secret.as_str() });
            admin.print(
                "POST",
                &format!("/keys/{}/rotate", encode_percent(&name)),
                Some(body),
                true,
            )
        }
        VaultCommand::Token { command } => token(&admin, command),
        VaultCommand::Sandbox { command } => sandbox(&admin, command),
        VaultCommand::Approvals { approve, deny } => match (approve, deny) {
            (Some(id), _) => decide(&admin, &id, true),
            (_, Some(id)) => decide(&admin, &id, false),
            _ => admin.print("GET", "/approvals", None, false),
        },
        VaultCommand::Audit { tail } => {
            admin.print("GET", &format!("/audit?tail={tail}"), None, false)
        }
        VaultCommand::Status => admin.print("GET", "/status", None, false),
        VaultCommand::Serve { providers, model } => {
            let mut config = Config::new(opts.state.clone(), opts.admin.clone());
            config.sockets = opts.sockets.clone();
            (opts.serve)(Serve {
                config,
                providers,
                model,
            })?;
            Ok(exit::OK)
        }
        VaultCommand::With {
            keys,
            sandbox,
            intent,
            mode,
            methods,
            url_prefix,
            lasts,
            command,
        } => Ok(with::run(
            &admin,
            With {
                keys,
                sandbox,
                intent,
                mode,
                methods,
                url_prefixes: url_prefix,
                ttl: lasts,
                command,
            },
        )),
    }
}

fn token(admin: &Admin, command: TokenCommand) -> Result<i32> {
    match command {
        TokenCommand::Create {
            keys,
            sandbox,
            mode,
            methods,
            url_prefix,
            intent,
            ttl,
            uses,
        } => {
            let body = json!({
                "keys": keys,
                "sandbox": sandbox,
                "mode": mode,
                "methods": methods,
                "url_prefixes": url_prefix,
                "intent": intent,
                "ttl": ttl,
                "uses": uses,
            });
            let (status, mut value) = admin.call("POST", "/tokens", Some(body), true)?;
            warn(&value);
            if status == 200 {
                eprintln!("ai2 vault: the token is printed once; it is not stored anywhere you can read it back");
                if let Some(object) = value.as_object_mut() {
                    object.remove("revoke");
                }
            }
            print(&value);
            Ok(code(status))
        }
        TokenCommand::List => admin.print("GET", "/tokens", None, false),
        TokenCommand::Revoke { id } => admin.print(
            "DELETE",
            &format!("/tokens/{}", encode_percent(&id)),
            None,
            true,
        ),
    }
}

pub(crate) fn warn(value: &Value) {
    for warning in value["warnings"].as_array().into_iter().flatten() {
        eprintln!("ai2 vault: {}", warning.as_str().unwrap_or_default());
    }
}

fn sandbox(admin: &Admin, command: SandboxCommand) -> Result<i32> {
    match command {
        SandboxCommand::Add {
            name,
            socket,
            tcp,
            peer,
        } => {
            let body = json!({ "name": name, "socket": socket, "tcp": tcp, "peers": peer });
            admin.print("POST", "/sandboxes", Some(body), true)
        }
        SandboxCommand::List => admin.print("GET", "/sandboxes", None, false),
        SandboxCommand::Remove { name } => admin.print(
            "DELETE",
            &format!("/sandboxes/{}", encode_percent(&name)),
            None,
            true,
        ),
    }
}
