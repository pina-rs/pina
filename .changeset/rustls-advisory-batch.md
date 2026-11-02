---
pina_test: none
---

# Patch the rustls stack and suppress the 0.21 advisories

The advisory database published four rustls vulnerabilities (RUSTSEC-2026-0346, -0347, -0348, -0349) on 2026-10-14, failing the `cargo-deny` gate for every pull request. The 0.23 line is patched at 0.23.46: the lockfile now carries it for `pina_cli`'s `ureq` graph and the `reqwest 0.12` stack, alongside `ruzstd` 0.9.1 for RUSTSEC-2026-0344. The end-of-life `rustls 0.21.12` hits were reached only through `pina_test → surfpool → txtx-addon-kit → reqwest 0.11`. Surfpool 1.7 resolves `reqwest` 0.12 and `rustls` 0.23, so that chain and its `deny.toml` suppressions are gone (resolves #621).
