---
pina: docs
pina_cli: docs
pina_codama_renderer: docs
pina_lints: docs
pina_macros: docs
pina_profile: docs
pina_sdk_ids: docs
pina_test: docs
pina_codama_nodes: docs
pina_cli_npm: docs
pina_skill: docs
---

# Improve readme quality with managed badge rows

Replace the per-crate inline reference-link badges with an mdt-managed `crateReadmeBadgeRow` template and a new `npmReadmeBadgeRow` for the npm packages, so badges stay synchronized from `templates/readme-badges.t.md`. Each readme now carries concrete usage examples: a full counter program flow for `pina`, typed ID checks for `pina_sdk_ids`, a complete `ProgramTest` fixture for `pina_test`, CLI loops for `@pina-rs/cli`, and fresh usage claims in `pina_codama_renderer` (now published).
