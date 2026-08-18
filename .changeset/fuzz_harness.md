---
pina: feat
---

Add fuzz harness infrastructure for pina-rs targeting `PinaAccount::try_from_bytes` and `parse_instruction`.

- New `crates/pina_fuzz/` crate with `libfuzzer-sys` integration
- Fuzz targets for account deserialization of `CounterState`, `RegistryConfig`, and `RoleEntry`
- Fuzz targets for instruction parsing of `CounterInstruction` and `RegistryInstruction`
- Uses real workspace example programs (counter_program, role_registry_program) for authentic account/instruction types
- Compiles both standalone fuzz binaries during the normal CI test task so broken dependencies and entry points cannot silently merge
