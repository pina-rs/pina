---
pina: patch
---

Harden no-logs builds and release workflow compatibility.

- Gate `core::panic::Location` behind the `logs` feature and explicitly mark assertion messages as used in non-logs builds so `pina` compiles cleanly in no-logs paths (including Surfpool smoke builds).
- Move `ignore_conventional_commits` from `PrepareRelease` to the `[changes]` section in `knope.toml` to match current `knope` configuration expectations.
