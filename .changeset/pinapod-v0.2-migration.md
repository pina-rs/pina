---
pina: breaking
pina_macros: breaking
pina_cli: breaking
pina_codama_renderer: breaking
pina_cpi_renderer: breaking
pina_codama_nodes: breaking
pina_codama_renderer_cpi: breaking
pina_lints: feat
pina_skill: docs
---

# migrate Pina to the safe PinaPod 0.2 account API

Replace the ZeroPod-era account surface with PinaPod fixed and compact accounts while preserving existing on-chain bytes. Fixed accounts now support bounded strings, vectors, and options. Compact accounts support multiple tails, optional bounded values, and bounded vectors of fixed-footprint strings through checked patches and automatic resizing.

Account creation uses one-pass `invoke_with` initialization, stored-bump fixed PDAs gain `load_pda` and `load_pda_mut`, and resizable builders consistently use `rent_account` and `target_size`. Generated Rust, CPI, JavaScript, and Dart clients expose semantic string, vector, and option values at their boundaries.

The migration also strengthens validation around account borrows and trusted CPI builders, adds exact Mollusk instruction compute-unit comparisons alongside static SBF profiles, and documents the reviewed performance exceptions and complete downstream migration.
