# `pina_test`

<p align="center">
	<img src="https://raw.githubusercontent.com/pina-rs/pina/main/.github/assets/logo.png" alt="The Pina logo: a low-poly origami pineapple" width="140">
</p>

Host-side Surfpool integration test support for Pina programs.

<!-- {=crateReadmeBadgeRow:"pina_test"} -->

[![Crates.io](https://img.shields.io/badge/crates.io-pina**test-orange?logo=rust)](https://crates.io/crates/pina_test) [![Docs.rs](https://img.shields.io/badge/docs.rs-pina**test-1f425f?logo=docs.rs)](https://docs.rs/pina_test/) [![CI](https://github.com/pina-rs/pina/actions/workflows/ci.yml/badge.svg)](https://github.com/pina-rs/pina/actions/workflows/ci.yml) [![Coverage](https://codecov.io/gh/pina-rs/pina/branch/main/graph/badge.svg)](https://codecov.io/gh/pina-rs/pina) [![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://opensource.org/license/apache-2.0)

<!-- {/crateReadmeBadgeRow} -->

`ProgramTest` is the high-level fixture used by projects created with `pina init`. It reads the SBF artifact supplied by `pina test`, starts an embedded Surfpool instance on dynamic ports with upstream RPC access disabled, deploys the program at its declared address, and owns deterministic shutdown.

Tests can then focus on program behavior:

```rust,ignore
let program_id = Pubkey::new_from_array(ID.to_bytes());
let mut program = ProgramTest::start(program_id).await?;

// Fund a counter PDA authority before the first transaction.
program.fund(&user_pubkey, 1_000_000_000)?;

// Send instruction data and accounts; the payer signs and confirms.
program
	.send(&[INCREMENT_DISCRIMINATOR], vec![
		AccountMeta::new(authority.pubkey(), true),
		AccountMeta::new(counter_pda, false),
	])?;

let state = program.account(&counter_pda)?;
assert_eq!(state.lamports, LAMPORTS_PER_SOL);

program.stop()?;
```

The payer signs and submits by default; `send_with_signers` adds program-specific signers, and `TestError::operation` plus `TestError::message` make failure assertions readable without parsing display text. `OfflineSurfnet` remains available when a test needs to control deployment itself.

Generated programs keep `pina_test` in a dedicated `tests/surfpool` Cargo package with its own workspace boundary. Native tests therefore do not resolve, compile, or link Surfpool, and SBF builds cannot enable the host dependency.

This crate is for host tests only. Do not enable it in an SBF build.

The crate is intentionally isolated from Pina's main Cargo workspace: Surfpool 1.5 uses Agave 4.1, while the workspace's current Mollusk release uses Agave 4.2. The separation keeps both test layers reproducible without forcing either runtime into the other's dependency graph.

See [security.md](security.md) for the offline-only trust boundary, dependency audit exceptions, and package-specific license policy.

## Known runtime limitations

Surfpool 1.5 cannot derive CPI signers for PDAs with four or more seed arguments (five including the bump); derivations that work on the host and on mainnet fail there with `Provided seeds do not result in a valid address`. Programs seeding PDAs with four arguments cannot run their Surfpool suites end to end until the runtime is fixed; pin or skip those flows loudly.
