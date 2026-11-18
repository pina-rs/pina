---
pina_test: none
---

# Patch quinn advisories and suppress unmaintained chrono

The advisory database published RUSTSEC-2026-0523 and RUSTSEC-2026-0524 for `quinn-proto` 0.11.18, RUSTSEC-2026-0538 for `quinn-udp` 0.5.15, and RUSTSEC-2026-0543 marking `chrono` unmaintained, failing the `cargo-deny` and `security` gates on `main` and every open pull request. The lockfile now carries the patched `quinn-proto` 0.11.19 and `quinn-udp` 0.5.16; `quinn-udp` is reached only through `pina_test → solana-client → anza-quinn`. `chrono` has no maintained release, and is reached only through `pina_test`'s Surfpool (`surfpool-db → diesel`, `surfpool-types`) and Agave (`solana-runtime → solana-genesis-config`) host test stack, so `deny.toml` suppresses it with a reachability note. No runtime crate (`pina`, `pina_abi`, `pina_macros`, `pina_cli`, `pina_profile`) depends on any of these crates.
