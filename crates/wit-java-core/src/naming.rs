//! Name forms, the frozen reserved-word list, mangling, version segments and
//! Java package computation (mapping spec §3–§4).

use crate::diagnostic::{Code, Diagnostic};
use semver::Version;

/// Java keywords, reserved literals, `_` and `var` (spec §4.2 group 1).
pub const RESERVED_KEYWORDS: &[&str] = &[
    "abstract",
    "assert",
    "boolean",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extends",
    "final",
    "finally",
    "float",
    "for",
    "goto",
    "if",
    "implements",
    "import",
    "instanceof",
    "int",
    "interface",
    "long",
    "native",
    "new",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "short",
    "static",
    "strictfp",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "transient",
    "try",
    "void",
    "volatile",
    "while",
    "true",
    "false",
    "null",
    "_",
    "var",
];

/// `java.lang.Object` methods mangled in every lowerCamel position
/// (spec §4.2 group 2).
pub const OBJECT_METHOD_GROUP: &[&str] = &[
    "wait",
    "notify",
    "notifyAll",
    "getClass",
    "toString",
    "hashCode",
    "equals",
    "clone",
    "finalize",
];

fn is_reserved(lower: &str) -> bool {
    RESERVED_KEYWORDS.contains(&lower) || OBJECT_METHOD_GROUP.contains(&lower)
}

/// lowerCamel of a kebab label, keyword-mangled, for use as a Java package
/// segment (spec §3.1: a reserved result is mangled, `class` → `class_`).
pub fn package_segment(kebab: &str) -> String {
    let seg = to_lower_camel(kebab);
    if is_reserved(&seg) {
        format!("{seg}_")
    } else {
        seg
    }
}

fn capitalize(word: &str) -> String {
    let mut cs = word.chars();
    match cs.next() {
        Some(c) => c.to_ascii_uppercase().to_string() + cs.as_str(),
        None => String::new(),
    }
}

pub fn to_upper_camel(kebab: &str) -> String {
    kebab.split('-').map(capitalize).collect()
}

pub fn to_lower_camel(kebab: &str) -> String {
    let mut words = kebab.split('-');
    let first = words.next().unwrap_or("").to_string();
    words.map(capitalize).fold(first, |mut acc, w| {
        acc.push_str(&w);
        acc
    })
}

pub fn to_upper_snake(kebab: &str) -> String {
    kebab
        .split('-')
        .collect::<Vec<_>>()
        .join("_")
        .to_uppercase()
}

/// Mangling loop (spec §4.3): reserved names get `_`; repeat-collisions keep
/// appending. `taken` reports names already assigned in the same scope.
pub fn mangle(
    name: &str,
    form: NameForm,
    taken: &dyn Fn(&str) -> bool,
) -> Result<String, Diagnostic> {
    let mut candidate = match form {
        NameForm::LowerCamel if is_reserved(name) => format!("{name}_"),
        _ => name.to_string(),
    };
    for _ in 0..64 {
        if !taken(&candidate) {
            return Ok(candidate);
        }
        candidate.push('_');
    }
    Err(Diagnostic::new(
        Code::ManglingCollision,
        format!("cannot find a free Java name for `{name}` in this scope"),
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameForm {
    UpperCamel,
    LowerCamel,
    UpperSnake,
    PackageSegment,
}

/// Version segment (spec §3.2). Build metadata is ignored.
pub fn version_segment(version: &Version) -> String {
    let mut seg = if version.major > 0 {
        format!("v{}", version.major)
    } else {
        format!("v0_{}", version.minor)
    };
    if !version.pre.is_empty() {
        seg.push('_');
        seg.push_str(&sanitize_prerelease(&version.pre.to_string()));
    }
    seg
}

/// `sanitize(s)` per spec §3.2.
pub fn sanitize_prerelease(s: &str) -> String {
    let mapped: String = s
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() {
                c
            } else {
                '_'
            }
        })
        .collect();
    let mut out = String::new();
    for c in mapped.chars() {
        if c == '_' && out.ends_with('_') {
            continue;
        }
        out.push(c);
    }
    let out = out.trim_matches('_');
    let out = if out.is_empty() { "pre" } else { out };
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{out}")
    } else {
        out.to_string()
    }
}

