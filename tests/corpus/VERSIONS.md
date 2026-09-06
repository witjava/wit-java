# Vendored corpus provenance

Each directory is the `wit/` tree of the named upstream repository, pinned
per phase D9 (2026-09-06). Refresh = re-vendor + update this file.

| directory | upstream | ref | commit |
|---|---|---|---|
| wasi-cli | WebAssembly/wasi-cli | v0.2.8 | da4d2c006e2c17568a5c95d574f323e36bd19f68 |
| wasi-cli-0.2.7 | WebAssembly/wasi-cli | v0.2.7 | 07e4b66066ee7c183b10e8ee696e5aa571e9fbef (tag object) |
| wasi-http | WebAssembly/wasi-http | v0.2.8 | 14a19b388f4828604a08ede2af8ee1285c020113 |
| wasi-filesystem | WebAssembly/wasi-filesystem | v0.2.8 | 971b11617b50e7496bea85f36e60141bda172964 |
| wasi-sockets | WebAssembly/wasi-sockets | v0.2.8 | 85f0c064f5b9ea2faa3c65b1a80b870119c0fc7f |
| wasi-random | WebAssembly/wasi-random | v0.2.8 | 39796b821b61d5141d9f87f6e5bde8e67f73a47a |

`wasi-cli-0.2.7` is byte-identical to the upstream `v0.2.7` tag's `wit/` tree
(verified 2026-09-06 against the release tarball) and is kept as an extra
regression snapshot.

There is no 0.3 corpus: real WASI 0.3 keeps streams as resources, and its
remaining new constructs (`future` / `stream` / `error-context` builtins) are
covered by the synthetic negative conformance cases (`unsupported-future`,
`unsupported-map`).
