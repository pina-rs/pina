---
pina_test: none
---

# Patch the emSign advisories and suppress the old hits

The advisory database published three advisories on 2026-10-21 for certificate paths through distrusted emSign intermediates (RUSTSEC-2026-0381, -0382, -0383), failing the `cargo-deny` and `security` gates for every pull request. The patched lines are now in the lockfile: `rustls-webpki` 0.103.16, `webpki-root-certs` 1.0.10, and `webpki-roots` 1.0.10. The remaining hits are on `rustls-webpki` 0.101.7 and `webpki-roots` 0.25.4, reached only through `pina_test → surfpool → txtx-addon-kit → reqwest 0.11`, and on the `webpki-roots` 0.26.11 shim under `pina_test → solana-client → tokio-tungstenite`, which re-exports the patched 1.x roots. No shipped crate links any of them, so they are suppressed in `deny.toml` alongside the existing rustls 0.21 entries, tracked in #621.
