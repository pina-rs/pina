---
default: patch
pina_codama_renderer: patch
---

Refactor `pina_codama_renderer`: split monolithic `lib.rs` into focused render modules.

- `render/helpers.rs` — string utilities, docs rendering, numeric casts
- `render/discriminator.rs` — discriminator type/value resolution
- `render/types.rs` — POD type rendering and defined-type pages
- `render/accounts.rs` — account struct, PDA helpers, accounts mod
- `render/instructions.rs` — instruction struct, account metas, data struct
- `render/seeds.rs` — variable and constant PDA seed expression rendering
- `render/errors.rs` — error enum pages and errors mod
- `render/scaffold.rs` — crate scaffold creation and file writing
- `render/mods.rs` — root mod and programs mod rendering

`lib.rs` retains only the public API (`RenderConfig`, `read_root_node`,
`render_idl_file`, `render_root_node`, `render_program`) and the
orchestrator `render_program_to_files`.

All 13 existing tests continue to pass.
