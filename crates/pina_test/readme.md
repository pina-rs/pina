# `pina_test`

Host-side Surfpool integration test support for Pina programs.

`ProgramTest` is the high-level fixture used by projects created with `pina init`. It reads the SBF artifact supplied by `pina test`, starts an embedded Surfpool instance on dynamic ports with upstream RPC access disabled, deploys the program at its declared address, and owns deterministic shutdown.

Tests can then focus on program behavior:

```rust,ignore
let program_id = Pubkey::new_from_array(ID.to_bytes());
let mut program = ProgramTest::start(program_id).await?;

program.send(&instruction_data, account_metas)?;
let state = program.account(&state_address)?;

program.stop()?;
```

The fixture also supports funding arbitrary addresses, additional transaction signers, balance reads, and prebuilt instructions. `TestError::operation` and `TestError::message` make failure assertions readable without parsing display text. `OfflineSurfnet` remains available when a test needs to control deployment itself.

Generated programs keep `pina_test` in a dedicated `tests/surfpool` Cargo package with its own workspace boundary. Native tests therefore do not resolve, compile, or link Surfpool, and SBF builds cannot enable the host dependency.

This crate is for host tests only. Do not enable it in an SBF build.

The crate is intentionally isolated from Pina's main Cargo workspace: Surfpool 1.5 uses Agave 4.1, while the workspace's current Mollusk release uses Agave 4.2. The separation keeps both test layers reproducible without forcing either runtime into the other's dependency graph.

See [security.md](security.md) for the offline-only trust boundary, dependency audit exceptions, and package-specific license policy.
