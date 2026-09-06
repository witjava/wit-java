//! Option semantics not covered by conformance cases (spec §2, §11):
//! `--world` restricts world generation, support-package collisions are
//! rejected. The conformance runner deliberately excludes `--world` from
//! case options (conformance/README.md), so the semantics are pinned here
//! instead.

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

/// A `--world` naming no world in the input would otherwise silently
/// generate nothing but the world-less interfaces (spec §2: usage error).
#[test]
fn unknown_world_name_is_a_usage_error() {
    let src = two_world_wit();

    let opts = GenerateOptions {
        worlds: vec!["nonexistent".to_string()],
        ..Default::default()
    };
    match wit_java_core::generate(&src, &opts) {
        Err(wit_java_core::GenError::Input(e)) => {
            let msg = format!("{e:#}");
            assert!(
                msg.contains("nonexistent"),
                "names the unknown world: {msg}"
            );
            assert!(msg.contains("other"), "lists the known worlds: {msg}");
        }
        Err(other) => panic!("expected Input error, got {other:?}"),
        Ok(files) => panic!("expected error, got {} files", files.len()),
    }

    // Partial match is still an error: the typo must not pass silently.
    let opts = GenerateOptions {
        worlds: vec!["app".to_string(), "typo".to_string()],
        ..Default::default()
    };
    assert!(
        wit_java_core::generate(&src, &opts).is_err(),
        "one unknown name among valid ones must still fail"
    );
}

/// Generated files landing in the support package itself (flat style +
/// `--package-map` onto the support FQN) must be rejected, not silently
/// overwrite a support source — the shared `package-info.java` is the usual
/// victim (spec §8). Sub-packages of the support package stay legal.
#[test]
fn support_package_collision_is_rejected() {
    let src = two_world_wit();

    let mut package_map = std::collections::BTreeMap::new();
    package_map.insert(
        "demo:sel".to_string(),
        wit_java_core::DEFAULT_SUPPORT_PACKAGE.to_string(),
    );

    // Flat style puts the interface declaration directly into the support
    // package → its package-info.java clashes with the support one.
    let opts = GenerateOptions {
        package_map: package_map.clone(),
        interface_style: wit_java_core::InterfaceStyle::Flat,
        ..Default::default()
    };
    match wit_java_core::generate(&src, &opts) {
        Err(wit_java_core::GenError::Input(e)) => {
            let msg = format!("{e:#}");
            assert!(msg.contains("collides"), "collision message: {msg}");
        }
        Err(other) => panic!("expected Input error, got {other:?}"),
        Ok(files) => panic!("expected error, got {} files", files.len()),
    }

    // Nested style only populates sub-packages of the support package —
    // no shared path, so this combination stays legal.
    let opts = GenerateOptions {
        package_map,
        ..Default::default()
    };
    assert!(
        wit_java_core::generate(&src, &opts).is_ok(),
        "sub-packages of the support package must not be rejected"
    );
}
