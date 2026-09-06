//! Shared helpers for the wit-java-core integration test suite (phase D9).

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use wit_java_core::config::{GenerateOptions, InterfaceStyle, OptionStyle, Role, U64Style};

/// Workspace root (the `wit-java` repository).
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Vendored conformance snapshot (phase A3: vendored per mapping version).
pub fn cases_dir() -> PathBuf {
    workspace_root().join("tests/conformance/v1/cases")
}

pub fn checker_path() -> PathBuf {
    workspace_root().join("tests/conformance/v1/checker/Checker.java")
}

/// Path to a `java` executable for the Tier-1 checker (any JDK ≥ 11;
/// single-file source launcher).
pub fn java_path() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("WIT_JAVA_HOME") {
        let c = PathBuf::from(&home).join("bin/java");
        if c.exists() {
            return Some(c);
        }
    }
    let probe = Command::new("java").arg("-version").output();
    match probe {
        Ok(out) if out.status.success() => Some(PathBuf::from("java")),
        _ => None,
    }
}

/// A JDK 17 toolchain for `javac`/`javadoc` verification (DESIGN §14: 17 is
/// the verification baseline). Resolution order: `WIT_JAVA17_HOME`, `~/.local/jdks/jdk-17*`,
/// `JAVA_HOME`, system PATH.
pub fn java17() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("WIT_JAVA17_HOME") {
        let home = PathBuf::from(home);
        if home.join("bin/javac").exists() {
            return Some(home.join("bin/javac"));
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let mut candidates: Vec<PathBuf> = glob_walk(&PathBuf::from(home).join(".local/jdks"));
        candidates.sort();
        if let Some(home) = candidates.pop() {
            return Some(home.join("bin/javac"));
        }
    }
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        let c = PathBuf::from(home).join("bin/javac");
        if c.exists() {
            return Some(c);
        }
    }
    let probe = Command::new("javac").arg("-version").output();
    if matches!(probe, Ok(out) if out.status.success()) {
        return Some(PathBuf::from("javac"));
    }
    None
}

#[cfg(target_os = "macos")]
fn glob_walk(jdks: &Path) -> Vec<PathBuf> {
    // macOS layout: <jdk>/Contents/Home
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(jdks) {
        for e in entries.flatten() {
            let home = e.path().join("Contents/Home/bin/javac");
            if home.exists() && e.file_name().to_string_lossy().starts_with("jdk-17") {
                out.push(e.path().join("Contents/Home"));
            }
        }
    }
    out
}

#[cfg(not(target_os = "macos"))]
fn glob_walk(jdks: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(jdks) {
        for e in entries.flatten() {
            let home = e.path().join("bin/javac");
            if home.exists() && e.file_name().to_string_lossy().starts_with("jdk-17") {
                out.push(e.path());
            }
        }
    }
    out
}

/// Maps case.toml `options` strings onto `GenerateOptions`.
pub fn options_from(case_dir: &Path, options: &[String]) -> GenerateOptions {
    let mut opts = GenerateOptions::default();
    for o in options {
        match o.as_str() {
            "--no-support" => opts.no_support = true,
            "--all-features" => opts.all_features = true,
            "--option-style=optional" => opts.option_style = OptionStyle::Optional,
            "--option-style=nullable" => opts.option_style = OptionStyle::Nullable,
            "--u64=long" => opts.u64_style = U64Style::Long,
            "--u64=BigInteger" => opts.u64_style = U64Style::BigInteger,
            "--interface-package-style=nested" => opts.interface_style = InterfaceStyle::Nested,
            "--interface-package-style=flat" => opts.interface_style = InterfaceStyle::Flat,
            "--role=host" => opts.role = Role::Host,
            "--role=guest" => opts.role = Role::Guest,
            "--role=both" => opts.role = Role::Both,
            other => {
                if let Some(map) = other.strip_prefix("--package-map=") {
                    let path = case_dir.join(map);
                    let src = std::fs::read_to_string(&path)
                        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
                    opts.package_map =
                        wit_java_core::config::parse_package_map(&src).expect("package map");
                } else if let Some(feature) = other.strip_prefix("--features=") {
                    opts.features.push(feature.to_string());
                } else {
                    panic!("test harness does not understand option `{other}`");
                }
            }
        }
    }
    opts
}

/// The input path for a case (`src.wit` or `src/`).
pub fn case_source(case_dir: &Path) -> PathBuf {
    let file = case_dir.join("src.wit");
    if file.exists() {
        file
    } else {
        case_dir.join("src")
    }
}

/// Minimal TOML access for the case files (no serde dependency here).
pub fn case_options(case_dir: &Path) -> Vec<String> {
    let src = std::fs::read_to_string(case_dir.join("case.toml")).expect("case.toml");
    let value: toml::Value = toml::from_str(&src).expect("valid toml");
    value
        .get("options")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|v| v.as_str().expect("string option").to_string())
                .collect()
        })
        .unwrap_or_default()
}

pub fn expected_error_code(case_dir: &Path) -> String {
    let src =
        std::fs::read_to_string(case_dir.join("expected-error.toml")).expect("expected-error.toml");
    let value: toml::Value = toml::from_str(&src).expect("valid toml");
    value["code"].as_str().expect("code").to_string()
}

pub fn all_case_names() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(cases_dir())
        .expect("cases dir")
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    names
}

pub fn is_positive(name: &str) -> bool {
    cases_dir().join(name).join("expected").exists()
}

/// Fresh unique temp dir.
pub fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wit-java-test-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Writes generated files to a directory.
pub fn write(files: &[wit_java_core::GeneratedFile], root: &Path) {
    for f in files {
        let p = root.join(&f.path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, &f.content).unwrap();
    }
}
