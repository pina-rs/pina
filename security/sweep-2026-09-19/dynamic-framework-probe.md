# Framework probe — harness verification + shadow-PDA creation finding (2026-09-19)

Executed personally (not via sub-agent) to verify the sweep harness and to confirm the creation-side half of the shadow-PDA exploit class end-to-end against a real SBF artifact. Scratch probe retained at `worktrees/security-sweep-2026-09-19/tmp/sweep/harness-probe/`.

## Environment

- Sweep worktree: `worktrees/security-sweep-2026-09-19` (branch `security-sweep-2026-09-19`, cut at `feat/abi-version-reset` @ `3f7d38f9`).
- All 25 example SBF artifacts built via `devenv shell -- scripts/build-surfpool-examples.sh` into `target/surfpool/examples/` (build log: `tmp-sbf-build.log`, exit 0).
- Harness: `pina_test::ProgramTest::start_with_artifact` boots an offline Surfnet (surfpool), deploys the real `.so`, and drives instructions through RPC. Proven with `counter_program` end-to-end: deploy → `Initialize` → `Increment` → state readback (`[1, 0, bump, count…8]` — discriminator, migration version byte, bump, u64 count).
- Wire format confirmed: instruction data = `[discriminator, version_byte, payload…]`; the migration envelope's version byte is mandatory even for zero-field payloads. Account data = `[account_discriminator, version, fields…]`. The doc comment in `examples/counter_program/src/lib.rs` (state layout "10 bytes") is stale vs the shipped 11-byte layout — minor docs drift worth fixing.

## Finding FP1 — CONFIRMED (Low; example-level instance of a known class)

**Shadow PDAs are creatable end-to-end on `counter_program` via `CreateProgramAccountWithUncheckedBump`.**

1. **Location**: `examples/counter_program/src/lib.rs:184`; builder at `crates/pina/src/cpi.rs` (`CreateProgramAccountWithUncheckedBump`, ~line 465+); load path `CounterState::load_pda_mut` (`examples/counter_program/src/lib.rs:211`).
2. **Mechanism**: `Initialize` accepts an arbitrary bump from instruction data and the builder verifies only that the bump derives the submitted address — never that it is canonical. `Initialize([0, 0, bump=254], …)` succeeded (`Ok`) and created a second, fully valid counter PDA for the same `[b"counter", authority]` seed namespace. Loads are stored-bump (`load_pda_mut`), so the shadow account is a first-class citizen.
3. **Exploit scenario (executed)**: attacker (the authority itself here) sends Initialize twice with bump 255 (canonical) and 254 → two live counters, one authority. For counter this is self-inflicted by design; the same idiom in a program where the seed namespace is a singleton or where any consumer derives addresses canonically from seeds becomes the shadow-account/double-claim class.
4. **Severity justification**: Low as shipped — every inspected call site documents a per-site safety argument (counter: "seeds bind authority + signer required → duplicate is the signer's own namespace"; role_registry: "role entries keyed by registry address"). It is the default idiom in **8 examples** (counter, escrow, optional_accounts, profile, role_registry ×2, todo, validation, vesting), i.e. the template every downstream program copies, and nothing mechanical enforces the documented precondition.
5. **Fix**: (a) extend the lint pack with the audit's open proposal — flag `CreateProgramAccountWithUncheckedBump` / `load_pda*` / `with_pda` call sites whose seeds do not bind a required signer (lexical shape is identical to the M1-remediation lint already proposed for `with_pda`); (b) extend the IDL schema with `bumpVerification: canonical | stored | unchecked` so downstream auditors can see the choice; (c) fix the stale layout doc comment.

**Context**: `REQUIRE_CANONICAL_BUMP_BEFORE_PDA_WRITE` (`crates/pina_lints/src/lints/require_canonical_bump_before_pda_write.rs`) already gates validation-side `assert_seeds_with_bump` paths and its docs name the UncheckedBump exemption as deliberate — this finding is the residual gap, not a regression.

## Blocked attacks (harness probes)

| Attack                                    | Stopped by                                                    |
| ----------------------------------------- | ------------------------------------------------------------- |
| `Increment` without authority signature   | `assert_signer` (tx rejected)                                 |
| Unknown discriminator `[0xAA, 0xBB]`      | entrypoint exact-width dispatch → `invalid instruction data`  |
| Version-byte omission on instruction data | exact-width instruction parse rejects (envelope is mandatory) |

## Harness notes for future sweeps

- `send_with_signers(instruction, &[&dyn Signer])` takes a built `Instruction`; the payer is always first signer and fee payer; extra signers need `program.fund` first.
- `Pubkey::find_program_address` returns `(address, bump)`; canonical bump for the probe's fixture is exactly 255 — beware debug-mode `+1` overflow when synthesizing "wrong" bumps.
- Offline Surfnet instances allocate ports dynamically; concurrent sweeps work.
