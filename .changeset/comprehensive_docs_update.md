---
default: patch
pina: patch
pina_cli: patch
pina_profile: patch
---

Comprehensive documentation update across workspace.

New mdt providers (template.t.md):

- `pinaCliCommands` — CLI command reference table
- `pinaIntrospectionDescription` — introspection module overview
- `pinaProfileDescription` — static CU profiler overview

Updated documentation:

- `docs/src/crates-and-features.md` — added `pina_profile`, CLI commands table, multi-file parser note, pod arithmetic, codama renderer module structure
- `docs/src/core-concepts.md` — added Pod types table, arithmetic description, introspection section; fixed stale `loaders.rs` → `impls.rs` reference
- `readme.md` — added Pod arithmetic examples, static CU profiler section, replaced outdated 3-crate table with full workspace packages table
- `crates/pina_cli/readme.md` — added `pina profile` command, multi-file note
- Fixed missing `CU_PER_INSTRUCTION` import in profiler tests

mdt provider/consumer counts: 23/46 → 26/56.
