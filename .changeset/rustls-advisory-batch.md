---
pina_test: none
---

# Patch the rustls stack and suppress the rustls 0.21 advisories

The advisory database published four rustls vulnerabilities (RUSTSEC-2026-0346, -0347, -0348, -0349) on 2026-10-16, failing the `cargo-deny` gate for every pull request. The 0.23 line is patched at 0.23.46: the lockfile now carries it for `pina_cli`'s `ureq` graph and the `reqwest 0.12` stack, alongside `ruzstd` 0.9.1 for RUSTSEC-2026-0344. The remaining hits are on the end-of-life `rustls 0.21.12`, reached only through `pina_test → surfpool → txtx-addon-kit → reqwest 0.11` — host/test stack only, no shipped crate links it — so those two are suppressed in `deny.toml` until the Surfpool train drops `reqwest 0.11`, tracked in #621 (same shape as the #619 `unsafe-libyaml` suppression).
