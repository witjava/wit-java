# wit-java

Reference implementation of the
[`wit-java-mapping`](https://github.com/witjava/wit-java-mapping) spec:
a WIT → Java 17 declaration generator.

**Declaration-only.** wit-java generates Java type declarations from WIT.
It does NOT generate canonical-ABI lifting/lowering, memory layout, handle
tables, or host/guest glue — a component runtime adapter (Chicory / Endive /
wasmtime-java) implements that layer against the generated declarations.

- **Zero runtime dependencies**: output compiles with `javac --release 17`
  (`-Xlint:all -Werror` clean), no third-party JARs.
- **Deterministic**: same input → byte-identical output on every platform.
- **Spec-first**: the mapping lives in the separate `wit-java-mapping`
  repository; this crate is its reference implementation, not its definition.

Status: pre-1.0. Implements the `wit-java-mapping` v1 draft end to end:
the conformance suite (15 positive + 7 negative cases), byte-level golden
tests, and the vendored WASI corpus (0.2.8 + a 0.2.7 snapshot) all pass, with
`javac --release 17 -Xlint:all -Werror` and `javadoc -Xdoclint:all,-missing
-Werror` clean. See `TODO.md` for what remains before the 1.0 freeze.

License: Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT
(see `LICENSE-APACHE` / `LICENSE-MIT`). Generated files carry no license
header.
