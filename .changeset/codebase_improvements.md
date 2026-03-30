---
default: patch
pina: patch
pina_cli: patch
pina_codama_renderer: patch
---

Codebase quality improvements:

- Fix cu_benchmarks test crash by checking for SBF binary before loading mollusk
- Mark `typed_builder` re-export as `#[doc(hidden)]` non-stable API
- Add 11 tests for `pina_cli` error type Display impls
- Add `cargo doc` API docs check to `verify:docs` CI
- Rename `loaders.rs` → `impls.rs` for clarity
- Improve SAFETY documentation for all unsafe blocks in impls.rs
