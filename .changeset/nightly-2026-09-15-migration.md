---
pina_lints: fix
pina_abi: fix
pina_macros: fix
pina_cli: fix
pina_cli_renderer: fix
pina_cpi_renderer: fix
---

# Build against nightly-2026-09-15

The `nightly-2026-09-15` toolchain bump moved the compiler APIs `pina_lints` links against, and this release follows them. `rustc_session::declare_lint!`, `declare_lint_pass!`, and `impl_lint_pass!` moved to `rustc_lint`; `LintStore::register_late_pass` and `register_pre_expansion_pass` became `register_late_lint_pass` and `register_pre_expansion_lint_pass` and now take boxed pass factories; `LangItem` moved to `rustc_hir::attrs::lang_items`; `TyCtxt::lint_level_at_node` became `lint_level_spec_at_node`; `type_of(..).instantiate_identity()` returns an `Unnormalized` wrapper that `skip_norm_wip` unwraps; `env_depinfo` moved from `ParseSess` to `Session`; and rustc deleted the unstable `--env-set` flag the lint driver used, which never wrote dep-info entries anyway — the driver's own `Session::env_depinfo` writes are what record `PINA_LINT_*` variables, and the emitted dep-info still carries them. Emission went through the compiler's `DiagDecorator` behind one `pina_lints::diagnostics::emit` helper, and every UI snapshot is byte-for-byte identical, so no diagnostic text changed.

`#[pda(seeds = [...])]` no longer expands an `Address` or fixed-bytes seed into a repeated field name (`authority: authority`), which the new clippy flags in every downstream crate that declares one; the generated initializer uses the field shorthand, and converted seeds (`to_le_bytes`, fixed arrays) are unchanged. The expansion snapshots were re-blessed for the same change plus the toolchain's new `Eq` derive and `assert_eq!` expansion shapes.

New clippy lints the bump also surfaced are fixed in place: `chunks_exact(2)` with a constant width becomes `as_chunks::<2>()` in `pina_abi`, `pina_cli`, and `pina_cpi_renderer`; the six `is_link_like`-style path helpers in `pina_cli` drop their cfg-blind `if` in favor of per-platform expressions so the Windows reparse-point check is explicit instead of deleted by clippy's suggestion; one codegen lookup uses `?`; and redundant `use pina::*;` lines were removed from example entrypoint modules that already glob-imported the crate root. The `redundant_field_names` warnings in `pina_macros` come from darling 0.24's own generated literal and are allowed at module scope, because no field- or struct-level attribute reaches that expansion.
