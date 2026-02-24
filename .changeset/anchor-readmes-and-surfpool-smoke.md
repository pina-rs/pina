---
pina: note
---

Expand Anchor parity documentation and add Surfpool-based IDL smoke coverage.

- Add dedicated `readme.md` files for each `examples/anchor_*` crate documenting intent and key differences from Anchor.
- Update each Anchor example crate manifest to point its `readme` field at the local example README.
- Strengthen IDL verification checks to assert discriminator metadata is present for generated anchor instructions/accounts.
- Add a Surfpool smoke test script that patches a test program ID, generates IDL, deploys the compiled program to Surfpool, and invokes it using generated IDL discriminator metadata.
- Add a dedicated `surfpool` GitHub Actions workflow for these longer-running deployment/invocation checks.
- Update pinned Surfpool binary from `v0.12.0` to `v1.0.1` in `.eget/.eget.toml`.
- Update pinned Agave release from `v3.0.12` to `v3.1.8` so `cargo-build-sbf` can build workspace edition-2024 programs for Surfpool smoke tests.
