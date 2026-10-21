---
pina_test: none
---

# Patch the webpki advisories and suppress legacy hits

The advisory database published RUSTSEC-2026-0381, -0382 and -0383 on 2026-10-19 and 2026-10-20, which failed the `security` and `cargo-deny` gates. The current lines are patched: the lockfile now carries `webpki-roots` 1.0.10, `webpki-root-certs` 1.0.10 and `rustls-webpki` 0.103.16. The remaining hits are on `webpki-roots` 0.25.4 and `rustls-webpki` 0.101.7, which are reached only through `pina_test → surfpool → txtx-addon-kit → reqwest 0.11`. That is the same host/test-only chain as the rustls 0.21 suppressions, so those two are suppressed in `deny.toml` until the Surfpool train drops `reqwest 0.11`, tracked in #621.
