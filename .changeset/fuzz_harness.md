---
type: note
---

Add fuzz harness infrastructure for pina-rs targeting `AccountDeserialize::try_from_bytes` and `parse_instruction`.

- New `crates/pina_fuzz/` crate with `libfuzzer-sys` integration
- Fuzz targets for account deserialization of `CounterState`, `RegistryConfig`, and `RoleEntry`
- Fuzz targets for instruction parsing of `CounterInstruction` and `RegistryInstruction`
- Uses real workspace example programs (counter_program, role_registry_program) for authentic account/instruction types
