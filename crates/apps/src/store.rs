use crate::error::{Problem, Result};
use crate::icon;
use crate::model::{Api, App, AppPatch, Icon, IconUpload, LogLine, NewApp, Restart, Stream};
use crate::port;
use crate::time;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

pub const LOG_LIMIT: u64 = 8 * 1024 * 1024;

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Running {
    pub pid: u32,
    pub started_at: String,
    pub boot: String,
    pub since: String,
    #[serde(default)]
    pub piped: bool,
}

#[derive(Clone, Debug)]
pub struct Store {
    dir: PathBuf,
}

pub struct Lock(File);

impl Drop for Lock {
    fn drop(&mut self) {
        unsafe {
            libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

impl Store {
    pub fn open(dir: &Path) -> Result<Store> {
        for sub in ["", "icons", "logs", "openapi"] {
            std::fs::create_dir_all(dir.join(sub))?;
        }
        Ok(Store {
            dir: dir.to_path_buf(),
        })
    }

    pub fn at_home(ai2_home: &Path) -> Result<Store> {
        Store::open(&ai2_home.join("apps"))
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn lock(&self) -> Result<Lock> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.dir.join("apps.lock"))?;
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(Lock(file))
    }

    pub fn apps(&self) -> Result<Vec<App>> {
        read_json(&self.dir.join("apps.json"))
    }

    pub fn app(&self, id: &str) -> Result<App> {
        self.apps()?
            .into_iter()
            .find(|app| app.id == id)
            .ok_or_else(|| Problem::not_found(id))
    }

    fn save(&self, apps: &[App]) -> Result<()> {
        write_json(&self.dir.join("apps.json"), &apps)
    }

    pub fn register(&self, new: NewApp) -> Result<(App, bool)> {
        let fields = Fields::checked(new)?;
        let _lock = self.lock()?;
        let mut apps = self.apps()?;
        let now = time::stamp();
        if let Some(app) = apps.iter_mut().find(|app| app.name == fields.name) {
            let id = app.id.clone();
            let icon = match &fields.icon {
                Some(upload) => Some(self.save_icon(&id, upload)?),
                None => app.icon.clone(),
            };
            *app = fields.into_app(id, icon, app.added.clone(), now);
            let app = app.clone();
            self.save(&apps)?;
            return Ok((app, false));
        }
        let Some(upload) = &fields.icon else {
            return Err(Problem::new(
                422,
                "icon_required",
                "Send the app's favicon: a png, svg, ico, webp or jpg image.",
            ));
        };
        let id = unique_id(&apps, &fields.name);
        let icon = self.save_icon(&id, upload)?;
        let app = fields.into_app(id, Some(icon), now.clone(), now);
        apps.push(app.clone());
        self.save(&apps)?;
        Ok((app, true))
    }

    pub fn update(&self, id: &str, patch: AppPatch) -> Result<App> {
        let _lock = self.lock()?;
        let mut apps = self.apps()?;
        let Some(app) = apps.iter_mut().find(|app| app.id == id) else {
            return Err(Problem::not_found(id));
        };
        let icon = match &patch.icon {
            Some(upload) => Some(self.save_icon(id, upload)?),
            None => app.icon.clone(),
        };
        let port_changed = patch.port.is_some();
        let fields = Fields::checked(NewApp {
            name: patch.name.unwrap_or_else(|| app.name.clone()),
            description: patch.description.unwrap_or_else(|| app.description.clone()),
            command: patch.command.unwrap_or_else(|| app.command.clone()),
            openapi: patch.openapi.unwrap_or_else(|| app.openapi.clone()),
            icon: None,
            cwd: Some(patch.cwd.unwrap_or_else(|| app.cwd.clone())),
            port: Some(patch.port.unwrap_or(app.port)),
            url: patch
                .url
                .or_else(|| (!port_changed).then(|| app.url.clone())),
            env: patch.env.unwrap_or_else(|| app.env.clone()),
            set_port: Some(patch.set_port.unwrap_or(app.set_port)),
            restart: Some(patch.restart.unwrap_or(app.restart)),
        })?;
        *app = fields.into_app(id.to_string(), icon, app.added.clone(), time::stamp());
        let app = app.clone();
        self.save(&apps)?;
        Ok(app)
    }

    pub fn remove(&self, id: &str) -> Result<App> {
        let _lock = self.lock()?;
        let mut apps = self.apps()?;
        let Some(at) = apps.iter().position(|app| app.id == id) else {
            return Err(Problem::not_found(id));
        };
        let app = apps.remove(at);
        self.save(&apps)?;
        if let Some(icon) = &app.icon {
            let _ = std::fs::remove_file(self.dir.join(&icon.file));
        }
        let _ = std::fs::remove_file(self.document_path(id));
        let _ = std::fs::remove_file(self.api_path(id));
        Ok(app)
    }

    fn save_icon(&self, id: &str, upload: &IconUpload) -> Result<Icon> {
        let (content_type, bytes) = icon::decode(upload)?;
        if let Some(old) = std::fs::read_dir(self.dir.join("icons"))?
            .flatten()
            .map(|entry| entry.path())
            .find(|path| path.file_stem().and_then(|s| s.to_str()) == Some(id))
        {
            let _ = std::fs::remove_file(old);
        }
        let file = format!("icons/{id}.{}", icon::extension(&content_type));
        write_bytes(&self.dir.join(&file), &bytes)?;
        Ok(Icon {
            content_type,
            file,
            bytes: bytes.len() as u64,
        })
    }

    pub fn icon(&self, id: &str) -> Result<Option<(String, Vec<u8>)>> {
        let app = self.app(id)?;
        let Some(icon) = app.icon else {
            return Ok(None);
        };
        Ok(Some((
            icon.content_type,
            std::fs::read(self.dir.join(&icon.file))?,
        )))
    }

    pub fn log_path(&self, id: &str) -> PathBuf {
        self.dir.join("logs").join(format!("{id}.log"))
    }

    pub fn open_log(&self, id: &str) -> Result<File> {
        let path = self.log_path(id);
        if std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) >= LOG_LIMIT {
            std::fs::rename(&path, path.with_extension("log.1"))?;
        }
        Ok(OpenOptions::new().create(true).append(true).open(path)?)
    }

    pub fn mark_start(&self, id: &str) -> Result<File> {
        let mut file = self.open_log(id)?;
        writeln!(file, "--- {} start", time::stamp())?;
        Ok(file)
    }

    pub fn tail(&self, id: &str, n: usize) -> Result<Vec<LogLine>> {
        let text = match std::fs::read(self.log_path(id)) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error.into()),
        };
        let lines: Vec<&str> = text.lines().collect();
        Ok(lines[lines.len().saturating_sub(n)..]
            .iter()
            .map(|line| LogLine {
                t: None,
                stream: Stream::Log,
                line: line.to_string(),
            })
            .collect())
    }

    pub fn document_path(&self, id: &str) -> PathBuf {
        self.dir.join("openapi").join(format!("{id}.json"))
    }

    pub fn api_path(&self, id: &str) -> PathBuf {
        self.dir.join("openapi").join(format!("{id}.state.json"))
    }

    pub fn api(&self, app: &App) -> Api {
        let stored: Option<Api> = std::fs::read(self.api_path(&app.id))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok());
        match stored {
            Some(api) if api.openapi == app.openapi => api,
            _ => Api {
                openapi: app.openapi.clone(),
                href: href(&app.id),
                ..Api::default()
            },
        }
    }

    pub fn api_age(&self, id: &str) -> Option<std::time::Duration> {
        std::fs::metadata(self.api_path(id))
            .and_then(|m| m.modified())
            .ok()
            .and_then(|at| at.elapsed().ok())
    }

    pub fn save_api(&self, id: &str, api: &Api, document: Option<&Value>) -> Result<()> {
        if let Some(document) = document {
            write_json(&self.document_path(id), document)?;
        }
        write_json(&self.api_path(id), api)
    }

    pub fn document(&self, id: &str) -> Option<Value> {
        std::fs::read(self.document_path(id))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
    }

    pub fn running(&self) -> Result<BTreeMap<String, Running>> {
        let path = self.dir.join("running.json");
        if !path.exists() {
            return Ok(BTreeMap::new());
        }
        read_json(&path)
    }

    pub fn set_running(&self, id: &str, entry: Option<Running>) -> Result<()> {
        let _lock = self.lock()?;
        let mut running = self.running()?;
        match entry {
            Some(entry) => running.insert(id.to_string(), entry),
            None => running.remove(id),
        };
        write_json(&self.dir.join("running.json"), &running)
    }
}

