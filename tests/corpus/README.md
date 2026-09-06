# Vendored WIT corpus (phase D9)

Real-world WIT packages used as test corpus, pinned to upstream tags/commits:

- `wasi:cli@0.2.8` — basics, world/role
- `wasi:cli@0.2.7` — extra regression snapshot
- `wasi:http@0.2.8` — resource-heavy, feature gates
- `wasi:filesystem@0.2.8` — flags-heavy
- `wasi:sockets@0.2.8` — tuple + resource heavy
- `wasi:random@0.2.8` — small package smoke

Upstream repositories, tags and pinned commits are recorded in `VERSIONS.md`
next to this file. The WJ0001 negative path (WIT 0.3 async constructs) is
covered by the synthetic conformance cases (`unsupported-future`,
`unsupported-map`); see `VERSIONS.md` for why no 0.3 directory is vendored.
