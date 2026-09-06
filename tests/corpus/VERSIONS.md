# Vendored corpus provenance

Each directory is the `wit/` tree of the named upstream repository, pinned
per phase D9 (2026-09-06). Refresh = re-vendor + update this file.

| directory | upstream | ref | commit |
|---|---|---|---|
| wasi-cli | WebAssembly/wasi-cli | v0.2.8 | da4d2c006e2c17568a5c95d574f323e36bd19f68 |
| wasi-http | WebAssembly/wasi-http | v0.2.8 | 14a19b388f4828604a08ede2af8ee1285c020113 |
| wasi-filesystem | WebAssembly/wasi-filesystem | v0.2.8 | 971b11617b50e7496bea85f36e60141bda172964 |
| wasi-sockets | WebAssembly/wasi-sockets | v0.2.8 | 85f0c064f5b9ea2faa3c65b1a80b870119c0fc7f |
| wasi-random | WebAssembly/wasi-random | v0.2.8 | 39796b821b61d5141d9f87f6e5bde8e67f73a47a |
| wasi-io-0.3-draft | WebAssembly/wasi-io | main (0.3 draft; no tag yet) | 3983fe1feab6b3a3b4e5c47c8b13daaf22266f00 |

`wasi-io-0.3-draft` is the negative corpus: its streams use WIT 0.3
constructs (`future` / `stream` / `error-context`), so generation must fail
with WJ0001.
