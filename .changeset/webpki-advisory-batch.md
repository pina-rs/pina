---
pina_test: none
---

# Patch webpki and suppress the reqwest 0.11 copies

The advisory database published three webpki vulnerabilities (RUSTSEC-2026-0381, -0382, -0383), failing the `cargo-deny` gate for every pull request. The lockfile now carries the fixed `webpki-roots` 1.0.10, `webpki-root-certs` 1.0.10, and `rustls-webpki` 0.103.16. The remaining hits are `webpki-roots` 0.25.4 and `rustls-webpki` 0.101.7, which have no fixed release on their lines and are reached only through `pina_test → surfpool → txtx-addon-kit → reqwest 0.11`. That stack is host/test only, and no shipped crate links it, so those two are suppressed in `deny.toml` until the Surfpool train drops `reqwest 0.11`, tracked in #621.
