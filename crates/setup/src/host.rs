use crate::runner::{Cmd, Runner};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "os", rename_all = "lowercase")]
pub enum Os {
    Macos { version: u32, arm64: bool },
    Linux { systemd: Option<u32> },
    Other { name: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Host {
    pub os: Os,
    pub user: String,
    pub uid: u32,
    pub home: PathBuf,
}

pub fn macos_major(version: &str) -> Option<u32> {
    version.trim().split('.').next()?.parse().ok()
}

pub fn systemd_version(text: &str) -> Option<u32> {
    let mut words = text.lines().next()?.split_whitespace();
    (words.next()? == "systemd")
        .then(|| words.next()?.parse().ok())
        .flatten()
}

pub fn invoker(
    uid: u32,
    sudo_user: Option<String>,
    sudo_uid: Option<String>,
) -> (Option<String>, u32) {
    if uid != 0 {
        return (None, uid);
    }
    match sudo_uid.and_then(|id| id.parse().ok()) {
        Some(id) => (sudo_user.filter(|name| !name.is_empty()), id),
        None => (None, uid),
    }
}

impl Host {
    pub fn detect(runner: &dyn Runner) -> Self {
        let os = match std::env::consts::OS {
            "macos" => Os::Macos {
                version: runner
                    .run(&Cmd::new("sw_vers", &["-productVersion"]))
                    .ok()
                    .and_then(|ran| macos_major(&ran.out))
                    .unwrap_or(0),
                arm64: runner
                    .run(&Cmd::new("sysctl", &["-n", "hw.optional.arm64"]))
                    .is_ok_and(|ran| ran.out.trim() == "1"),
            },
            "linux" => Os::Linux {
                systemd: runner
                    .exists(std::path::Path::new("/run/systemd/system"))
                    .then(|| {
                        runner
                            .run(&Cmd::new("systemctl", &["--version"]))
                            .ok()
                            .and_then(|ran| systemd_version(&ran.out))
                    })
                    .flatten(),
            },
            other => Os::Other { name: other.into() },
        };
        let (sudo_user, sudo_uid) = invoker(
            unsafe { libc::getuid() },
            std::env::var("SUDO_USER").ok(),
            std::env::var("SUDO_UID").ok(),
        );
        let user = sudo_user.unwrap_or_else(|| {
            runner
                .run(&Cmd::new("id", &["-un"]))
                .map(|ran| ran.out.trim().to_string())
                .unwrap_or_default()
        });
        Self {
            os,
            user,
            uid: sudo_uid,
            home: std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default(),
        }
    }
}

#[cfg(test)]
impl Host {
    pub fn mac(version: u32, arm64: bool) -> Self {
        Self {
            os: Os::Macos { version, arm64 },
            user: "ada".into(),
            uid: 501,
            home: "/Users/ada".into(),
        }
    }

    pub fn linux(systemd: Option<u32>) -> Self {
        Self {
            os: Os::Linux { systemd },
            user: "ada".into(),
            uid: 1000,
            home: "/home/ada".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_read_from_sw_vers_and_systemctl() {
        assert_eq!(macos_major("26.4\n"), Some(26));
        assert_eq!(macos_major("15.7.1"), Some(15));
        assert_eq!(macos_major(""), None);
        assert_eq!(
            systemd_version("systemd 255 (255.4-1ubuntu8)\n+PAM +AUDIT"),
            Some(255)
        );
        assert_eq!(systemd_version("systemd 249"), Some(249));
        assert_eq!(systemd_version("something else"), None);
    }

    #[test]
    fn under_sudo_the_installing_user_is_the_one_who_ran_sudo() {
        assert_eq!(
            invoker(0, Some("ada".into()), Some("1000".into())),
            (Some("ada".into()), 1000)
        );
        assert_eq!(invoker(0, None, None), (None, 0));
        assert_eq!(
            invoker(1000, Some("eve".into()), Some("1001".into())),
            (None, 1000)
        );
    }
}
