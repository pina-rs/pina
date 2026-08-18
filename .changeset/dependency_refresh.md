---
pina: fix
pina_cli: fix
pina_codama_renderer: fix
pina_macros: fix
pina_pod_primitives: fix
pina_profile: fix
pina_sdk_ids: fix
---

# Refresh workspace dependencies

Refresh workspace dependencies to their latest compatible versions.

- Bump `pinocchio-token` to `0.7` and `pinocchio-token-2022` to `0.4` (drops the `token_program` field from `TransferChecked`/`CloseAccount`; examples now use the `new()` constructors).
- Bump `codama-nodes` to `0.11` (spec `1.8.0`), `mollusk-svm` to `0.15`, `solana-account` to `4`, `solana-system-interface` to `3`, `insta-cmd` to `0.7`, and `object` to `0.40`.
- Bump the JS Codama toolchain (`codama` to `1.10`, `@codama/renderers-js` to `2.3`) and regenerate all IDLs and Rust/JS clients.
- Keep `syn` at `2` by pinning `clap` `4.6.3`, `thiserror` `2.0.18`, `serde` `1.0.228`, `tokio-macros` `2.7.1`, `bytemuck_derive` `1.11.0`, and `enum-ordinalize-derive` `4.4.1` (newer releases of these derive crates require `syn` 3).
- Resolves `RUSTSEC-2026-0097` (unsound `rand` 0.7.3) and `RUSTSEC-2026-0173` (unmaintained `proc-macro-error2`), dropping them from the dependency tree, and updates `jiff`, `defmt`, `env_logger`, `solana-logger`, and `crossbeam-epoch` to patched versions.
