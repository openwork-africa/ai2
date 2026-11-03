use crate::host::{Host, Os};
use crate::steps::supports_containers;
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StepInfo {
    pub id: &'static str,
    pub title: &'static str,
    pub why: &'static str,
    pub fact: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Field {
    pub label: &'static str,
    pub secret: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InputInfo {
    pub kind: &'static str,
    pub title: &'static str,
    pub why: &'static str,
    pub fields: Vec<Field>,
}

fn step(id: &'static str, title: &'static str, why: &'static str, fact: &'static str) -> StepInfo {
    StepInfo {
        id,
        title,
        why,
        fact,
    }
}

fn containers(os: &Os) -> StepInfo {
    match os {
        Os::Macos { .. } if supports_containers(os) => step(
            "containers",
            "Containers",
            "Agents work inside their own tiny Linux VM, so they can only see what you give them.",
            "Apple's container boots a whole Linux virtual machine for each sandbox in well under a second. The vault's socket is the only way out.",
        ),
        Os::Linux { .. } if supports_containers(os) => step(
            "containers",
            "Sandboxes",
            "Each sandbox gets its own socket to the vault, and only you and the vault's group can open it.",
            "Unix sockets have guarded local services since 4.2BSD, in 1983. Each one here opens only for you and the vault's group.",
        ),
        _ => step(
            "containers",
            "Containers",
            "This machine cannot keep agents in a container yet; setup carries on, with a warning.",
            "Apple's container needs macOS 26 on Apple silicon; Docker support is coming. Until then an agent on the host can read what you can.",
        ),
    }
}

pub fn steps_info(os: &Os) -> Vec<StepInfo> {
    let mac = matches!(os, Os::Macos { .. });
    vec![
        containers(os),
        if mac {
            step(
                "master-key",
                "Master key",
                "One random key, kept by the Mac, locks every secret in the vault.",
                "The login keychain has guarded Mac passwords since Mac OS 8.6, in 1999. Only the ai2 binary may read this item.",
            )
        } else {
            step(
                "master-key",
                "Master key",
                "One random key, sealed by systemd-creds, locks every secret in the vault.",
                "systemd-creds arrived in systemd 250, in December 2021. With a TPM2 chip the key can only be opened on this very board.",
            )
        },
        if mac {
            step(
                "presence",
                "Human check",
                "Every change to the vault needs Touch ID or your password, so a process cannot approve itself.",
                "Your fingerprint never leaves the Secure Enclave; macOS, and ai2, only ever get a yes or a no.",
            )
        } else {
            step(
                "presence",
                "Human check",
                "Every change to the vault needs your login password, or this passphrase if the machine cannot ask for it, so a process cannot approve itself.",
                "Argon2id won the Password Hashing Competition in 2015. It is slow and hungry for memory on purpose, so guessing is expensive.",
            )
        },
        if mac {
            step(
                "daemon",
                "Vault service",
                "The one process that ever holds a secret; it starts with you and injects keys into requests on the agent's behalf.",
                "launchd has been PID 1 on every Mac since Tiger, in 2005. It will start the vault at login and again if it ever stops.",
            )
        } else {
            step(
                "daemon",
                "Vault service",
                "The one process that ever holds a secret, under its own user, with the rest of the system read-only to it.",
                "ProtectSystem=strict makes the whole filesystem read-only for the service except the two folders it is handed.",
            )
        },
        if mac {
            step(
                "dashboard",
                "Dashboard",
                "This page starts when you log in and keeps your apps running, at one address you can bookmark.",
                "A LaunchAgent runs as you, not as root, so the dashboard can reach only what you can. launchd brings it back if it ever stops.",
            )
        } else {
            step(
                "dashboard",
                "Dashboard",
                "This page starts when you log in and keeps your apps running, at one address you can bookmark.",
                "A systemd user unit starts when you log in and stops when you log out; loginctl enable-linger keeps it up even then.",
            )
        },
    ]
}

pub fn inputs_info() -> Vec<InputInfo> {
    vec![
        InputInfo {
            kind: "passphrase",
            title: "Choose the vault passphrase",
            why: "It is asked for vault changes when the machine cannot ask for your login password. It is hashed with Argon2id; the vault never keeps it.",
            fields: vec![
                Field {
                    label: "Passphrase",
                    secret: true,
                },
                Field {
                    label: "The same again",
                    secret: true,
                },
            ],
        },
        InputInfo {
            kind: "sudo",
            title: "Your password, for sudo",
            why: "Setup writes to /etc and installs a system service; sudo asks once and remembers for a few minutes.",
            fields: vec![Field {
                label: "Password",
                secret: true,
            }],
        },
    ]
}

impl Host {
    pub fn to_json(&self) -> Value {
        let (os, version, arm64, systemd) = match &self.os {
            Os::Macos { version, arm64 } => ("macos", json!(version), json!(arm64), Value::Null),
            Os::Linux { systemd } => ("linux", Value::Null, Value::Null, json!(systemd)),
            Os::Other { name } => (name.as_str(), Value::Null, Value::Null, Value::Null),
        };
        json!({
            "os": os,
            "version": version,
            "arm64": arm64,
            "systemd": systemd,
            "user": self.user,
            "supports_containers": supports_containers(&self.os),
        })
    }
}
