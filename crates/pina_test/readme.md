# `pina_test`

<p align="center">
	<img src="https://raw.githubusercontent.com/pina-rs/pina/main/.github/assets/logo.png" alt="The Pina logo: a low-poly origami pineapple" width="140">
</p>

Host-side Surfpool integration test support for Pina programs.

<!-- {=crateReadmeBadgeRow:"pina_test"} -->

[![Crates.io](https://img.shields.io/badge/crates.io-pina__test-orange?logo=rust)](https://crates.io/crates/pina_test) [![Docs.rs](https://img.shields.io/badge/docs.rs-pina__test-1f425f?logo=docs.rs)](https://docs.rs/pina_test/) [![CI](https://github.com/pina-rs/pina/actions/workflows/ci.yml/badge.svg)](https://github.com/pina-rs/pina/actions/workflows/ci.yml) [![Coverage](https://codecov.io/gh/pina-rs/pina/branch/main/graph/badge.svg)](https://codecov.io/gh/pina-rs/pina) [![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://opensource.org/license/apache-2.0)

<!-- {/crateReadmeBadgeRow} -->

`ProgramTest` is the high-level fixture used by projects created with `pina init`. It reads the SBF artifact supplied by `pina test`, starts an embedded Surfpool instance on dynamic ports with upstream RPC access disabled, deploys the program at its declared address, and owns deterministic shutdown.

Tests can then focus on program behavior:

```rust,ignore
let program_id = Pubkey::new_from_array(ID.to_bytes());
let mut program = ProgramTest::start(program_id).await?;

// Fund a counter PDA authority before the first transaction.
program.fund(&user_pubkey, 1_000_000_000)?;

// Send instruction data and accounts (the counter was initialized in an
// earlier step); the payer signs and confirms.
program
	.send(&[INCREMENT_DISCRIMINATOR], vec![
		AccountMeta::new(authority.pubkey(), true),
		AccountMeta::new(counter_pda, false),
	])?;

let state = program.account(&counter_pda)?;
assert_eq!(state.count, 1);

program.stop()?;
```

The payer signs and submits by default; `send_with_signers` adds program-specific signers, and `TestError::operation` plus `TestError::message` make failure assertions readable without parsing display text. `OfflineSurfnet` remains available when a test needs to control deployment itself.

## Test historical compatibility

Run `pina test --compatibility` to verify the checked-in migration history and run the complete Surfpool suite against the latest SBF artifact. `compatibility_mode()` returns `true` during that run. Use the signal to add expensive historical matrices without skipping current-flow tests.

Build fixtures from checked-in golden bytes. Do not encode them with the current generated type.

```rust,ignore
let old_state = HistoricalAccount::new(
	0,
	state_address,
	program.program_id(),
	include_bytes!("fixtures/state-v0.bin").to_vec(),
);
program.install_historical_account(&old_state)?;

let old_update = HistoricalInstruction::new(
	0,
	include_bytes!("fixtures/update-v0.bin").to_vec(),
	vec![AccountMeta::new(authority.pubkey(), true)],
);
program.send_historical_instruction(&old_update, &[&authority])?;

let old_event = HistoricalEvent::new(
	0,
	include_bytes!("fixtures/value-changed-v0.bin").to_vec(),
);
ValueChangedEvent::with_current_event_data(
	old_event.data(),
	|current, source_version| {
		assert_eq!(source_version, old_event.version());
		let event = ValueChangedEvent::try_from_bytes(current)?;
		assert_eq!(event.memo.get(), 0);
		Ok(())
	},
)?;
```

`HistoricalInstruction` preserves the old positional account list. An old request can therefore omit an optional suffix that the current process added. `HistoricalAccount` installs the complete old account state, including the discriminator and migration version.

`HistoricalEvent` preserves immutable log bytes. The generated event projection validates their exact historical shape, returns current-shape bytes in caller-owned scratch space, and reports the source version so a field absent from the old event is not confused with a field that was emitted as its default value.

For rejection tests, protect every account that the migration can touch:

```rust,ignore
program.expect_historical_rejection_with_rollback(
	&malformed_update,
	&[state_address, treasury_address],
	&[&authority],
)?;
```

The assertion accepts only an error from program execution. Signing and RPC failures do not satisfy it. After rejection, the assertion compares the protected accounts byte-for-byte, including lamports, ownership, the executable flag, and the rent epoch.

Generated programs keep `pina_test` in a dedicated `tests/surfpool` Cargo package with its own workspace boundary. Native tests therefore do not resolve, compile, or link Surfpool, and SBF builds cannot enable the host dependency.

This crate is for host tests only. Do not enable it in an SBF build.

`pina_test` lives in Pina's main Cargo workspace. Its dependencies are declared as ranges, so the resolver unifies Surfpool's host runtime with the workspace's Mollusk-based tests on a single Agave train — no exact version pins, no separate workspace.

See [security.md](security.md) for the offline-only trust boundary, dependency audit exceptions, and package-specific license policy.

## Known runtime limitations

Surfpool 1.5 cannot derive CPI signers for PDAs with four or more seed arguments (five including the bump); derivations that work on the host and on mainnet fail there with `Provided seeds do not result in a valid address`. Programs seeding PDAs with four arguments cannot run their Surfpool suites end to end until the runtime is fixed; pin or skip those flows loudly.
