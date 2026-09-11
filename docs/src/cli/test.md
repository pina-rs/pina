# Test a Program

`pina test` keeps two deliberately different feedback loops.

```bash
# Fast host tests, including Mollusk instruction tests.
pina test --unit

# Build the real SBF program and run the generated Surfpool test package.
pina test

# Select tests by name in either layer.
pina test --filter initialize
pina test --unit --filter rejects_wrong_owner

# Verify migration history and run historical compatibility cases.
pina test --compatibility
```

## Default SBF workflow

The default command:

1. discovers the program from `--project` or the current directory;
2. builds the release `bpfel-unknown-none` artifact with `bpf-entrypoint` enabled;
3. fails if the expected `.so` or `tests/surfpool` package is missing;
4. publishes the artifact at `target/deploy/<library-name>.so`;
5. sets `PINA_SBF_ARTIFACT` and runs the ignored test in `tests/surfpool`.

Projects created by `pina init` put the host-only `pina_test` dependency in a dedicated Cargo package under `tests/surfpool`. Its standalone workspace boundary isolates Surfpool's host runtime from the program's Mollusk dependencies and SBF build. `pina_test` owns the Surfpool 1.5 compatibility graph and exposes only the offline lifecycle, deployment, and instruction operations the generated test needs. The generated test starts an embedded, offline Surfnet on dynamic ports, deploys the explicit `.so` at a non-system declared program address, submits and confirms the starter `Initialize` instruction, and calls `Surfnet::stop` before returning. `Surfnet` also stops itself through `Drop` if an assertion panics. This avoids fixed ports, background daemon processes, readiness sleeps, leaked validator instances, and host SDK dependencies entering the on-chain artifact.

The embedded SDK is the correct fit for isolated integration tests. See Surfpool's official [SDK overview](https://docs.surfpool.run/sdk/overview) and [installation guide](https://docs.surfpool.run/sdk/installation).

Pina's prebuilt CLI supports more operating systems and CPU targets than Surfpool currently publishes. On a machine where Surfpool or the SDK cannot run, `pina test --unit` remains available; the SBF integration command fails with the missing dependency instead of silently skipping it.

## Unit mode

`--unit` runs `cargo test` without building SBF or requiring Surfpool. Pina intentionally leaves Cargo attached to the terminal in both test modes so filters, output, and interrupts behave like direct Cargo use. Keep pure logic and serialization tests native, and use Mollusk for fast instruction-level VM tests. Surfpool adds a real RPC boundary; it does not replace those faster layers.

## Compatibility mode

`pina test --compatibility` verifies the checked-in ABI history and transition hashes before it runs the default SBF workflow. It sets `PINA_COMPATIBILITY=1` for the Surfpool package. Use `pina_test::compatibility_mode()` to enable large historical fixture matrices.

Compatibility mode runs the complete Surfpool suite. Do not use the signal to skip current-flow tests.

Build historical cases with `HistoricalAccount` and `HistoricalInstruction`. These types preserve golden bytes from a released version. `ProgramTest::install_historical_account` installs old account data directly. `ProgramTest::send_historical_instruction` submits the old payload and positional account metas to the latest SBF artifact.

For a rejected migration, call `ProgramTest::expect_historical_rejection_with_rollback`. The helper accepts only a program-execution failure. It then compares each protected account with its exact pre-transaction state.

## Options

| Option                  | Meaning                                                   |
| ----------------------- | --------------------------------------------------------- |
| `--project <DIR>`       | Project directory or a directory below it                 |
| `--unit`                | Run only native Rust and Mollusk tests                    |
| `--compatibility`       | Enable historical fixtures in the complete SBF test suite |
| `-f, --filter <FILTER>` | Pass a test-name filter to Cargo                          |

Run `pina test --help` for the authoritative command contract.
