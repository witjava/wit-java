//! Byte-level golden tests (Tier 2, spec §14): constrain the reference
//! implementation only. `BLESS=1 cargo test` regenerates the golden trees.

mod support;

use std::path::{Path, PathBuf};
use support::*;

fn golden_dir(name: &str) -> PathBuf {
    workspace_root().join("tests/golden").join(name)
}

fn should_bless() -> bool {
    std::env::var("BLESS").is_ok_and(|v| !v.is_empty() && v != "0")
}

fn collect(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    fn walk(dir: &Path, prefix: &str, out: &mut Vec<(String, Vec<u8>)>) {
        for e in std::fs::read_dir(dir).expect("read dir").flatten() {
            let p = e.path();
            let rel = if prefix.is_empty() {
                e.file_name().to_string_lossy().to_string()
            } else {
                format!("{prefix}/{}", e.file_name().to_string_lossy())
            };
            if p.is_dir() {
                walk(&p, &rel, out);
            } else {
                out.push((rel, std::fs::read(p).expect("read file")));
            }
        }
    }
    walk(dir, "", &mut out);
    out.sort();
    out
}

#[test]
fn golden_trees_match_byte_for_byte() {
    let bless = should_bless();
    let mut failures = Vec::new();
    for name in all_case_names().iter().filter(|n| is_positive(n)) {
        let case_dir = cases_dir().join(name);
        let opts = options_from(&case_dir, &case_options(&case_dir));
        let files = match wit_java_core::generate(&case_source(&case_dir), &opts) {
            Ok(f) => f,
            Err(e) => {
                failures.push(format!("{name}: generate failed: {e}"));
                continue;
            }
        };
        let golden = golden_dir(name);
        if bless {
            let _ = std::fs::remove_dir_all(&golden);
            let tmp = temp_dir(&format!("bless-{name}"));
            write(&files, &tmp);
            std::fs::create_dir_all(golden.parent().unwrap()).unwrap();
            std::fs::rename(&tmp, &golden).expect("move blessed tree");
            println!("blessed {name}");
            continue;
        }
        if !golden.exists() {
            failures.push(format!(
                "{name}: golden tree missing (run BLESS=1 cargo test)"
            ));
            continue;
        }
        let expected = collect(&golden);
        let actual: Vec<(String, Vec<u8>)> = files
            .iter()
            .map(|f| (f.path.clone(), f.content.clone().into_bytes()))
            .collect();
        if expected != actual {
            let expected_names: Vec<&str> = expected.iter().map(|(p, _)| p.as_str()).collect();
            let actual_names: Vec<&str> = actual.iter().map(|(p, _)| p.as_str()).collect();
            failures.push(format!(
                "{name}: golden mismatch\n  files golden={:?}\n  files actual={:?}",
                expected_names, actual_names
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "golden failures:\n{}",
        failures.join("\n")
    );
}
