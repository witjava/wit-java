//! Option semantics not covered by conformance cases (spec §2, §11):
//! `--world` restricts world generation. The conformance runner
//! deliberately excludes `--world` from case options (conformance/README.md),
//! so the semantics are pinned here instead.

mod support;

use support::temp_dir;
use wit_java_core::GenerateOptions;

fn two_world_wit() -> std::path::PathBuf {
    let dir = temp_dir("world-select");
    let src = dir.join("src.wit");
    std::fs::write(
        &src,
        r#"package demo:sel@0.1.0;

interface clock {
    now: func() -> u64;
}

world app {
    import clock;
}

world other {
    import clock;
}
"#,
    )
    .unwrap();
    src
}

#[test]
fn world_selection_restricts_generation() {
    let src = two_world_wit();

    // Default: all worlds in the package are generated (spec §2).
    let all = wit_java_core::generate(&src, &GenerateOptions::default()).unwrap();
    assert!(
        all.iter().any(|f| f.path.contains("/app/")),
        "default run must generate world `app`"
    );
    assert!(
        all.iter().any(|f| f.path.contains("/other/")),
        "default run must generate world `other`"
    );

    // --world other: only the named world's aggregates; interfaces stay
    // (they are generated regardless of world membership, spec §2).
    let opts = GenerateOptions {
        worlds: vec!["other".to_string()],
        ..Default::default()
    };
    let sel = wit_java_core::generate(&src, &opts).unwrap();
    assert!(
        !sel.iter().any(|f| f.path.contains("/app/")),
        "--world other must not generate world `app`"
    );
    assert!(
        sel.iter().any(|f| f.path.contains("/other/")),
        "--world other must generate world `other`"
    );
    assert!(
        sel.iter().any(|f| f.path.ends_with("clock/Clock.java")),
        "interfaces are generated regardless of world membership"
    );
}
