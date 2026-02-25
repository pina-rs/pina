---
pina: docs
pina_pod_primitives: minor
---

Added `PodBool::is_canonical()` method to detect non-canonical boolean values (2–255) that pass `bytemuck` deserialization but fail `PartialEq` comparison against canonical `PodBool(0)` or `PodBool(1)`. Programs should call `is_canonical()` at deserialization boundaries to validate account data integrity.

Added badges (crates.io, docs.rs, CI, license, codecov) to `pina_pod_primitives` readme and root workspace readme. Created readme for `pina_codama_renderer` crate.

Added 50+ new tests across pina and pina_pod_primitives covering:

- `parse_instruction` (valid/invalid discriminators, wrong program ID, empty data, error remapping)
- `PinaProgramError` error codes (correct discriminants, reserved range, uniqueness)
- `assert` function (true/false conditions, custom error types)
- PDA functions (determinism, seed variations, roundtrip, wrong bump)
- Pod types (boundary values, endianness, bytemuck deserialization, defaults)
- PodBool canonical validation (non-canonical equality mismatch detection)
- AccountDeserialize trait (field preservation, mutable modification, wrong offset)
- Discriminator write/read roundtrips for all primitive sizes
- Lamport helper edge cases (exact balance, zero transfer, max values)

Updated book chapters to use mdt shared blocks for codama workflow commands, release workflow commands, and feature flags table. Added three new mdt providers (`codamaWorkflowCommands`, `releaseWorkflowCommands`, `pinaFeatureFlags`) to `template.t.md`.
