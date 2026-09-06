//! Real-world corpus verification (spec §14.2, phase D9): generate all
//! vendored WASI packages, then verify with `javac --release 17
//! -Xlint:all -Werror` and `javadoc -Xdoclint:all,-missing` (zero warnings).
//!
//! The WJ0001 negative path for WIT 0.3 async constructs is covered by the
//! conformance cases; real WASI 0.3 does not use the `future`/`stream`
//! builtins (streams remain resources).

mod support;

use std::path::PathBuf;
use std::process::Command;
use support::*;
use wit_java_core::GenerateOptions;

fn corpus_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(workspace_root().join("tests/corpus"))
        .expect("corpus dir")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs
}

#[test]
fn wasi_corpus_compiles_and_documents_clean() {
    let Some(javac) = java17() else {
        eprintln!("SKIP: no JDK 17 available (set WIT_JAVA17_HOME)");
        return;
    };
    let javadoc = javac
        .parent()
        .map(|p| p.join("javadoc"))
        .expect("javadoc next to javac");

    // One merged tree: every corpus with --no-support + a single support copy.
    let merged = temp_dir("corpus-merged");
    for dir in corpus_dirs() {
        let opts = GenerateOptions {
            no_support: true,
            ..Default::default()
        };
        let files = wit_java_core::generate(&dir, &opts)
            .unwrap_or_else(|e| panic!("generate {}: {e}", dir.display()));
        write(&files, &merged);
    }
    let support_files =
        wit_java_core::generate(&corpus_dirs()[0], &GenerateOptions::default()).expect("support");
    let support_only: Vec<_> = support_files
        .into_iter()
        .filter(|f| f.path.starts_with("io/github/witjava/support"))
        .collect();
    write(&support_only, &merged);

    let java_files = list_java_files(&merged);
    assert!(
        java_files.len() > 150,
        "expected a large merged tree, got {}",
        java_files.len()
    );

    // javac --release 17 -Xlint:all -Werror
    let out = Command::new(&javac)
        .args(["--release", "17", "-Xlint:all", "-Werror"])
        .arg("-d")
        .arg(temp_dir("corpus-classes"))
        .args(&java_files)
        .output()
        .expect("run javac");
    assert!(
        out.status.success(),
        "javac failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // javadoc -Xdoclint:all,-missing, zero warnings (R5: -Werror promotes any
    // doclint warning to a failure)
    let out = Command::new(&javadoc)
        .args(["-quiet", "-Xdoclint:all,-missing", "-Werror"])
        .arg("-d")
        .arg(temp_dir("corpus-docs"))
        .args(&java_files)
        .output()
        .expect("run javadoc");
    assert!(
        out.status.success(),
        "javadoc failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn list_java_files(root: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn walk(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).expect("read dir").flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "java") {
                out.push(p);
            }
        }
    }
    walk(root, &mut out);
    out.sort();
    out
}
