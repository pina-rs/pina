---
pina: breaking
pina_abi: fix
pina_cli: fix
pina_codama_renderer: fix
pina_cpi_renderer: fix
pina_lints: fix
pina_macros: fix
pina_test: fix
---

# Upgrade pinapod to 0.3

Bump the workspace `pinapod` dependency from 0.2 to 0.3.1 and refresh the pina_fuzz lock files. Pina only re-exports and type-mentions the pinapod API, so no pina source changes were required, but the upgrade is breaking for downstream code that matches on `pina::PinaPodError` exhaustively: the error is now `#[non_exhaustive]` and requires a wildcard arm. `PodOption::raw_tag` returns `u64` so eight-byte tags cannot truncate, `PodOption` accepts eight-byte prefixes, and generated compact readers cache tail offsets for constant-time accessors. The wire format is unchanged.
