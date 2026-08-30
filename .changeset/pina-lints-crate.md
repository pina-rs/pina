---
pina_lints: feat
pina_cli: feat
---

# Ship the pina_lints crate and drop the Dylint pipeline

Introduce the `pina_lints` crate: every Pina security, performance, and IDL lint now lives in one self-contained, importable crate that mirrors the Dylint authoring API (`declare_late_lint!`, `declare_pre_expansion_lint!`) and registers through a single `register_all_lints` entry point. The crate builds as a library and a Dylint-compatible cdylib, and it ships the `pina_lint_driver` binary that statically links the whole catalog.

`pina lint` now runs `cargo check` (or `cargo fix` with `--fix`) with the bundled driver as `RUSTC_WRAPPER`. The driver is installed below Cargo home from the `pina_lints` release matching the CLI version on first use; no precompiled Dylint tools or lint bundles are downloaded. Lint levels are configurable through the new `[lints]` table of `pina.toml` (lint name = `allow`, `warn`, or `deny`), and unknown names are rejected with the full catalog list.
