---
pina_cli: fix
pina_codama_renderer_cli: fix
---

# Add the multisig example and fix generators

`examples/multisig_program` is a production-shaped multisig built on pina: bitmask voting over a sorted compact roster, unified proposal accounts carrying compiled vault messages or config action streams, timelocked execution with per-proposal ephemeral signers, vote revocation, proposal expiry, spending limits, rent collection, a generic Anchor-layout legacy import, and the full migration envelope. Building it surfaced two generator defects that this changeset also ships.

Freshly scaffolded Rust CLI clients hard-coded `../../rust/<example>` as the dependency path, but the crate lives three directories deeper, so the manifest never resolved; existing examples only survived because update mode preserves their committed manifests, meaning no new example could scaffold a working CLI. The path is now derived from the configured rust and CLI output roots. The Dart CLI renderer emitted boolean instruction arguments as mandatory string options cast to `as String`, which failed `dart analyze` with a String-to-bool error; booleans are now declared as flags and read as `bool`, defaulting to false. The `pina docs` snapshot scratch space also moved under the workspace `target/` directory so recorded argument paths keep the same relative form on every machine and CI runner instead of leaking environment-specific absolute temp paths.
