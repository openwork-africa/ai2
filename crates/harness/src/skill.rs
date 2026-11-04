use crate::setup::Setup;
use anyhow::{bail, Context, Result};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

pub const AI2: &str = include_str!("../../../SKILL.md");
pub const NAME: &str = "ai2";
pub const FILE: &str = "SKILL.md";

pub fn folder(setup: &Setup, home: &Path) -> Option<PathBuf> {
    let relative = Path::new(setup.skills.as_deref()?);
    let plain = relative
        .components()
        .all(|part| matches!(part, Component::Normal(_)));
    (plain && !relative.as_os_str().is_empty()).then(|| home.join(relative))
}

pub fn path(setup: &Setup, home: &Path) -> Option<PathBuf> {
    folder(setup, home).map(|dir| dir.join(NAME).join(FILE))
}

pub fn installed(setup: &Setup, home: &Path) -> bool {
    path(setup, home)
        .is_some_and(|file| std::fs::read_to_string(file).is_ok_and(|text| text == AI2))
}

fn linked(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
}

pub fn install(setup: &Setup, home: &Path) -> Result<PathBuf> {
    let Some(dir) = folder(setup, home) else {
        bail!("{} has no skills folder that ai2 knows of", setup.label);
    };
    let own = dir.join(NAME);
    let file = own.join(FILE);
    if linked(&own) || linked(&file) {
        bail!(
            "{} is a link; ai2 writes only inside {}",
            own.display(),
            dir.display()
        );
    }
    std::fs::create_dir_all(&own).with_context(|| format!("making {}", own.display()))?;
    let staged = own.join(format!(".{FILE}.{}", std::process::id()));
    let _ = std::fs::remove_file(&staged);
    let mut out = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staged)
        .with_context(|| format!("writing {}", staged.display()))?;
    out.write_all(AI2.as_bytes())?;
    out.sync_all()?;
    std::fs::rename(&staged, &file).with_context(|| format!("writing {}", file.display()))?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::Registry;

    fn every_file(dir: &Path, into: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                every_file(&path, into);
            } else {
                into.push(path);
            }
        }
    }

    #[test]
    fn the_skill_goes_only_into_the_harness_skills_folder() {
        let home = tempfile::tempdir().unwrap();
        let registry = Registry::public();
        let claude = registry.get("claude").unwrap();
        assert!(!installed(claude, home.path()));
        let written = install(claude, home.path()).unwrap();
        assert_eq!(written, home.path().join(".claude/skills/ai2/SKILL.md"));
        assert!(installed(claude, home.path()));
        install(claude, home.path()).unwrap();
        let mut files = Vec::new();
        every_file(home.path(), &mut files);
        assert_eq!(files, vec![written.clone()]);
        assert_eq!(std::fs::read_to_string(&written).unwrap(), AI2);
        assert!(AI2.starts_with("---\nname: ai2\n"));

        let gh = registry.get("gh").unwrap();
        assert!(install(gh, home.path()).is_err());
        let codex = registry.get("codex").unwrap();
        assert_eq!(
            install(codex, home.path()).unwrap(),
            home.path().join(".agents/skills/ai2/SKILL.md")
        );
    }

    #[test]
    fn a_link_or_a_folder_that_climbs_out_is_refused() {
        let home = tempfile::tempdir().unwrap();
        let away = tempfile::tempdir().unwrap();
        let registry = Registry::public();
        let claude = registry.get("claude").unwrap();
        std::fs::create_dir_all(home.path().join(".claude/skills")).unwrap();
        std::os::unix::fs::symlink(away.path(), home.path().join(".claude/skills/ai2")).unwrap();
        assert!(install(claude, home.path()).is_err());
        assert_eq!(std::fs::read_dir(away.path()).unwrap().count(), 0);

        let climbing = Setup {
            id: "x".into(),
            label: "X".into(),
            skills: Some("../outside".into()),
            ..Setup::default()
        };
        assert_eq!(folder(&climbing, home.path()), None);
        assert!(install(&climbing, home.path()).is_err());
        let rooted = Setup {
            skills: Some("/etc".into()),
            ..climbing
        };
        assert_eq!(folder(&rooted, home.path()), None);
    }

    #[test]
    fn every_known_skills_folder_is_under_home() {
        let home = Path::new("/home/someone");
        for setup in Registry::public().all() {
            if let Some(dir) = folder(setup, home) {
                assert!(dir.starts_with(home), "{}", setup.id);
            }
        }
        let registry = Registry::public();
        let with: Vec<&str> = registry
            .all()
            .iter()
            .filter(|setup| setup.skills.is_some())
            .map(|setup| setup.id.as_str())
            .collect();
        assert_eq!(
            with,
            [
                "claude",
                "codex",
                "opencode",
                "pi",
                "antigravity",
                "muse",
                "hermes",
                "fx"
            ]
        );
    }
}
