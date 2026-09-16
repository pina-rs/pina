---
pina: breaking
pina_cli: breaking
pina_macros: feat
---

# Name account decode failures and guard migration wire format

A fixed-account decode failure reported `InvalidAccountData` for both a length mismatch and a discriminator mismatch, so a stale or un-migrated representation was indistinguishable from bytes of a different type — the failure mode that makes a migration-envelope change undiagnosable from a client. The reader generated for `#[account]` now reports `InvalidAccountSize` (`0xFFFF_FFFB`) for a length mismatch and `InvalidDiscriminator` (`0xFFFF_FFFF`) for a discriminator mismatch, as does `as_account`'s own size check. A stale or future envelope already reported `MigrationRequired` and `InvalidMigrationVersion`; those codes now stay distinguishable from a structural failure too.

## Breaking change

The account reader's error codes change on the wire: `try_from_bytes` for a `#[account]` type now returns `InvalidAccountSize` (`0xFFFF_FFFB`) for a length mismatch and `InvalidDiscriminator` (`0xFFFF_FFFF`) for a discriminator mismatch, where both previously returned `InvalidAccountData`. A client branching on the old code must handle both new ones. Nothing is deployed on mainnet yet, which is why this lands as a major rather than needing a migration window.

The two failures are deliberately _not_ split on the generic trait defaults (`try_from_bytes_mut`, `validate_account_data`) or on the instruction and event readers. Splitting the generic defaults measured compute-unit regressions of 1-11 CU across example suites that load accounts, and `as_account` already validates size before it reaches them, so those callers keep a precise error either way. Instructions and events keep `InvalidInstructionData` for a length mismatch because they describe payload bytes rather than an account size. Accepted paths are unchanged from before; only the rejected arm differs, and both error constructions live in `#[cold] #[inline(never)]` functions.

Generated Dart clients now fail closed when an account the IDL marks as envelope-aware has no migration-version read to harden. The hardener previously treated "statement not found" the same as "already hardened" and shipped the generic constant decoder unchanged, so decoding could silently skip the version check. This mirrors the JavaScript hardener's generation-time assert.

`pina migrations make` writes a machine-checked `tests/abi_layout.rs` from the manifest, recording each contract's envelope geometry (`DISCRIMINATOR_BYTES`, `VERSION_OFFSET`, `VERSION_BYTES`, `MIGRATION_HEADER_SIZE`), payload size with `SIZE` derived from it, and every fixed field's absolute offset. A generated `layout_guard` module asserts those sizes against the values the macros computed for the current source and checks the manifest program id against `declare_id!`. `pina migrations check` fails with `AbiLayoutTestStale` or `AbiLayoutTestMissing` when the file no longer matches, so a layout change that forgets to regenerate goes red in the same pull request. The shared check used by `pina build` and `pina migrations status` is unchanged, so neither blocks on a guard file that predates it.

On a program that already has published deployments, `pina migrations make` refuses to record a first-time envelope for contracts that did not carry one, listing them and requiring `--envelope-ack`. Widening `[migrations].auto` — say from accounts to accounts and events — shifts bytes for every affected contract, so the command asks before recording it. A program with nothing published still envelops freely.
