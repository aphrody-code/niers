# Reverse-engineering center

The canonical local reverse-engineering workspace is [`data/re`](../../data/re/00-index/README.md).

`docs/re` intentionally contains only this lightweight entry point. Binaries,
VFS inventories, Ghidra projects, dumps, derived data, IECODE snapshots, and
machine-generated manifests live together under `data/re` so documentation
cannot drift into a second archive.

The local-to-canonical consumer and parity decision is recorded in
[`PARITY-AUDIT-2026-09-07.md`](PARITY-AUDIT-2026-09-07.md). It complements
[`docs/COMPUTER-USE-RE-TRACE.md`](../COMPUTER-USE-RE-TRACE.md) for the `nie-re`/`nie-trace` bridge.
