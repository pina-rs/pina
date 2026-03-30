---
default: minor
pina_cli: minor
---

Add multi-file module resolution to the IDL parser.

`parse_program()` now follows `mod` declarations from `src/lib.rs` to discover and parse additional source files. This enables IDL generation for programs that split code across multiple modules (e.g. `src/state.rs`, `src/instructions/mod.rs`).

New module: `crates/pina_cli/src/parse/module_resolver.rs` with 5 unit tests covering single-file crates, child modules, `mod.rs` style, missing modules, and inline modules.

The existing `assemble_program_ir()` function is preserved for backward compatibility and now delegates to the new `assemble_program_ir_multi()`.
