---
pina_cli: fix
---

Update crypto dependencies.

- `base64` 0.22 → 0.23 (SIMD engine additions; measured binary-size-neutral, the CLI uses the scalar engine so SIMD paths are dead-stripped).
- `ed25519-dalek` 2.2 → 3.0 (curve25519-dalek 5, MSRV 1.85; required no code changes).
- `sha2` 0.10 → 0.11 (digest 0.11, `std` feature replaced by `alloc`); digest arrays no longer implement `LowerHex`, so the eleven hash-formatting sites in `pina_cli` (library and tests) now map bytes explicitly with identical output.

CLI release binary measured −592 B (−0.007%, noise); SBF program artifact byte-identical — `sha2 0.10.9` stays pinned in the program graph by `solana-address 2.6.1` and is unaffected by this change.

This repeats the revert of #255 with the test-target compilation fixed, so all CLI portability checks pass.
