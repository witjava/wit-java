//! `wit-java-core` — reference implementation of the
//! [`wit-java-mapping`](https://github.com/witjava/wit-java-mapping) spec.
//!
//! Pipeline: `wit` (adapter) → `map` (mapping) → `ir` → `passes` (import
//! resolve) → `render` (deterministic renderer) → files.

pub mod config;
pub use config::{
    GenerateOptions, InterfaceStyle, OptionStyle, Role, U64Style, DEFAULT_SUPPORT_PACKAGE,
};
pub mod diagnostic;
pub mod ir;
pub mod map;
pub mod naming;
pub mod passes;
pub mod render;
pub mod support;
pub mod wit;

/// The mapping version this crate implements.
pub const MAPPING_VERSION: &str = "v1";

/// Tool version stamped into `// @generated` headers.
pub const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");

/// A generated file: path relative to `--out`, full content.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct GeneratedFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug)]
pub enum GenError {
    /// Mapping diagnostics (spec §10): exit code 1, never write output.
    Diagnostics(diagnostic::Diagnostics),
    /// Load/validation failures (parse errors, invalid options): exit 1.
    Input(anyhow::Error),
    /// Internal errors: exit code 2.
    Internal(anyhow::Error),
}

impl GenError {
    pub fn exit_code(&self) -> i32 {
        match self {
            GenError::Diagnostics(_) | GenError::Input(_) => 1,
            GenError::Internal(_) => 2,
        }
    }
}

impl std::fmt::Display for GenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GenError::Diagnostics(diags) => {
                for d in &diags.0 {
                    writeln!(f, "{d}")?;
                }
                Ok(())
            }
            GenError::Input(e) => write!(f, "{e:#}"),
            GenError::Internal(e) => write!(f, "{e:#}"),
        }
    }
}

/// Full pipeline (spec §11 `generate`; `check` = same pipeline, no writes).
pub fn generate(
    wit_path: &std::path::Path,
    opts: &config::GenerateOptions,
) -> Result<Vec<GeneratedFile>, GenError> {
    opts.validate()
        .map_err(|d| GenError::Input(anyhow::anyhow!("{d}")))?;
    let (resolve, _pkg_ids) =
        wit::load(wit_path, &opts.features, opts.all_features).map_err(GenError::Input)?;
    let project = map::generate(&resolve, opts).map_err(GenError::Diagnostics)?;
    let mut files: Vec<GeneratedFile> =
        render::render(&project, TOOL_VERSION, &opts.support_package)
            .into_iter()
            .map(|(path, content)| GeneratedFile { path, content })
            .collect();
    if !opts.no_support {
        for (path, content) in support::sources(&opts.support_package) {
            files.push(GeneratedFile { path, content });
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

/// `mapping-info`: the mapping table, from the same data the generator uses.
pub const MAPPING_TABLE: &str = include_str!("mapping-table.md");

/// Writes generated files under `out`, creating directories as needed.
pub fn write_output(files: &[GeneratedFile], out_dir: &std::path::Path) -> Result<(), GenError> {
    for f in files {
        let path = out_dir.join(&f.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                GenError::Internal(anyhow::anyhow!(
                    "failed to create {}: {e}",
                    parent.display()
                ))
            })?;
        }
        std::fs::write(&path, &f.content).map_err(|e| {
            GenError::Internal(anyhow::anyhow!("failed to write {}: {e}", path.display()))
        })?;
    }
    Ok(())
}
