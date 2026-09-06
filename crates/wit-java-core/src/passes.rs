//! Import resolve pass (spec §8/§12: same simple name in one file → import
//! the lexicographically-first FQN, fully qualify the rest).

use crate::ir::{Decl, JavaFile, Method, TypeRef};
use crate::naming::Fqn;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub struct ImportPlan {
    /// FQNs (outermost simple names) that may be imported.
    pub imports: BTreeSet<String>,
    /// Full `pkg.Name` strings that must be rendered qualified.
    pub must_qualify: BTreeSet<String>,
}

/// Computes the import plan for one file.
pub fn resolve_imports(file: &JavaFile) -> ImportPlan {
    let mut fqns: BTreeMap<&str, BTreeSet<&Fqn>> = BTreeMap::new();
    collect_file_fqns(file, &mut |fqn: &Fqn| {
        if fqn.is_java() || fqn.package == file.package {
            return; // java.lang / same package: never imported, always simple
        }
        fqns.entry(fqn.outer_simple()).or_default().insert(fqn);
    });

    let mut plan = ImportPlan::default();
    for (_simple, set) in fqns {
        let mut sorted: Vec<&Fqn> = set.into_iter().collect();
        sorted.sort();
        let (first, rest) = sorted.split_first().expect("non-empty");
        plan.imports
            .insert(format!("{}.{}", first.package, first.outer_simple()));
        for other in rest {
            plan.must_qualify.insert(other.to_string());
        }
    }
    plan
}

fn collect_file_fqns<'a>(file: &'a JavaFile, f: &mut dyn FnMut(&'a Fqn)) {
    for decl in &file.decls {
        collect_decl(decl, f);
    }
}

fn collect_decl<'a>(decl: &'a Decl, f: &mut dyn FnMut(&'a Fqn)) {
    match decl {
        Decl::Record {
            components,
            consts,
            methods,
            ..
        } => {
            for c in components {
                visit(&c.ty, f);
            }
            for c in consts {
                visit(&c.ty, f);
            }
            for m in methods {
                visit_method(m, f);
            }
        }
        Decl::Enum { .. } => {}
        Decl::Interface {
            extends,
            methods,
            nested,
            ..
        } => {
            for t in extends {
                visit(t, f);
            }
            for m in methods {
                visit_method(m, f);
            }
            for n in nested {
                collect_decl(n, f);
            }
        }
    }
}

fn visit_method<'a>(m: &'a Method, f: &mut dyn FnMut(&'a Fqn)) {
    for p in &m.params {
        visit(&p.ty, f);
    }
    visit(&m.ret, f);
}

fn visit<'a>(ty: &'a TypeRef, f: &mut dyn FnMut(&'a Fqn)) {
    let mut out = Vec::new();
    ty.visit_fqns(&mut out);
    for fqn in out {
        f(fqn);
    }
}
