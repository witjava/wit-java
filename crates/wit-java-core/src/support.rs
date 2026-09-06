//! Support-package generation (spec §8): sources are normative, versioned
//! with the mapping, embedded at compile time. The package declaration line
//! is the ONLY templated part; with the default FQN the output is
//! byte-identical to the spec repository's conformance data.

pub const DEFAULT_PACKAGE: &str = "io.github.witjava.support";

/// (file name, contents with `{{PACKAGE}}` placeholder)
pub const FILES: &[(&str, &str)] = &[
    (
        "package-info.java",
        include_str!("support/v1/package-info.java"),
    ),
    ("Unit.java", include_str!("support/v1/Unit.java")),
    ("Result.java", include_str!("support/v1/Result.java")),
    ("Nullable.java", include_str!("support/v1/Nullable.java")),
    (
        "WitGenerated.java",
        include_str!("support/v1/WitGenerated.java"),
    ),
    ("Tuple2.java", include_str!("support/v1/Tuple2.java")),
    ("Tuple3.java", include_str!("support/v1/Tuple3.java")),
    ("Tuple4.java", include_str!("support/v1/Tuple4.java")),
    ("Tuple5.java", include_str!("support/v1/Tuple5.java")),
    ("Tuple6.java", include_str!("support/v1/Tuple6.java")),
    ("Tuple7.java", include_str!("support/v1/Tuple7.java")),
    ("Tuple8.java", include_str!("support/v1/Tuple8.java")),
];

/// Returns the support sources for the given package FQN, as
/// (relative path, content) pairs.
pub fn sources(support_package: &str) -> Vec<(String, String)> {
    FILES
        .iter()
        .map(|(name, src)| {
            let content = src.replace("{{PACKAGE}}", support_package);
            let path = format!("{}/{}", support_package.replace('.', "/"), name);
            (path, content)
        })
        .collect()
}