pub fn href(id: &str) -> String {
    format!("/api/apps/{id}/openapi.json")
}

struct Fields {
    name: String,
    description: String,
    command: String,
    openapi: String,
    icon: Option<IconUpload>,
    cwd: String,
    port: u16,
    url: String,
    env: BTreeMap<String, String>,
    set_port: bool,
    restart: Restart,
}

fn required(value: String, error: &str, message: &str) -> Result<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(Problem::new(422, error, message));
    }
    Ok(value)
}

impl Fields {
    fn checked(new: NewApp) -> Result<Fields> {
        let name = required(new.name, "name_required", "Send the app's name.")?;
        let description = required(
            new.description,
            "description_required",
            "Send a description: one line on what the app is for.",
        )?;
        let command = required(
            new.command,
            "command_required",
            "Send the command that runs the app, e.g. deno task start.",
        )?;
        let openapi = required(
            new.openapi,
            "openapi_required",
            "Send the path of the app's OpenAPI document, e.g. /openapi.json, or a file.",
        )?;
        let cwd = new.cwd.unwrap_or_default();
        if !Path::new(&cwd).is_absolute() || !Path::new(&cwd).is_dir() {
            return Err(Problem::new(
                422,
                "cwd_required",
                format!("Send the app's folder as an absolute path; \"{cwd}\" is not a folder."),
            ));
        }
        if let Some(key) = new
            .env
            .keys()
            .find(|key| key.is_empty() || key.contains('=') || key.contains('\0'))
        {
            return Err(Problem::new(
                422,
                "env_invalid",
                format!("\"{key}\" is not a variable name."),
            ));
        }
        let Some(port) = new
            .port
            .filter(|port| *port > 0)
            .or_else(|| port::detect(Path::new(&cwd), &command))
        else {
            return Err(Problem::new(
                422,
                "port_required",
                format!("Could not tell which port \"{command}\" opens; send --port."),
            ));
        };
        let url = new
            .url
            .map(|url| url.trim().to_string())
            .filter(|url| !url.is_empty())
            .unwrap_or_else(|| format!("http://localhost:{port}"));
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(Problem::new(
                422,
                "url_invalid",
                format!("\"{url}\" is not an http address."),
            ));
        }
        Ok(Fields {
            name,
            description,
            command,
            openapi,
            icon: new.icon,
            cwd,
            port,
            url,
            env: new.env,
            set_port: new.set_port.unwrap_or(true),
            restart: new.restart.unwrap_or_default(),
        })
    }

    fn into_app(self, id: String, icon: Option<Icon>, added: String, updated: String) -> App {
        App {
            id,
            name: self.name,
            description: self.description,
            command: self.command,
            cwd: self.cwd,
            port: self.port,
            url: self.url,
            env: self.env,
            set_port: self.set_port,
            restart: self.restart,
            icon,
            openapi: self.openapi,
            added,
            updated,
        }
    }
}

