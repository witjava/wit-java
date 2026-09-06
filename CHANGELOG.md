# Changelog

## [Unreleased]

### Fixed

- `--option-style=nullable` output is now compilable: every file using
  `@Nullable` imports the support annotation. Previously the annotation was
  emitted with no import, so `javac` failed with `cannot find symbol` — the
  Tier-1 digest cannot see imports and no compile test covered the style.
  A corpus regression test now compiles the nullable and `--u64=BigInteger`
  styles with `javac --release 17 -Xlint:all -Werror` (spec §5.4, §5.1).
- An empty WIT interface (no types and no functions, or everything filtered
  out by disabled feature gates) now emits its declaration file and
  `package-info.java` (spec §2, §6). Previously it was silently dropped
  while world accessors still imported it, producing uncompilable output.
  New conformance case `empty-interface` pins both the world-referenced and
  the unreferenced interface.
- Internal-invariant failures when resolving an interface FQN now raise a
  diagnostic instead of silently skipping the world aggregates (a silent
  skip would report success with missing output).
- World-local type definitions (inline world interfaces) are now emitted into
  the world package segment and their functions fold into the aggregates;
  previously the entire inline interface was silently dropped (spec §7.2).
- An interface whose name is repeated by one of its member types (real WASI:
  `interface error { resource error; }`, `network`, `terminal-input`,
  `terminal-output`) no longer silently overwrites the interface declaration
  file: the declaration mangles to `Error_` per spec §4.3 while the member
  type keeps its WIT-derived name.
- Java keywords in WIT package and world names (`class`, `default`, …) are
  mangled in Java package segments (spec §3.1); previously they produced
  uncompilable package declarations.
- `--package-map` keys with partial versions (`wasi:io@0.2`) now match
  packages with the same major.minor, as in the spec's examples (§3.4).
- `--u64=BigInteger` no longer emits the `long`-specific unsigned Javadoc
  note (spec §5.1).
- A WIT-defined resource member named `close` mangles to `close_` instead of
  producing a duplicate declaration.
- Javadoc containing `*/` no longer terminates the generated comment early.
- Usage errors (invalid `--package-root` / `--support-package` /
  `--package-map`, unknown `--mapping-version`) print a plain `error:` line
  instead of misusing the frozen `WJnnnn` diagnostic codes.
- Nested owned returns carry the spec §5.11 "contains owned handles" note;
  direct `own<T>` returns keep the plain owned-handle note.

### Changed

- FQN-collision diagnostics (WJ0004/WJ0007) name the kind of the
  already-claimed declaration (`resource`, `record`, …).
- Generated Javadoc carries only real content: WIT docs, ownership notes and
  the unsigned/char notes. Invented filler (`@param x x`, `@return name`,
  `Nothing to see here.`) is gone; undocumented members get no tag.
- Accessor Javadoc in `Exports` aggregates says "exported" interface.
- CLI: `generate --check` removed (identical to the `check` subcommand);
  `--u64` accepts the spec spelling `BigInteger` (plus `big-integer`).
- Corpus: `wasi-cli-0.3-rc` renamed to `wasi-cli-0.2.7` — the vendored tree
  is byte-identical to the upstream `v0.2.7` tag, not a 0.3 preview.
- Conformance suite grown to 18 positive + 7 negative cases: cases pin
  inline world interfaces, interface/member name collisions, reserved-word
  package segments, empty interfaces, feature gates (`--features` enabled /
  default-skipped) and single-role generation (`--role guest` keeps the role
  segment). Golden trees re-blessed.
- javadoc verification now runs with `-Werror` so any doclint warning fails
  the build.

### Internal

- Comment references that cited DESIGN.md section numbers as "spec §" now
  name their actual source; boxing cites spec §5.1.1.
- Removed unused dependencies (`thiserror`, `camino`).
- Duplicate output paths are now an internal error instead of a silent
  overwrite; packages sort by semver precedence instead of version string.
- package-info without description no longer renders an empty `*` line.
