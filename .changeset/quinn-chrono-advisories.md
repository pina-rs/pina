---
pina_test: none
---

# Patch quinn advisories and suppress unmaintained chrono

The advisory database published RUSTSEC-2026-0523 and RUSTSEC-2026-0524 for `quinn-proto` 0.11.18, RUSTSEC-2026-0538 for `quinn-udp` 0.5.15, RUSTSEC-2026-0543 marking `chrono` unmaintained, and GHSA-fc5q-39xc-vjpf for `picomatch` <=4.0.7, failing the `cargo-deny` and `security` gates on `main` and every open pull request. The lockfile now carries the patched `quinn-proto` 0.11.19 and `quinn-udp` 0.5.16. `chrono` has no maintained release and is reached only through `pina_test`'s Surfpool and Agave host test stack, so `deny.toml` suppresses it, tracked in #716. `picomatch` has no patched release and is reached only through npm devDependencies, so `pnpm-workspace.yaml` ignores it until a fix ships. No runtime crate (`pina`, `pina_abi`, `pina_macros`, `pina_cli`, `pina_profile`) and no published npm package's runtime graph depends on any of these.