/// A Java fully-qualified name. `name` may contain `.` for nested types
/// (`Result.Ok`); imports use the outermost segment.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Fqn {
    pub package: String,
    pub name: String,
}

impl Fqn {
    pub fn new(package: impl Into<String>, name: impl Into<String>) -> Self {
        Fqn {
            package: package.into(),
            name: name.into(),
        }
    }

    pub fn outer_simple(&self) -> &str {
        self.name.split('.').next().unwrap_or(&self.name)
    }

    pub fn is_java(&self) -> bool {
        self.package.starts_with("java.")
    }
}

impl std::fmt::Display for Fqn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.package, self.name)
    }
}

/// Java package for a WIT package with `--package-map` precedence
/// (spec §3.1–§3.4). Override keys are matched most-specific first: exact
/// version `ns:pkg@x.y.z`, then major.minor `ns:pkg@x.y` (the spec's
/// examples use the short form), then unversioned `ns:pkg`.
pub fn java_package(
    namespace: &str,
    name: &str,
    version: Option<&Version>,
    package_root: Option<&str>,
    package_map: &std::collections::BTreeMap<String, String>,
) -> String {
    let unversioned_key = format!("{namespace}:{name}");
    let versioned_keys: Vec<String> = match version {
        Some(v) => vec![
            format!("{namespace}:{name}@{v}"),
            format!("{namespace}:{name}@{}.{}", v.major, v.minor),
        ],
        None => Vec::new(),
    };

    for key in versioned_keys
        .iter()
        .chain(std::iter::once(&unversioned_key))
    {
        if let Some(mapped) = package_map.get(key) {
            return mapped.clone();
        }
    }

    let ns = package_segment(namespace);
    let pkg = package_segment(name);
    let mut parts: Vec<String> = Vec::new();
    if let Some(root) = package_root {
        parts.push(root.to_string());
    }
    parts.push(ns);
    parts.push(pkg);
    if let Some(v) = version {
        parts.push(version_segment(v));
    }
    parts.join(".")
}

