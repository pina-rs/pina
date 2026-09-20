# Account migration do's and don'ts

Produced by the 2026-09-19 security sweep (static analysis of the migration machinery plus dynamic execution of the attack matrix against real SBF artifacts in an offline Surfnet). Full evidence: `migration-static.md` (findings M1–M7, blocked-attack log B1–B14) and `dynamic-migrations.md` (executed scenarios) in this directory. This file is the maintainer-facing distillation, structured for adoption into `docs/src/migrations/`.

## The one property everything rests on

**Transitions are structurally pure.** A generated transition is `include!`d author code whose only signature is `fn migrate(data: &mut [u8])` (+ `target_size`/`working_size` for compact steps). A transition cannot read sysvars, other accounts, time, or randomness — program semantics after a migration are a pure function of stored bytes. This signature constraint — not convention — is what makes the permissionless reserved route safe. Never widen it: adding a context parameter to transitions without a threat-model pass would convert the entire permissionless model into an exploit surface.

## The framework enforces (you get these for free)

1. **Version envelope integrity** — stale, unknown, and future versions are classified before any decode (`InvalidMigrationVersion` / `MigrationRequired`), including compact `update` paths. Version-byte forging and trailing-byte smuggling both fail closed.
2. **Exact physical layouts** — every historical and current representation is length-exact and field-validated on plan, per step, and on destination; the version marker commits last.
3. **Transition purity** — see above; also sha256-pinned files, TODO-scan in manual mode, and source-schema re-verification against the manifest on every compile. Missing pins fail the build — there is no downgrade hole.
4. **All-or-nothing ladder** — catchable failures happen before the first effect; anything after the first effect aborts the instruction and Solana rolls the transaction back.
5. **Bounded cost** — workspace ≤ 1 KiB (compile assert); account growth ≤ 10 KiB above the pre-instruction size per step; lamports only ever flow payer→account with a declared ceiling; the payer must sign when it pays.
6. **One rewrite per account per invocation** — duplicate slots, payer/target aliasing, foreign ownership, and read-only targets all reject before any effect.
7. **Shrink exactness** — resize-down truncates; no stale tail survives; overfunding is unreachable through generated transitions.

## The framework relies on YOU to uphold (each violated rule is a finding)

1. **DON'T encode authority meaning in a version number.** The reserved `0xFF` route is permissionless by design: anyone can migrate anyone's account at their own expense. "v2 = admin approved" is an exploit, not a design. If you need a gated route, add an authority-signature check in business handlers — not in the envelope.
2. **DON'T remove, reorder, or retype fields.** Evolution is append-only and additive. The macro re-verifies the current source against the manifest, but the manifest can be regenerated — history review is the real control. Never remove an authority field from a historical schema: old accounts still carry it and the versioned view still reads it.
3. **DON'T reuse discriminator values across versions or contracts.** Same-enum duplicates are a compile error; cross-contract reuse is only conventionally avoided (manifest-lint candidate).
4. **DO write every added byte in every transition.** The account executor does not zero the grown region before `apply_migration` (finding M4); until an executor pre-zero lands, a skipped added field bakes the account's own realloc residue into persistent, version-stamped state — stale-data resurrection inside one account. Prefer `automatic` mode for additive steps; review `manual` transitions like consensus code.
5. **DON'T branch a transition on anything but stored bytes.** The signature enforces this today; keep it that way.
6. **DO keep histories shorter than the inline budget.** `MAX_INLINE_STEPS = 8` is hardcoded: an account 9+ versions behind can neither migrate on the reserved route nor normalize inline — it is stranded for the life of the program. Compact history before `current − oldest > 8`; clients can also drive repeated `[0xFF]` calls (≤ 8 steps each) to walk a deep ladder — document that in your release tooling.
7. **DO authorize before you mutate in inline migrations.** A handler that runs `MigrateAccount`/resize before its authority check is saved only by transaction atomicity; add one early `Ok(())` and it becomes a free-effect gadget. (Lint candidate: migration effect above an authority assert in the same handler.)
8. **DO treat `manual` transition edits as program upgrades.** File and manifest pin change in one commit; review together; run the historical Surfpool suite before shipping. Transition review is upgrade review.
9. **DON'T assume the owner performed a migration.** Telemetry or logic keyed on "someone migrated my account" must never gate payouts or state transitions — an attacker may have run it, and may have locked their own lamports inside the account as a side effect (self-cost grief; the lamports are unrecoverable to them and inert to you).
10. **DO prefer migrating over `try_from_bytes_versioned`** whenever a writable touch is schedulable. The versioned view is a read path for accounts the transaction cannot write, and every match arm it forces is a place to mishandle an old layout.

## Runtime defects found by this sweep (fix in framework)

| ID | Finding                                                                                                                                                             | Fix                                                                                                                                                                     |
| -- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| M4 | Account executor skips zero-fill of the grown region before `apply_migration` (`crates/pina/src/migration.rs:814-820`); instruction/event paths already zero theirs | Zero `[old_len..working_size]` right after growth resize (bounded by the 10 KiB cap, so cheap) + a Kani/lint check that transitions write every added byte              |
| M2 | `MAX_INLINE_STEPS = 8` hardcoded (`crates/pina_macros/src/migration.rs:1098`); accounts 9+ behind strand permanently                                                | Make the cap a `pina.toml` setting with a computed-bounds assert, or generate a chunked deep ladder; at minimum document the client-driven repeated-`[0xFF]` workaround |
| M3 | Post-mutation invariant failures present as on-chain panics (atomic, by design)                                                                                     | Add `pina migrations verify` — replay every transition against generated fixtures at CI time                                                                            |

## Lints and checks proposed by this sweep

- `pina_lints`: inline-migration-before-authorization; `manual` transition on a contract whose fields suggest authority semantics; history depth approaching 8; cross-version discriminator-reuse manifest check.
- Compile-time: reject non-trailing optional accounts (shared root cause with tonight's N2 slot-shift finding — see `macro-codegen.md`).
- Docs: one page stating the permissionless-reserved-route model and the purity invariant in `docs/src/migrations/`.
