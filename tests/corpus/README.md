# Vendored WIT corpus (phase D9)

Real-world WIT packages used as test corpus, pinned to upstream commits:

- `wasi:cli@0.2.x` — basics, world/role
- `wasi:http@0.2.x` — resource-heavy, feature gates
- `wasi:filesystem@0.2.x` — flags-heavy
- `wasi:sockets@0.2.x` — tuple + resource heavy
- one 0.3 async world — negative path for WJ0001

Populated in phase D9. Upstream repositories, tags and pinned commits are
recorded in `VERSIONS.md` next to this file. The 0.3 async negative path is
covered by the synthetic conformance cases (`unsupported-future`,
`unsupported-map`); real WASI 0.3 keeps streams as resources and generates
cleanly (`wasi-cli-0.3-rc`).
