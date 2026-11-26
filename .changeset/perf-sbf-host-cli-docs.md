---
pina: fix
pina_cli: fix
---

# Optimize SBF/host, CLI discovery, and docs delivery

- Add a dedicated `sbf_derive_ata` kernel in `pina` that coalesces `[bump || ATA_PROGRAM_ID]` into a single 33-byte stack slice for Agave's `sol_sha256` metering (`-108 CU` on legacy ATA, `-75 CU` on Token-2022 ATA), keep `bump` as `u64` in `sbf_derive_pda` (`-16 B` / `-2 CU`), inline legacy `AsTokenAccount` loaders (`-8` to `-10 CU`), avoid stack buffers on empty Token-2022 extension allow-lists, and precompute upgradeable loader `PROGRAM_DATA` PDAs at compile time in `multisig_program` and `privacy_pool_program` (`-4,880 B` total across 8 of 28 SBF ELFs, 0 regressions).
- Accelerate host PDA derivation with `Fe51::square`, branchless tiered `jacobi_is_non_negative_mod_p`, and zero-allocation SHA-256 prefix state cloning (`2.76x–3.07x` faster across all 11 host PDA benchmarks), and add compile-time inline pair checks in `run_static_alias_check` for compact `<= 4`-account layouts with `#[pina(unique)]` (`6.5x–9.1x` faster).
- Speed up `pina_cli` by eliminating the `cargo metadata` subprocess via direct manifest/workspace discovery, removing `rayon` thread-pool startup, keeping `RootNode` in memory across IDL/client rendering, avoiding unchanged atomic IDL rewrites, fast-pathing `--version`/`-V`/`docs`, and parsing workspace dependency keys with `IgnoredAny` (`17.9x` faster `pina generate`, `3.73x` faster `pina docs`, `-5 MB` release binary).
- Optimize `pina.rs` landing page and mdBook documentation builds with subsetted WOFF2 fonts, SVG logo replacement, build-time syntax highlighting, single hashed CSS/JS bundles, static SSR chrome, and compact on-demand search (`-69.5%` to `-77.5%` wire bytes).
