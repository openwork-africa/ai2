use std::path::Path;

pub const LABEL: &str = "africa.openwork.ai2-vault";
pub const LABEL_ENV: &str = "AI2_VAULT_LABEL";
pub const UNIT: &str = "ai2-vault.service";
pub const UNIT_PATH: &str = "/etc/systemd/system/ai2-vault.service";
pub const USER: &str = "ai2-vault";
pub const ADMIN_GROUP: &str = "ai2-vault-admin";
pub const SERVICE_BIN: &str = "/usr/local/libexec/ai2-vault/ai2";
pub const UI_LABEL: &str = "africa.openwork.ai2-ui";
pub const UI_LABEL_ENV: &str = "AI2_UI_LABEL";
pub const UI_UNIT: &str = "ai2-ui.service";
pub const UI_PORT: u16 = 7797;
pub const UI_PORT_ENV: &str = "AI2_UI_PORT";
pub const UI_SERVICE_ENV: &str = "AI2_UI_SERVICE";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dashboard {
    pub label: String,
    pub port: u16,
    pub in_service: bool,
}

impl Default for Dashboard {
    fn default() -> Self {
        Self {
            label: UI_LABEL.into(),
            port: UI_PORT,
            in_service: false,
        }
    }
}

impl Dashboard {
    pub fn from_env() -> Self {
        Self {
            label: ui_label(std::env::var(UI_LABEL_ENV).ok()),
            port: ui_port(std::env::var(UI_PORT_ENV).ok()),
            in_service: std::env::var_os(UI_SERVICE_ENV).is_some(),
        }
    }
}

pub fn ui_label(value: Option<String>) -> String {
    value
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| UI_LABEL.to_string())
}

pub fn ui_port(value: Option<String>) -> u16 {
    value
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(UI_PORT)
}

pub fn ui_log(ai2_home: &Path) -> std::path::PathBuf {
    ai2_home.join("ui").join("dashboard.log")
}

pub fn ui_unit_path(home: &Path) -> std::path::PathBuf {
    home.join(".config/systemd/user").join(UI_UNIT)
}

pub fn ui_path(bin: &Path) -> String {
    let mut dirs: Vec<String> = bin
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.display().to_string())
        .into_iter()
        .collect();
    for dir in [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/usr/bin",
        "/bin",
        "/usr/sbin",
        "/sbin",
    ] {
        if !dirs.iter().any(|known| known == dir) {
            dirs.push(dir.into());
        }
    }
    dirs.join(":")
}

pub fn label(value: Option<String>) -> String {
    value
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| LABEL.to_string())
}

pub fn plist_path(home: &Path, label: &str) -> std::path::PathBuf {
    home.join("Library/LaunchAgents")
        .join(format!("{label}.plist"))
}

fn xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn plist(
    bin: &Path,
    ai2_home: &Path,
    state: &Path,
    label: &str,
    keychain: Option<&Path>,
) -> String {
    let bin = xml(&bin.display().to_string());
    let home = xml(&ai2_home.display().to_string());
    let log = xml(&state.join("daemon.log").display().to_string());
    let label = xml(label);
    let keychain = keychain.map_or(String::new(), |file| {
        format!(
            "\n\t\t<key>{}</key>\n\t\t<string>{}</string>",
            keys::KEYCHAIN_ENV,
            xml(&file.display().to_string())
        )
    });
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>{label}</string>
	<key>ProgramArguments</key>
	<array>
		<string>{bin}</string>
		<string>vault</string>
		<string>serve</string>
	</array>
	<key>EnvironmentVariables</key>
	<dict>
		<key>AI2_HOME</key>
		<string>{home}</string>{keychain}
	</dict>
	<key>LimitLoadToSessionType</key>
	<string>Aqua</string>
	<key>ProcessType</key>
	<string>Interactive</string>
	<key>RunAtLoad</key>
	<true/>
	<key>KeepAlive</key>
	<true/>
	<key>Umask</key>
	<integer>63</integer>
	<key>StandardOutPath</key>
	<string>{log}</string>
	<key>StandardErrorPath</key>
	<string>{log}</string>
</dict>
</plist>
"#
    )
}

