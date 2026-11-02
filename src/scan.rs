use crate::model::Model;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

const SKIPPED: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    "vendor",
    ".venv",
    ".ai2",
];
const MANIFESTS: &[&str] = &[
    "package.json",
    "deno.json",
    "deno.jsonc",
    "Cargo.toml",
    "go.mod",
    "requirements.txt",
    "pyproject.toml",
    "Pipfile",
    "Gemfile",
    "composer.json",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "pubspec.yaml",
    "mix.exs",
    "Package.swift",
];
const MANIFEST_DEPTH: usize = 4;
const MANIFESTS_KEPT: usize = 30;
const MANIFEST_CHARS: usize = 8_000;

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Spec {
    pub os: String,
    pub languages: Vec<Language>,
    pub libraries: Vec<Library>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Language {
    pub name: String,
    pub lines: usize,
    pub share: f64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Library {
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub ecosystem: String,
}

pub fn scan(folder: &Path, model: Option<&Model>, notes: &mut Vec<String>) -> Spec {
    scan_skipping(folder, model, notes, None)
}

pub fn scan_skipping(
    folder: &Path,
    model: Option<&Model>,
    notes: &mut Vec<String>,
    skip: Option<&Path>,
) -> Spec {
    let libraries = syft(folder).unwrap_or_else(|| match model {
        Some(model) => from_manifests(folder, model).unwrap_or_else(|error| {
            notes.push(format!(
                "The libraries could not be read from the manifests: {error}"
            ));
            Vec::new()
        }),
        None => {
            notes.push("No syft and no model: the libraries are unknown.".into());
            Vec::new()
        }
    });
    Spec {
        os: os_info::get().to_string(),
        languages: languages_skipping(folder, skip),
        libraries,
    }
}

fn languages_skipping(folder: &Path, skip: Option<&Path>) -> Vec<Language> {
    let mut ignored: Vec<&str> = SKIPPED.to_vec();
    let extra;
    if let Some(skip) = skip {
        if let Ok(rel) = skip.strip_prefix(folder) {
            if let Some(first) = rel.components().next() {
                extra = first.as_os_str().to_string_lossy().to_string();
                ignored.push(&extra);
            }
        }
    }
    let mut found = tokei::Languages::new();
    found.get_statistics(&[folder], &ignored, &tokei::Config::default());
    let total: usize = found.values().map(|language| language.code).sum();
    let mut languages: Vec<Language> = found
        .iter()
        .filter(|(_, language)| language.code > 0)
        .map(|(kind, language)| Language {
            name: kind.name().to_string(),
            lines: language.code,
            share: percent(language.code as f64, total as f64),
        })
        .collect();
    languages.sort_by_key(|a| std::cmp::Reverse(a.lines));
    languages
}

#[derive(Deserialize)]
struct SyftOutput {
    artifacts: Vec<SyftArtifact>,
}

#[derive(Deserialize)]
struct SyftArtifact {
    name: String,
    version: String,
    #[serde(rename = "type")]
    kind: String,
}

fn syft(folder: &Path) -> Option<Vec<Library>> {
    let out = Command::new("syft")
        .arg("scan")
        .arg(format!("dir:{}", folder.display()))
        .args(["-o", "syft-json", "-q"])
        .output()
        .ok()?;
    let found: SyftOutput = serde_json::from_slice(&out.stdout)
        .ok()
        .filter(|_| out.status.success())?;
    Some(
        found
            .artifacts
            .into_iter()
            .map(|a| Library {
                ecosystem: ecosystem(&a.kind).to_string(),
                name: a.name,
                version: a.version,
            })
            .collect(),
    )
}

fn ecosystem(kind: &str) -> &str {
    match kind {
        "npm" => "npm",
        "rust-crate" => "crates.io",
        "python" => "PyPI",
        "go-module" => "Go",
        "gem" => "RubyGems",
        "java-archive" | "jenkins-plugin" => "Maven",
        "dotnet" => "NuGet",
        "php-composer" => "Packagist",
        "dart-pub" => "Pub",
        "hex" => "Hex",
        "swift" => "SwiftURL",
        other => other,
    }
}

#[derive(Deserialize)]
struct Extracted {
    libraries: Vec<Library>,
}

fn from_manifests(folder: &Path, model: &Model) -> Result<Vec<Library>> {
    let mut manifests = Vec::new();
    find_manifests(folder, 0, &mut manifests);
    if manifests.is_empty() {
        return Ok(Vec::new());
    }
    let input = manifests
        .iter()
        .map(|path| {
            let text: String = std::fs::read_to_string(path)
                .unwrap_or_default()
                .chars()
                .take(MANIFEST_CHARS)
                .collect();
            format!(
                "--- {}\n{text}",
                path.strip_prefix(folder).unwrap_or(path).display()
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    Ok(model
        .ask::<Extracted>(crate::prompts::SPEC, &input)?
        .libraries)
}

fn find_manifests(dir: &Path, depth: usize, found: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if found.len() >= MANIFESTS_KEPT {
            return;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if depth < MANIFEST_DEPTH && !name.starts_with('.') && !SKIPPED.contains(&name.as_str())
            {
                find_manifests(&path, depth + 1, found);
            }
        } else if MANIFESTS.contains(&name.as_str()) {
            found.push(path);
        }
    }
}

pub fn percent(part: f64, whole: f64) -> f64 {
    if whole <= 0.0 {
        0.0
    } else {
        (part / whole * 1000.0).round() / 10.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_languages_by_share_and_finds_manifests() {
        let dir = std::env::temp_dir().join(format!("ai2-scan-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("app/node_modules/x")).unwrap();
        std::fs::write(
            dir.join("main.rs"),
            "fn main() {\n    println!(\"hi\");\n}\n",
        )
        .unwrap();
        std::fs::write(dir.join("app/index.js"), "console.log(1);\n").unwrap();
        std::fs::write(dir.join("app/package.json"), "{}").unwrap();
        std::fs::write(dir.join("app/node_modules/x/package.json"), "{}").unwrap();
        std::fs::create_dir_all(dir.join(".ai2")).unwrap();
        std::fs::write(
            dir.join(".ai2/report.html"),
            "<html><body>hi</body></html>\n",
        )
        .unwrap();
        let languages = languages_skipping(&dir, None);
        assert!(languages.iter().all(|l| l.name != "HTML"));
        assert_eq!(languages[0].name, "Rust");
        assert_eq!(languages.iter().map(|l| l.share).sum::<f64>(), 100.0);
        let mut manifests = Vec::new();
        find_manifests(&dir, 0, &mut manifests);
        assert_eq!(manifests, vec![dir.join("app/package.json")]);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
