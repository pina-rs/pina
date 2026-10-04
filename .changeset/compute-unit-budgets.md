---
pina_cli: feat
pina_test: feat
pina_codama_renderer: feat
pina_cli_renderer: feat
pina_codama_renderer_cli: feat
pina_skill: docs
---

# Generate clients that request recorded compute unit limits

Most transactions request the runtime's default compute unit limit, and priority fees are charged per requested unit, so they pay for units they never use. Pina now measures what each instruction costs in the program's Surfpool suite and gives every generated client a tight, evidence-based limit.

- **`pina test --record-compute-units`** runs the complete Surfpool suite and writes `compute-units.json` beside the program's `Cargo.toml`: the most compute units each instruction consumed in a successful simulation, keyed by IDL name, with the sample count and the SHA-256 of the measured SBF build. The file is deterministic. Instructions no successful test sent are left out and named in a warning. The flag conflicts with `--unit` and `--filter`, and a failing suite writes nothing. `pina test` now also clears `PINA_CU_MANIFEST`, `PINA_CU_PROGRAM`, and `PINA_CU_RECORD_FILE` from the suite's environment, so a stray benchmark manifest can no longer deploy a different artifact.
- **IDL plugin.** `pina idl`, `pina build`, and `pina generate` attach each measurement as a `pinaComputeUnits` plugin node, `{ "measured": n, "limit": m }`, with `limit = round_up_to_100(measured × (100 + margin_percent) / 100) + 300`. `[compute_units] margin_percent` in `pina.toml` defaults to `20`, and the 300 units cover the `SetComputeUnitLimit` and `SetComputeUnitPrice` instructions, which each consume 150 compute units — measured by a new `pina_test` test, not assumed. Limits cap at the 1,400,000 unit transaction maximum. A measurement for an instruction the program no longer declares fails `pina generate`, whose error leads with the fix: re-record, or remove the stale entry. `pina build`, `pina idl`, and `pina test` warn about it and ignore it, and the recording build never reads the file it replaces, so a stale entry cannot block its own fix. `pina generate` also warns when `target/deploy` holds a different build than the one measured.
- **Rust clients** (`pina_codama_renderer`) export `<NAME>_MEASURED_COMPUTE_UNITS` and `<NAME>_COMPUTE_UNIT_LIMIT` beside each discriminator and a crate-level `set_compute_unit_limit_instruction(units)`, with no new dependency. `pina_codama_renderer::compute_units` owns the plugin contract.
- **TypeScript and Dart clients** gain the same constants and a `get<Program>ComputeUnitLimit(instructions)` helper that sums the limits of the program's instructions in a transaction and returns no limit when one of them is unmeasured.
- **Generated CLIs** (`cli-rust`, `cli-ts`, `cli-dart`) add one `SetComputeUnitLimit` with the instruction's recorded limit to every command, accept `--compute-unit-limit <UNITS>` to override it, and report `--simulate` consumption against the limit requested. The TypeScript CLI's `--simulate --json` output no longer throws on the RPC's bigint consumption figure. In `@pina-rs/codama-renderer-cli`, `extractCliModel` sets the new optional `InstructionModel.computeUnitLimitName` for measured instructions; a model built without it renders commands that request no limit, as before.
- **`pina_test`** records `discriminatorBytes` (the leading eight bytes of instruction data, so discriminators of any width are attributed correctly) and whether each simulated transaction succeeded, alongside the existing `discriminator` byte. The v1 `send_transaction` path records a failed simulation instead of returning early, so a test sees the same execution error with or without recording. New `ProgramTest::send_instructions` and `ProgramTest::simulate_compute_units` send several instructions in one legacy transaction and measure one.

Every example now commits its `compute-units.json`, and `codama/idls` plus every generated client carry the resulting limits. `counter_program` gains a Surfpool test, run by `test:surfpool`, that sends `increment` through the generated client under its generated limit and shows one compute unit below the measurement fails.