/// Collect diagnostics helper used by tests and the mapper.
pub fn err(code: Code, msg: impl Into<String>) -> Diagnostic {
    Diagnostic::new(code, msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn kebab_conversions() {
        for (kebab, upper, lower, snake) in [
            ("foo", "Foo", "foo", "FOO"),
            ("foo-bar", "FooBar", "fooBar", "FOO_BAR"),
            (
                "descriptor-flags",
                "DescriptorFlags",
                "descriptorFlags",
                "DESCRIPTOR_FLAGS",
            ),
            ("http-proxy", "HttpProxy", "httpProxy", "HTTP_PROXY"),
            ("a-1b-c", "A1bC", "a1bC", "A_1B_C"), // words are [a-z][0-9a-z]*
        ] {
            assert_eq!(to_upper_camel(kebab), upper);
            assert_eq!(to_lower_camel(kebab), lower);
            assert_eq!(to_upper_snake(kebab), snake);
        }
    }

    #[test]
    fn mangling_reserved_and_object_group() {
        let none = |_: &str| false;
        assert_eq!(
            mangle("class", NameForm::LowerCamel, &none).unwrap(),
            "class_"
        );
        assert_eq!(
            mangle("wait", NameForm::LowerCamel, &none).unwrap(),
            "wait_"
        );
        assert_eq!(
            mangle("toString", NameForm::LowerCamel, &none).unwrap(),
            "toString_"
        );
        // group-2 names can only surface in lowerCamel positions; the
        // UpperCamel form of the same kebab label never equals an Object
        // method name, so it needs no mangling
        assert_eq!(
            mangle("ToString", NameForm::UpperCamel, &none).unwrap(),
            "ToString"
        );
        assert_eq!(
            mangle("classX", NameForm::LowerCamel, &none).unwrap(),
            "classX"
        );
    }

    #[test]
    fn mangle_repeats_until_free() {
        let taken = |s: &str| s == "class_" || s == "class__";
        assert_eq!(
            mangle("class", NameForm::LowerCamel, &taken).unwrap(),
            "class___"
        );
    }

    #[test]
    fn package_segments_are_keyword_mangled() {
        assert_eq!(package_segment("incoming-handler"), "incomingHandler");
        assert_eq!(package_segment("class"), "class_");
        assert_eq!(package_segment("default"), "default_");
        assert_eq!(package_segment("wait"), "wait_");
        assert_eq!(package_segment("types"), "types");
    }

    #[test]
    fn package_map_partial_version_matches() {
        let mut map = BTreeMap::new();
        map.insert("wasi:io@0.2".to_string(), "org.example.io.v2".to_string());
        let v = Version::parse("0.2.8").unwrap();
        // the spec's short-form key `ns:pkg@x.y` matches x.y.z
        assert_eq!(
            java_package("wasi", "io", Some(&v), None, &map),
            "org.example.io.v2"
        );
        // ...but not a different minor
        let v2 = Version::parse("0.3.0").unwrap();
        assert_eq!(
            java_package("wasi", "io", Some(&v2), None, &map),
            "wasi.io.v0_3"
        );
        // exact version still beats major.minor
        map.insert(
            "wasi:io@0.2.8".to_string(),
            "org.example.io.exact".to_string(),
        );
        assert_eq!(
            java_package("wasi", "io", Some(&v), None, &map),
            "org.example.io.exact"
        );
    }

    #[test]
    fn version_segments() {
        let v = |s: &str| Version::parse(s).unwrap();
        assert_eq!(version_segment(&v("1.2.3")), "v1");
        assert_eq!(version_segment(&v("0.2.3")), "v0_2");
        assert_eq!(version_segment(&v("0.3.0-draft")), "v0_3_draft");
        assert_eq!(
            version_segment(&v("0.2.0-rc-2023-11-10")),
            "v0_2_rc_2023_11_10"
        );
        assert_eq!(version_segment(&v("0.2.0-alpha.1+b7c")), "v0_2_alpha_1");
        assert_eq!(version_segment(&v("2.0.0-rc.1")), "v2_rc_1");
    }

    #[test]
    fn sanitize_edge_cases() {
        assert_eq!(sanitize_prerelease("draft"), "draft");
        assert_eq!(sanitize_prerelease("RC-1"), "rc_1");
        assert_eq!(sanitize_prerelease("alpha.1"), "alpha_1");
        assert_eq!(sanitize_prerelease("1"), "_1");
        assert_eq!(sanitize_prerelease("--"), "pre");
        assert_eq!(sanitize_prerelease("a--b"), "a_b");
    }

    #[test]
    fn java_package_precedence() {
        let mut map = BTreeMap::new();
        map.insert("wasi:http".to_string(), "org.example.http".to_string());
        map.insert("wasi:io@0.2.8".to_string(), "org.example.io".to_string());
        let v = Version::parse("0.2.8").unwrap();
        // versioned exact beats unversioned
        assert_eq!(
            java_package("wasi", "io", Some(&v), None, &map),
            "org.example.io"
        );
        // unversioned override for a versioned package
        let v2 = Version::parse("0.3.0").unwrap();
        assert_eq!(
            java_package("wasi", "http", Some(&v2), None, &map),
            "org.example.http"
        );
        // default rule otherwise
        assert_eq!(
            java_package("wasi", "cli", Some(&v), None, &map),
            "wasi.cli.v0_2"
        );
        // package root prefix
        assert_eq!(
            java_package("wasi", "cli", Some(&v), Some("com.acme"), &map),
            "com.acme.wasi.cli.v0_2"
        );
    }
}
