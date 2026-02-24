---
pina: note
---

Added new example coverage and upstream BPF tooling updates:

- Added `examples/pinocchio_bpf_starter` based on the upstream `pinocchio-bpf-starter` template pattern.
- Added sequential Anchor parity examples:
  - `examples/anchor_declare_id`
  - `examples/anchor_declare_program`
  - `examples/anchor_duplicate_mutable_accounts`
  - `examples/anchor_errors`
  - `examples/anchor_events`
  - `examples/anchor_floats`
  - `examples/anchor_system_accounts`
  - `examples/anchor_sysvars`
  - `examples/anchor_realloc`
- Extended `examples/escrow_program` with parity-focused tests aligned with Anchor's escrow coverage.
- Updated `sbpf-linker` in `[workspace.metadata.bin]` to `0.1.8`.
- Added a `build-bpf` cargo alias for the starter example and documented Anchor porting progress in the mdBook docs, including explicit notes for suites that are Anchor-CLI-specific.
- Added Codama IDL fixtures for all `anchor_*` example programs under `codama/idls/` and new Rust/JS IDL verification tests (`test:idl`) that run in CI.