pub fn slug(text: &str) -> String {
    let lower = text.to_lowercase();
    let unscoped = match lower
        .strip_prefix('@')
        .and_then(|rest| rest.split_once('/'))
    {
        Some((_, rest)) => rest.to_string(),
        None => lower,
    };
    let mut out = String::new();
    for c in unscoped.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "app".to_string()
    } else {
        out
    }
}

fn unique_id(apps: &[App], name: &str) -> String {
    let base = slug(name);
    let taken = |id: &str| apps.iter().any(|app| app.id == id);
    if !taken(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|id| !taken(id))
        .unwrap_or(base)
}

fn read_json<T: DeserializeOwned + Default>(path: &Path) -> Result<T> {
    match std::fs::read(path) {
        Ok(bytes) if bytes.iter().all(u8::is_ascii_whitespace) => Ok(T::default()),
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| {
            Problem::internal(format!("{} is not valid: {error}", path.display()))
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(error) => Err(error.into()),
    }
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    write_bytes(path, &bytes)
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    let temp = path.with_extension(format!(
        "{}.new",
        path.extension().and_then(|e| e.to_str()).unwrap_or("")
    ));
    let written = (|| -> std::io::Result<()> {
        let mut out = File::create(&temp)?;
        out.write_all(bytes)?;
        out.sync_all()?;
        std::fs::rename(&temp, path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    written.map_err(|error| Problem::internal(format!("writing {}: {error}", path.display())))
}
