---
default: patch
pina_codama_renderer: patch
---

Reduce warning noise and restore local Codama client verification ergonomics.

- Annotate the generated Rust root `mod.rs` re-export of `programs::*` with `#[allow(unused_imports)]`.
- Add regression coverage for the generated root module allowance.
- Update the repository Codama JS test harness to type-check generated clients against the current `@solana/kit` dependency layout using a local compatibility shim.

This keeps crate-internal program ID constants available at `crate::<PROGRAM>_ID` for generated instruction modules, while avoiding warnings for IDLs that only generate a `programs` module and keeping `pnpm run check:js` green.
