---
pina: feat
pina_macros: feat
pina_lints: feat
pina_cli: feat
---

# symmetric aliasing, const PDAs, and readonly lint

- Enforce symmetric mutable account aliasing checks (`PinaProgramError::DuplicateMutableAccount`) across all mutable account slots, allow `readonly + mut` combinations under Pinocchio runtime borrow guards, support opt-in `#[pina(unique)]` to forbid aliasing against any other present account slot (readonly, mutable, or remaining), and preserve Anchor v2 sentinel immunity for omitted optional accounts (`address == program_id`).
- Add inline pairwise aliasing fast paths (2/3/4-mut) and compact zero-`.rodata` bitmask validation in `#[derive(Accounts)]`, eliminating unnecessary runtime overhead and binary bloat.
- Add `#[pina(unique)]` to `Multisig::vault_transaction_execute` in `examples/multisig_program` to reject proposal-account reuse.
- Add compile-time PDA derivation (`try_find_program_address_const`, `find_program_address_const`, and generated `try_find_pda_const` / `find_pda_const` for constant-seed PDAs) and optimize SBF PDA verification via direct `MaybeUninit` syscalls (`-74%` CU on legacy ATA verification, `-64.5%` total CU on `staking_rewards_program`, `-41.6%` on `vesting_program`).
- Add `invoke_signed_with_rent*` builders, opt-in `const-rent` helpers (`mainnet_rent`, `minimum_balance_const`), zero-double-zeroing creation via `pinapod`'s `initialize_zeroed`, and single-pass directional `UpdateResizableAccount` realloc with post-grow divergence guards (`-28.8%` to `-68.4%` CU and `-1,408 B` on `account_realloc_program`, `-1,912 B` on `compact_accounts_program`).
- Track writable vs readonly loader duplicates with 2-bit flags in `entry::walk_accounts`, factor cold failure logging and branch hints in account validators, and shrink ELF sizes across 23 of 28 example programs (up to `-6,496 B` on `multisig_program`) with zero instruction-CU or size regressions.
- Add the `Deny`-level `deny_readonly_account_copy_escalation` compile-time security lint in `pina_lints` and register it in `pina_cli` and the lint reference docs to block mutable borrow escalation on `AccountView` values copied from shared references.