pub fn ui_plist(
    bin: &Path,
    ai2_home: &Path,
    label: &str,
    port: u16,
    keychain: Option<&Path>,
) -> String {
    let path = xml(&ui_path(bin));
    let bin = xml(&bin.display().to_string());
    let home = xml(&ai2_home.display().to_string());
    let log = xml(&ui_log(ai2_home).display().to_string());
    let label = xml(label);
    let keychain = keychain.map_or(String::new(), |file| {
        format!(
            "\n\t\t<key>{}</key>\n\t\t<string>{}</string>",
            keys::KEYCHAIN_ENV,
            xml(&file.display().to_string())
        )
    });
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>{label}</string>
	<key>ProgramArguments</key>
	<array>
		<string>{bin}</string>
		<string>ui</string>
		<string>--no-open</string>
		<string>--port</string>
		<string>{port}</string>
	</array>
	<key>EnvironmentVariables</key>
	<dict>
		<key>AI2_HOME</key>
		<string>{home}</string>
		<key>{UI_SERVICE_ENV}</key>
		<string>1</string>
		<key>PATH</key>
		<string>{path}</string>{keychain}
	</dict>
	<key>LimitLoadToSessionType</key>
	<string>Aqua</string>
	<key>ProcessType</key>
	<string>Interactive</string>
	<key>RunAtLoad</key>
	<true/>
	<key>KeepAlive</key>
	<true/>
	<key>Umask</key>
	<integer>63</integer>
	<key>StandardOutPath</key>
	<string>{log}</string>
	<key>StandardErrorPath</key>
	<string>{log}</string>
</dict>
</plist>
"#
    )
}

fn systemd_env(key: &str, value: &str) -> String {
    let pair = format!("{key}={value}");
    if pair
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-+=:".contains(c))
    {
        return pair;
    }
    format!(
        "\"{}\"",
        pair.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('%', "%%")
    )
}

pub fn ui_unit(bin: &Path, ai2_home: &Path, port: u16) -> String {
    let exec = systemd_arg(&bin.display().to_string());
    let home = systemd_env("AI2_HOME", &ai2_home.display().to_string());
    let service = systemd_env(UI_SERVICE_ENV, "1");
    format!(
        "[Unit]
Description=ai2 dashboard: apps, reviews and the vault in the browser

[Service]
Type=simple
ExecStart={exec} ui --no-open --port {port}
Environment={home}
Environment={service}
Restart=always
RestartSec=2
UMask=0077

[Install]
WantedBy=default.target
"
    )
}

pub fn ui_restart(macos: bool, uid: u32, label: &str) -> String {
    if macos {
        format!("launchctl kickstart -k gui/{uid}/{label}")
    } else {
        format!("systemctl --user restart {UI_UNIT}")
    }
}

fn systemd_arg(text: &str) -> String {
    if text
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-+".contains(c))
    {
        return text.to_string();
    }
    format!(
        "\"{}\"",
        text.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('%', "%%")
            .replace('$', "$$")
    )
}

