---
pina_cli: none
---

# Adopt migrations across every example program

All 23 remaining examples now enable ABI migrations: each `pina.toml` carries `[migrations]` with `version_type = "u8"` and `auto = true`, and each program has the recorded version-0 manifest, publication ledger, generated `tests/abi_layout.rs` guard, and `build.rs` rerun directive. `examples/migrations_program` keeps its existing `u8` history unchanged.

Every example therefore ships the `[discriminator][schema version][payload]` envelope, and the regenerated IDLs and Rust/CPI/JavaScript/Dart clients carry the `migrationVersion` field that matches it. The examples double as the reference for what a default configuration produces.

Hand-written layout assertions in the affected examples were updated to the enveloped geometry: sizes grow by the version byte, and raw-byte tests gained the envelope byte or moved to the offsets `tests/abi_layout.rs` records. Where an assertion changed meaning rather than just a number, the comment now names the envelope so the shift is explained instead of restated.

Two defects in the tooling surfaced while converting the examples and are fixed here:

- **`pina migrations make` and `fix:format` fought over `tests/abi_layout.rs`.** The generator emits long `SCHEMA_SHA256` constants on one line and expands every `FIELDS` array, while `rustfmt` re-wraps the former and collapses the latter. Since `fix:format` runs on every checkout, a formatted guard file was reported as stale by `pina migrations check`, for any migration-aware project — including on `main`. The generated guard is now excluded from formatting, alongside the other generated artifacts (`**/snapshots`, `tests/expand/*.expanded.rs`) that the repository already excludes, and `check` accepts either spelling.
- **The scaffolded `build.rs` failed `-D warnings`.** Neither `pina init` nor `pina migrations make` emitted crate-level documentation, so any workspace that lints with `-D warnings` failed to compile the build script. Both scaffolds now write a `//!` doc comment.
