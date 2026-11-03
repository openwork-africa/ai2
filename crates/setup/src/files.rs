use std::path::Path;

pub const LABEL: &str = "africa.openwork.ai2-vault";
pub const LABEL_ENV: &str = "AI2_VAULT_LABEL";
pub const UNIT: &str = "ai2-vault.service";
pub const UNIT_PATH: &str = "/etc/systemd/system/ai2-vault.service";
pub const USER: &str = "ai2-vault";
pub const ADMIN_GROUP: &str = "ai2-vault-admin";
pub const SERVICE_BIN: &str = "/usr/local/libexec/ai2-vault/ai2";

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
    fn odd_binary_paths_are_quoted_for_systemd() {
        let text = unit(Path::new("/opt/my ai2/ai2"), "/c", 1000);
        assert!(text.contains("ExecStart=\"/opt/my ai2/ai2\" vault serve"));
    }
}