pub fn unit(bin: &Path, cred: &str, owner: u32) -> String {
    let bin = systemd_arg(&bin.display().to_string());
    format!(
        "[Unit]
Description=ai2 vault: keeps secrets away from agents
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User={USER}
Group={ADMIN_GROUP}
ExecStart={bin} vault serve
LoadCredentialEncrypted=master:{cred}
Environment=AI2_VAULT_OWNERS={owner}
Restart=on-failure
RuntimeDirectory=ai2-vault
RuntimeDirectoryMode=0755
StateDirectory=ai2-vault
StateDirectoryMode=0700
UMask=0077
MemorySwapMax=0
LimitCORE=0
NoNewPrivileges=yes
CapabilityBoundingSet=
AmbientCapabilities=
RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
PrivateDevices=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectKernelLogs=yes
ProtectControlGroups=yes
ProtectClock=yes
ProtectHostname=yes
RestrictNamespaces=yes
RestrictRealtime=yes
RestrictSUIDSGID=yes
LockPersonality=yes
MemoryDenyWriteExecute=yes
SystemCallArchitectures=native

[Install]
WantedBy=multi-user.target
"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_launch_agent_runs_vault_serve_in_the_session() {
        let text = plist(
            Path::new("/Users/a b/.local/bin/ai2"),
            Path::new("/Users/a b/.ai2"),
            Path::new("/Users/a b/.ai2/vault"),
            LABEL,
            None,
        );
        assert!(text.contains("<string>africa.openwork.ai2-vault</string>"));
        assert!(text.contains(
            "<string>/Users/a b/.local/bin/ai2</string>\n\t\t<string>vault</string>\n\t\t<string>serve</string>"
        ));
        assert!(text.contains("<key>AI2_HOME</key>\n\t\t<string>/Users/a b/.ai2</string>"));
        assert!(text.contains("<string>Aqua</string>"));
        assert!(text.contains("<string>/Users/a b/.ai2/vault/daemon.log</string>"));
        assert!(!text.contains("AI2_VAULT_KEYCHAIN"));
        assert_eq!(
            plist_path(Path::new("/Users/a"), LABEL),
            Path::new("/Users/a/Library/LaunchAgents/africa.openwork.ai2-vault.plist")
        );
    }

    #[test]
    fn the_label_is_the_real_one_unless_another_is_named() {
        assert_eq!(label(None), "africa.openwork.ai2-vault");
        assert_eq!(label(Some(String::new())), LABEL);
        assert_eq!(label(Some("test.ai2-vault".into())), "test.ai2-vault");
    }

    #[test]
    fn a_named_label_and_keychain_reach_the_launch_agent() {
        let text = plist(
            Path::new("/b/ai2"),
            Path::new("/h"),
            Path::new("/h/vault"),
            "test.ai2-vault",
            Some(Path::new("/t/v.keychain-db")),
        );
        assert!(text.contains("<key>Label</key>\n\t<string>test.ai2-vault</string>"));
        assert!(text.contains(
            "<string>/h</string>\n\t\t<key>AI2_VAULT_KEYCHAIN</key>\n\t\t<string>/t/v.keychain-db</string>\n\t</dict>"
        ));
        assert_eq!(
            plist_path(Path::new("/Users/a"), "test.ai2-vault"),
            Path::new("/Users/a/Library/LaunchAgents/test.ai2-vault.plist")
        );
    }

    #[test]
    fn paths_are_escaped_in_the_plist() {
        let text = plist(
            Path::new("/opt/<x>&y/ai2"),
            Path::new("/h"),
            Path::new("/h/vault"),
            LABEL,
            None,
        );
        assert!(text.contains("<string>/opt/&lt;x&gt;&amp;y/ai2</string>"));
    }

    #[test]
    fn the_unit_is_locked_down_and_loads_the_credential() {
        let text = unit(Path::new(SERVICE_BIN), "/etc/ai2-vault/master.cred", 1000);
        for line in [
            "User=ai2-vault",
            "Group=ai2-vault-admin",
            "ExecStart=/usr/local/libexec/ai2-vault/ai2 vault serve",
            "LoadCredentialEncrypted=master:/etc/ai2-vault/master.cred",
            "Environment=AI2_VAULT_OWNERS=1000",
            "Restart=on-failure",
            "RuntimeDirectory=ai2-vault",
            "RuntimeDirectoryMode=0755",
            "StateDirectory=ai2-vault",
            "StateDirectoryMode=0700",
            "UMask=0077",
            "MemorySwapMax=0",
            "LimitCORE=0",
            "NoNewPrivileges=yes",
            "CapabilityBoundingSet=",
            "AmbientCapabilities=",
            "RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6",
            "ProtectSystem=strict",
            "ProtectHome=yes",
            "PrivateTmp=yes",
            "PrivateDevices=yes",
            "ProtectKernelTunables=yes",
            "ProtectKernelModules=yes",
            "ProtectKernelLogs=yes",
            "ProtectControlGroups=yes",
            "ProtectClock=yes",
            "ProtectHostname=yes",
            "RestrictNamespaces=yes",
            "RestrictRealtime=yes",
            "RestrictSUIDSGID=yes",
            "LockPersonality=yes",
            "MemoryDenyWriteExecute=yes",
            "SystemCallArchitectures=native",
            "WantedBy=multi-user.target",
        ] {
            assert!(text.lines().any(|l| l == line), "{line}");
        }
        assert!(!text.contains("PrivateNetwork"));
        assert!(!text.contains("Group=ai2-vault\n"));
    }

    #[test]
    fn the_dashboard_launch_agent_runs_ui_without_opening_the_browser() {
        let text = ui_plist(
            Path::new("/Users/a b/.local/bin/ai2"),
            Path::new("/Users/a b/.ai2"),
            UI_LABEL,
            UI_PORT,
            None,
        );
        assert!(text.contains("<key>Label</key>\n\t<string>africa.openwork.ai2-ui</string>"));
        assert!(text.contains(
            "<string>/Users/a b/.local/bin/ai2</string>\n\t\t<string>ui</string>\n\t\t<string>--no-open</string>\n\t\t<string>--port</string>\n\t\t<string>7797</string>"
        ));
        assert!(text.contains("<key>AI2_HOME</key>\n\t\t<string>/Users/a b/.ai2</string>"));
        assert!(text.contains("<key>AI2_UI_SERVICE</key>\n\t\t<string>1</string>"));
        assert!(text.contains(
            "<key>PATH</key>\n\t\t<string>/Users/a b/.local/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>"
        ));
        assert!(text.contains("<key>RunAtLoad</key>\n\t<true/>"));
        assert!(text.contains("<key>KeepAlive</key>\n\t<true/>"));
        assert!(text.contains(
            "<key>StandardOutPath</key>\n\t<string>/Users/a b/.ai2/ui/dashboard.log</string>"
        ));
        assert!(text.contains(
            "<key>StandardErrorPath</key>\n\t<string>/Users/a b/.ai2/ui/dashboard.log</string>"
        ));
        assert!(!text.contains("vault</string>"));
        assert!(!text.contains("AI2_VAULT_KEYCHAIN"));
        assert_eq!(
            plist_path(Path::new("/Users/a"), UI_LABEL),
            Path::new("/Users/a/Library/LaunchAgents/africa.openwork.ai2-ui.plist")
        );
    }

    #[test]
    fn a_named_dashboard_label_port_and_keychain_reach_the_launch_agent() {
        let text = ui_plist(
            Path::new("/usr/local/bin/ai2"),
            Path::new("/h"),
            "test.ai2-ui",
            8123,
            Some(Path::new("/t/v.keychain-db")),
        );
        assert!(text.contains("<string>test.ai2-ui</string>"));
        assert!(text.contains("<string>--port</string>\n\t\t<string>8123</string>"));
        assert!(text.contains(
            "<string>/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>\n\t\t<key>AI2_VAULT_KEYCHAIN</key>\n\t\t<string>/t/v.keychain-db</string>\n\t</dict>"
        ));
        let odd = ui_plist(Path::new("/o/<&>/ai2"), Path::new("/h"), UI_LABEL, 1, None);
        assert!(odd.contains("<string>/o/&lt;&amp;&gt;/ai2</string>"));
    }

    #[test]
    fn the_dashboard_label_and_port_follow_the_environment_or_the_defaults() {
        assert_eq!(ui_label(None), "africa.openwork.ai2-ui");
        assert_eq!(ui_label(Some(String::new())), UI_LABEL);
        assert_eq!(ui_label(Some("test.ai2-ui".into())), "test.ai2-ui");
        assert_eq!(ui_port(None), 7797);
        assert_eq!(ui_port(Some("8001".into())), 8001);
        assert_eq!(ui_port(Some("not a port".into())), UI_PORT);
        assert_eq!(
            Dashboard::default(),
            Dashboard {
                label: UI_LABEL.into(),
                port: 7797,
                in_service: false
            }
        );
    }

    #[test]
    fn the_dashboard_user_unit_runs_ui_as_the_user_and_starts_with_the_session() {
        let text = ui_unit(
            Path::new("/home/ada/.local/bin/ai2"),
            Path::new("/home/ada/.ai2"),
            7797,
        );
        for line in [
            "[Service]",
            "ExecStart=/home/ada/.local/bin/ai2 ui --no-open --port 7797",
            "Environment=AI2_HOME=/home/ada/.ai2",
            "Environment=AI2_UI_SERVICE=1",
            "Restart=always",
            "UMask=0077",
            "WantedBy=default.target",
        ] {
            assert!(text.lines().any(|l| l == line), "{line}");
        }
        assert!(!text.contains("User="));
        assert!(!text.contains("multi-user.target"));
        let odd = ui_unit(Path::new("/opt/my ai2/ai2"), Path::new("/home/a b/.ai2"), 1);
        assert!(odd.contains("ExecStart=\"/opt/my ai2/ai2\" ui --no-open --port 1"));
        assert!(odd.contains("Environment=\"AI2_HOME=/home/a b/.ai2\""));
        assert_eq!(
            ui_unit_path(Path::new("/home/ada")),
            Path::new("/home/ada/.config/systemd/user/ai2-ui.service")
        );
        assert_eq!(
            ui_restart(true, 501, UI_LABEL),
            "launchctl kickstart -k gui/501/africa.openwork.ai2-ui"
        );
        assert_eq!(
            ui_restart(false, 1000, UI_LABEL),
            "systemctl --user restart ai2-ui.service"
        );
    }

    #[test]
    fn odd_binary_paths_are_quoted_for_systemd() {
        let text = unit(Path::new("/opt/my ai2/ai2"), "/c", 1000);
        assert!(text.contains("ExecStart=\"/opt/my ai2/ai2\" vault serve"));
    }
}
