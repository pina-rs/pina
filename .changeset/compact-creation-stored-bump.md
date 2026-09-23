---
pina: breaking
pina_macros: feat
---

# Reject compact creation patches that store a different bump

`CreateCompactProgramAccountWithBump` validated its `bump` argument canonically but never checked the patch's own stored bump field, an independent value the patch writes into account state. A caller could pass the canonical bump while the patch stored a different one; the account was created, held rent, and could then only be loaded through the bump it actually stores — the stranded-rent failure mode the canonical loaders exist to prevent (#418).

The builder now reads the account's declared bump field back out of the committed data and returns `PinaProgramError::StoredBumpMismatch` (0xFFFF_FFF0) when it disagrees, clearing the account data like any other failed initialization. The read is a single byte load at a compile-time offset, not a validation pass: `#[pda(bump = ...)]` emits the new `PinaCompactStoredBump` trait as a const prefix sum of the preceding header fields' pod sizes — the same mapping the compact derive stores inline, and sound because the compact grammar already forbids inline fields after dynamic ones — so the check adds only the load and compare to the creation path. Measured against the base branch, the three multisig creation instructions each consume 5 fewer compute units (the folded check removed a redundant re-borrow), `account_realloc_program/initialize` consumes 5 more, and `compact_accounts_program/initialize` consumes 4 more; those two approved totals are ratcheted in `scripts/compute-unit-policy.json` for the stored-bump invariant.

`CreateCompactProgramAccountWithBump::invoke*` now requires `T: PinaCompactStoredBump`. A compact account without a declared bump field must add the declaration or move to `CreateCompactProgramAccount`, whose `invoke_with_bump` threads the derived bump into the patch by construction and is unaffected.
