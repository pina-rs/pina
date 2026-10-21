---
pina_cli: patch
---

# Patch the webpki emSign advisories

The advisory database published RUSTSEC-2026-0381, -0382, and -0383 on 2026-10-21, failing the `cargo-deny` and `security` gates for every pull request. Each lets TLS certificate validation trust the distrusted emSign intermediates. `pina_cli` ships `rustls-webpki` and `webpki-roots` in its `ureq` client, so the lockfile now carries the patched `rustls-webpki` 0.103.16, `webpki-roots` 1.0.10, and `webpki-root-certs` 1.0.10.

The remaining hits are `rustls-webpki` 0.101.7 and `webpki-roots` 0.25.4, which have no patched release on those lines. They are reached only through `pina_test → surfpool → txtx-addon-kit → reqwest 0.11`, a host/test stack that no shipped crate links, so they are suppressed in `deny.toml` alongside the earlier rustls 0.21 advisories until the Surfpool train drops `reqwest 0.11` (tracked in #621). The `vite` override moves to 7.3.7 for the three `launch-editor` advisories in the vitest dev stack, which the `security` job reaches once the Rust audit passes.
