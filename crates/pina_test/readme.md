# `pina_test`

Host-side Surfpool integration test support for Pina programs.

`OfflineSurfnet` starts an embedded Surfpool instance on dynamic ports with upstream RPC access disabled. It provides the narrow operations generated Pina tests need: deploy a real SBF artifact, submit an instruction with a funded payer, inspect the deployed program, and shut down synchronously.

Generated programs keep `pina_test` in a dedicated `tests/surfpool` Cargo package with its own workspace boundary. Native and Mollusk tests therefore do not resolve, compile, or link Surfpool, and SBF builds cannot enable the host dependency.

This crate is for host tests only. Do not enable it in an SBF build.

The crate is intentionally isolated from Pina's main Cargo workspace: Surfpool 1.5 uses Agave 4.1, while the workspace's current Mollusk release uses Agave 4.2. The separation keeps both test layers reproducible without forcing either runtime into the other's dependency graph.

See [security.md](security.md) for the offline-only trust boundary, dependency audit exceptions, and package-specific license policy.
