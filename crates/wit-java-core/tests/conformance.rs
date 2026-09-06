//! Conformance runner (spec §14, phase D9): the reference implementation
//! consumes the vendored mapping-repository conformance data.

mod support;

use std::process::Command;
use support::*;
use wit_java_core::GenError;

#[test]
fn conformance_positive_cases_match_tier1_digest() {
    let Some(java) = java_path() else {
        eprintln!("SKIP: no java available for the Tier-1 checker");
        return;
    };
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
        let out = temp_dir(&format!("conf-{name}"));
        write(&files, &out);
        let output = Command::new(&java)
            .arg(checker_path())
            .arg("--compare")
            .arg(case_dir.join("expected"))
            .arg(&out)
            .output()
            .expect("run checker");
        if !output.status.success() {
            failures.push(format!(
                "{name}: digest mismatch\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let _ = std::fs::remove_dir_all(&out);
    }
    assert!(
        failures.is_empty(),
        "conformance failures:\n{}",
        failures.join("\n")
    );
}

#[test]
fn conformance_negative_cases_report_codes() {
    for name in all_case_names().iter().filter(|n| !is_positive(n)) {
        let case_dir = cases_dir().join(name);
        let expected = expected_error_code(&case_dir);
        let opts = options_from(&case_dir, &case_options(&case_dir));
        match wit_java_core::generate(&case_source(&case_dir), &opts) {
            Err(GenError::Diagnostics(diags)) => {
                let reported: Vec<String> = diags
                    .0
                    .iter()
                    .map(|d| d.code.as_str().to_string())
                    .collect();
                assert!(
                    reported.contains(&expected),
                    "{name}: expected {expected}, got {reported:?}"
                );
            }
            Err(other) => panic!("{name}: expected diagnostics, got {other:?}"),
            Ok(_) => panic!("{name}: expected {expected}, but generation succeeded"),
        }
    }
}

#[test]
fn checker_selftest_passes() {
    let Some(java) = java_path() else {
        eprintln!("SKIP: no java available");
        return;
    };
    let status = Command::new(java)
        .arg(checker_path())
        .arg("--selftest")
        .status()
        .expect("run checker");
    assert!(status.success(), "checker --selftest failed");
}
