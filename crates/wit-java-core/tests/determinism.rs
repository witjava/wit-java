//! Determinism (spec §12): same input + options → byte-identical output.

mod support;

#[test]
fn generated_output_is_deterministic() {
    for name in support::all_case_names()
        .into_iter()
        .filter(|n| support::is_positive(n))
    {
        let case_dir = support::cases_dir().join(&name);
        let opts = support::options_from(&case_dir, &support::case_options(&case_dir));
        let a = wit_java_core::generate(&support::case_source(&case_dir), &opts)
            .expect("first generation");
        let b = wit_java_core::generate(&support::case_source(&case_dir), &opts)
            .expect("second generation");
        assert_eq!(a, b, "{name}: two runs differ");
    }
}

/// Real-world corpus: generate twice, byte-compare (spec §14 determinism,
/// cross-platform part lives in CI).
#[test]
fn corpus_output_is_deterministic() {
    let corpus = support::workspace_root().join("tests/corpus");
    for entry in std::fs::read_dir(&corpus).expect("corpus dir").flatten() {
        if !entry.path().is_dir() || entry.file_name() == "VERSIONS.md" {
            continue;
        }
        let opts = wit_java_core::GenerateOptions::default();
        let a = wit_java_core::generate(&entry.path(), &opts).expect("first");
        let b = wit_java_core::generate(&entry.path(), &opts).expect("second");
        assert_eq!(
            a,
            b,
            "{}: two runs differ",
            entry.file_name().to_string_lossy()
        );
    }
}
