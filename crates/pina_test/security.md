# Security boundaries

`pina_test` is a host-only integration-test utility. Its supported path starts `OfflineSurfnet`, deploys a local SBF artifact, and sends transactions to the loopback RPC endpoint allocated by the SDK. It must not be linked into an SBF program or used to connect to an upstream network.

Surfpool 1.5.0 currently brings seven advisories through code outside this offline path:

- RUSTSEC-2022-0093 and RUSTSEC-2024-0344 are in Agave's legacy Ed25519 precompile dependency. Pina's starter transaction does not invoke that precompile.
- RUSTSEC-2024-0421 is in the URL 1.x IDNA implementation. The supported path uses an SDK-provided numeric loopback URL and never decodes an international domain name.
- RUSTSEC-2026-0258, RUSTSEC-2026-0098, RUSTSEC-2026-0099, and RUSTSEC-2026-0104 are in the legacy HTTP/TLS client graph. Offline Surfnet does not make an upstream HTTP or TLS connection.

The workspace audit ignores only these exact advisory IDs. Every other advisory remains a hard failure. Remove each exception when Surfpool publishes a compatible dependency graph containing the fix.

The workspace license policy additionally permits 0BSD, bzip2-1.0.6, CC0-1.0, and CDLA-Permissive-2.0. These are permissive licenses used by Surfpool or Agave transitive dependencies; they are not broad unknown license exceptions.
