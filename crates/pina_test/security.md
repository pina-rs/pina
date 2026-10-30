# Security boundaries

`pina_test` is a host-only integration-test utility. Its supported path starts `OfflineSurfnet`, deploys a local SBF artifact, and sends transactions to the loopback RPC endpoint allocated by the SDK. It must not be linked into an SBF program or used to connect to an upstream network.

Surfpool 1.7.0 (Agave 4.3, `litesvm` 0.18) resolves `reqwest` 0.12 and `rustls` 0.23, so the legacy `reqwest` 0.11 → `rustls` 0.21 → `rustls-webpki` 0.101 chain is gone. The HTTP/TLS advisory exceptions the workspace previously carried for RUSTSEC-2025-0134, -2026-0098, -2026-0099, -2026-0104, -2026-0346, -2026-0349, -2026-0381, and -2026-0382 are removed with it.

The advisories this version still brings through code outside the offline path:

- RUSTSEC-2022-0093 and RUSTSEC-2024-0344 are in Agave's legacy Ed25519 precompile dependency. Pina's starter transaction does not invoke that precompile.
- RUSTSEC-2024-0421 is in the URL 1.x IDNA implementation. The supported path uses an SDK-provided numeric loopback URL and never decodes an international domain name.
- RUSTSEC-2024-0449 is the unmaintained `unsafe-libyaml`, reached through Surfpool's deprecated `serde_yaml`. Offline mode never parses YAML.

The remaining unmaintained-crate notices (`ansi_term`, `atty`, `bincode`, `derivative`, `instant`, `libsecp256k1`, `net2`, `paste`) and the `crypto-bigint` timing advisory are recorded per-ID in `deny.toml`, which is the repository's single advisory policy. `security:deny` sets `unused-ignored-advisory = "deny"`, so an exception that stops matching the graph fails the gate instead of lingering.

The workspace license policy additionally permits 0BSD, bzip2-1.0.6, CC0-1.0, and CDLA-Permissive-2.0. These are permissive licenses used by Surfpool or Agave transitive dependencies; they are not broad unknown license exceptions.
