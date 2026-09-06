//! Thin `wit-parser` adapter: loading, feature gates and source locations.
//! This module is the SOLE upstream coupling point — when upgrading
//! `wit-parser`, review this file first.

use crate::diagnostic::Location;
use wit_parser::{Resolve, UnresolvedPackageGroup};

/// Loads a WIT file or directory with feature gates applied (spec §2).
/// Feature selection must happen before resolution: `Resolve.features` and
/// `Resolve.all_features` are consulted while items are filtered.
pub fn load(
    path: &std::path::Path,
    features: &[String],
    all_features: bool,
) -> anyhow::Result<(Resolve, Vec<wit_parser::PackageId>)> {
    let mut resolve = Resolve::new();
    for f in features {
        resolve.features.insert(f.clone());
    }
    resolve.all_features = all_features;

    let pkg_id = if path.is_dir() {
        let (id, _sources) = resolve.push_dir(path)?;
        id
    } else {
        let contents = std::fs::read_to_string(path)?;
        let group = UnresolvedPackageGroup::parse(path, &contents)
            .map_err(|(source_map, err)| anyhow::anyhow!("{}", err.render(&source_map)))?;
        resolve.push_group(group)?
    };
    Ok((resolve, vec![pkg_id]))
}

/// All packages currently in the resolve, deterministically ordered
/// (spec §12: never arena/insertion order). Versions compare by semver
/// precedence, not string order.
pub fn sorted_packages(resolve: &Resolve) -> Vec<wit_parser::PackageId> {
    let mut pkgs: Vec<_> = resolve.packages.iter().collect();
    pkgs.sort_by(|a, b| {
        let (an, bn) = (&a.1.name, &b.1.name);
        an.namespace
            .cmp(&bn.namespace)
            .then_with(|| an.name.cmp(&bn.name))
            .then_with(|| an.version.cmp(&bn.version))
    });
    pkgs.into_iter().map(|(id, _)| id).collect()
}

/// Renders a span to a `path:line:col` location string (spec §10).
pub fn locate(resolve: &Resolve, span: wit_parser::Span) -> Option<Location> {
    let rendered = resolve.source_map.render_location(span);
    if rendered.is_empty() {
        None
    } else {
        Some(Location(rendered))
    }
}
