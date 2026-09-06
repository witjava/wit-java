//! Generator options (spec §2, §11) and their validation.
//!
//! Validation failures here are usage errors (exit 1, plain message), not
//! mapping diagnostics: the frozen `WJnnnn` codes are reserved for the
//! conditions in the spec's error-code registry.

use crate::naming::RESERVED_KEYWORDS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterfaceStyle {
    Nested,
    Flat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionStyle {
    Optional,
    Nullable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum U64Style {
    Long,
    BigInteger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Host,
    Guest,
    Both,
}

pub const DEFAULT_SUPPORT_PACKAGE: &str = "io.github.witjava.support";

#[derive(Debug, Clone)]
pub struct GenerateOptions {
    /// `--package-root`
    pub package_root: Option<String>,
    /// `--package-map` contents (already parsed).
    pub package_map: std::collections::BTreeMap<String, String>,
    /// `--interface-package-style`
    pub interface_style: InterfaceStyle,
    /// `--mapping-version`
    pub mapping_version: String,
    /// `--support-package`
    pub support_package: String,
    /// `--no-support`
    pub no_support: bool,
    /// `--option-style`
    pub option_style: OptionStyle,
    /// `--u64`
    pub u64_style: U64Style,
    /// `--role`
    pub role: Role,
    /// `--features`
    pub features: Vec<String>,
    /// `--all-features`
    pub all_features: bool,
    /// `--world` (empty = all worlds)
    pub worlds: Vec<String>,
}

impl Default for GenerateOptions {
    fn default() -> Self {
        GenerateOptions {
            package_root: None,
            package_map: Default::default(),
            interface_style: InterfaceStyle::Nested,
            mapping_version: crate::MAPPING_VERSION.to_string(),
            support_package: DEFAULT_SUPPORT_PACKAGE.to_string(),
            no_support: false,
            option_style: OptionStyle::Optional,
            u64_style: U64Style::Long,
            role: Role::Both,
            features: Vec::new(),
            all_features: false,
            worlds: Vec::new(),
        }
    }
}

fn valid_java_package(pkg: &str) -> bool {
    if pkg.is_empty() {
        return false;
    }
    pkg.split('.').all(|seg| {
        let mut cs = seg.chars();
        let identifier = matches!(cs.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
            && cs.all(|c| c.is_ascii_alphanumeric() || c == '_');
        identifier && !RESERVED_KEYWORDS.contains(&seg)
    })
}

impl GenerateOptions {
    /// Validates everything that must fail before any mapping happens
    /// (spec §3.4, §11). Errors are usage errors, not mapping diagnostics.
    pub fn validate(&self) -> Result<(), String> {
        if self.mapping_version != crate::MAPPING_VERSION {
            return Err(format!(
                "unsupported mapping version `{}` (this tool implements `{}`)",
                self.mapping_version,
                crate::MAPPING_VERSION
            ));
        }
        if let Some(root) = &self.package_root {
            if !valid_java_package(root) {
                return Err(format!(
                    "`--package-root` is not a valid Java package name: `{root}`"
                ));
            }
        }
        if !valid_java_package(&self.support_package) {
            return Err(format!(
                "`--support-package` is not a valid Java package name: `{}`",
                self.support_package
            ));
        }
        for (k, v) in &self.package_map {
            if !valid_java_package(v) {
                return Err(format!(
                    "package-map value for `{k}` is not a valid Java package: `{v}`"
                ));
            }
        }
        Ok(())
    }
}

/// Parses `--package-map` TOML (spec §3.4).
pub fn parse_package_map(
    toml_src: &str,
) -> Result<std::collections::BTreeMap<String, String>, String> {
    let value: toml::Value =
        toml::from_str(toml_src).map_err(|e| format!("invalid package-map TOML: {e}"))?;
    let mut out = std::collections::BTreeMap::new();
    let table = value
        .as_table()
        .ok_or_else(|| "package-map must be a table".to_string())?;
    for (k, v) in table {
        let s = v
            .as_str()
            .ok_or_else(|| format!("package-map value for `{k}` must be a string"))?;
        out.insert(k.clone(), s.to_string());
    }
    Ok(out)
}
