# Research: Pinocchio Ecosystem Upgrade

**Date**: 2026-02-15 **Branch**: `001-pinocchio-upgrade`

## R1: Pinocchio 0.9.x → 0.10.x Migration

### Decision

Upgrade pinocchio from ^0.9 to ^0.10 with features `["cpi"]` enabled.

### Rationale

Pinocchio 0.10.0 is a fundamental architectural shift that integrates with the official Solana SDK types. The core pinocchio crate now re-exports types from dedicated SDK crates (`solana-account-view`, `solana-address`, `solana-program-error`, `solana-instruction-view`). This aligns pinocchio programs with the broader Solana ecosystem and provides long-term stability.

### Key Breaking Changes

| Old (0.9.x)                          | New (0.10.x)                           | Notes                       |
| ------------------------------------ | -------------------------------------- | --------------------------- |
| `AccountInfo`                        | `AccountView`                          | From `solana-account-view`  |
| `Pubkey`                             | `Address`                              | From `solana-address`       |
| `pinocchio::account_info`            | `pinocchio::account`                   | Module renamed              |
| `pinocchio::pubkey`                  | `pinocchio::address`                   | Module renamed              |
| `pinocchio::program_error`           | `pinocchio::error`                     | Module renamed              |
| `Instruction`                        | `InstructionView`                      | Requires `cpi` feature      |
| `AccountMeta`                        | `InstructionAccount`                   | Requires `cpi` feature      |
| `pinocchio::program::invoke_signed`  | `pinocchio::cpi::invoke_signed`        | Path changed                |
| `pinocchio::instruction::Signer`     | `solana_instruction_view::cpi::Signer` | Module changed              |
| `pinocchio::instruction::Seed`       | `solana_instruction_view::Seed`        | Module changed              |
| `AccountInfo::key()`                 | `AccountView::address()`               | Method renamed              |
| `AccountInfo::try_borrow_data()`     | `AccountView::try_borrow()`            | Method renamed              |
| `AccountInfo::try_borrow_mut_data()` | `AccountView::try_borrow_mut()`        | Method renamed              |
| `AccountInfo::realloc()`             | `AccountView::resize()`                | Method renamed              |
| `pinocchio::log` module              | REMOVED                                | Use `solana-program-log`    |
| `pinocchio::memory` module           | REMOVED                                | Use `solana-program-memory` |
| `std` feature                        | `alloc` feature (default)              | Feature renamed             |
| `HEAP_LENGTH`                        | `MAX_HEAP_LENGTH`                      | Constant renamed            |

### AccountView Methods (unchanged)

- `is_signer()`, `is_writable()`, `executable()`, `lamports()`, `owner()`, `data_len()`, `data_is_empty()`, `assign()`, `close()`

### New Features in 0.10.x

- `"cpi"` feature gate for CPI (opt-in)
- `"copy"` feature for Copy derives
- `"alloc"` feature (default, replaces `"std"`)
- Forward allocation in BumpAllocator (~7 CUs saved per alloc)
- `process_entrypoint` publicly exposed
- `MAX_TX_ACCOUNTS` constant (255)
- New sysvar implementation using `sol_get_sysvar` syscall

### Alternatives Considered

- **Stay on 0.9.x**: Rejected. Upstream development has moved to 0.10.x and all helper crates (token, system, etc.) now require 0.10.x types.
- **Use solana-program instead**: Rejected. Defeats the purpose of pina (performance, minimal dependencies).

---

## R2: pinocchio-pubkey Replacement

### Decision

Replace `pinocchio-pubkey` with `solana-address` (version ^2.0) using features `["decode"]` for `declare_id!` macro and `["syscalls"]` for PDA operations on-chain.

### Rationale

The `pinocchio-pubkey` crate has been removed from the pinocchio workspace. All functionality moved to `solana-address`:

- `pinocchio_pubkey::declare_id!` → `solana_address::declare_id!`
- `pinocchio_pubkey::pubkey!` → `solana_address::address!`
- PDA functions now on `Address` type methods

### Impact

- `pina_sdk_ids` crate: Replace all `pinocchio_pubkey::declare_id!` with `solana_address::declare_id!`
- `pina` crate: Replace `pub use pinocchio_pubkey::*` with re-exports from `solana-address` or `pinocchio::address`
- Remove `pinocchio-pubkey` from workspace dependencies

---

## R3: pinocchio-log Replacement

### Decision

Replace `pinocchio-log` with `solana-program-log` (version ^1.1).

### Rationale

The `pinocchio-log` crate functionality has moved to `solana-program-log` in the Solana SDK. The new crate provides the same `log!` macro, `Logger` type, and `log_cu_usage` attribute macro.

### Migration

- `pinocchio_log::log!` → `solana_program_log::log!`
- `pinocchio_log::Logger` → `solana_program_log::Logger`
- `pinocchio_log::log_cu_usage` → `solana_program_log::log_cu_usage`
- Pina's `log!` macro wrapper will need to forward to the new crate

---

## R4: Helper Crate Upgrades

### Decision

Upgrade all helper crates to their latest versions.

| Crate                              | Current | Target | Breaking? |
| ---------------------------------- | ------- | ------ | --------- |
| pinocchio-token                    | ^0.4    | ^0.5   | Yes       |
| pinocchio-system                   | ^0.3    | ^0.5   | Yes       |
| pinocchio-associated-token-account | ^0.2    | ^0.3   | Yes       |
| pinocchio-memo                     | ^0.2    | ^0.3   | Yes       |
| pinocchio-token-2022               | ^0.1    | ^0.2   | Yes       |

### Common Breaking Changes Across All Helper Crates

All helper crates underwent the same type migration:

- `AccountInfo` → `AccountView`
- `Pubkey` → `Address`
- `key()` → `address()`
- `from_account_info()` → `from_account_view()`
- CPI types from `solana-instruction-view`
- `Signer` from `solana_instruction_view::cpi::Signer`

### New Additions

- **pinocchio-token 0.5**: `Multisig` state, `InitializeMultisig`/ `InitializeMultisig2` instructions
- **pinocchio-system 0.5**: `create_account_with_minimum_balance()` helper, rent sysvar made optional
- **pinocchio-token-2022 0.2**: Extension instruction modules (default_account_state, group_member_pointer, group_pointer, etc.)

---

## R5: New Workspace Dependencies

### Decision

Add these new workspace dependencies:

```toml
solana-address = { default-features = false, version = "^2.0",
  features = ["decode"] }
solana-program-log = { default-features = false, version = "^1.1" }
```

### Rationale

These crates replace `pinocchio-pubkey` and `pinocchio-log` respectively. They are the canonical Solana SDK crates for these functionalities. The `pinocchio` crate itself re-exports types from `solana-account-view`, `solana-address`, and `solana-program-error`, so pina can re-export via pinocchio rather than depending on these crates directly where possible.

### Alternatives Considered

- **Depend on solana-account-view, solana-program-error directly**: Rejected. Pinocchio re-exports these, so pina should go through pinocchio to maintain a single dependency chain.
- **Keep pinocchio-pubkey**: Not possible; crate removed from workspace and will not receive updates.

---

## R6: pina_token_2022_extensions Removal

### Decision

Fully remove the `pina_token_2022_extensions` crate.

### Rationale

- Crate is explicitly marked "slated for deprecation" in CLAUDE.md
- Uses `#![allow(unsafe_code)]` which violates Constitution Principle II
- Upstream pinocchio-token-2022 is actively adding extension support (extension instructions merged, TLV state parsing in active PR)
- Migrating this crate to 0.10.x types would be substantial effort for a crate that will be replaced
- The crate has never been widely adopted externally

### Files to Remove

- `crates/pina_token_2022_extensions/` (entire directory)

### References to Remove

- `Cargo.toml`: workspace member, workspace dependency
- `knope.toml`: package entry, scopes, changelog
- `CLAUDE.md`: crate description
- `readme.md`: crate table entry
- `.specify/memory/constitution.md`: package scopes list

---

## R7: Pina Public API Migration Strategy

### Decision

Perform a clean break - rename all public types to match upstream. Do NOT provide backward-compatibility aliases.

### Rationale

- This is already a major version bump (0.2.0 → 1.0.0)
- Type aliases would add confusion about which name is canonical
- Downstream users need to update regardless due to pinocchio changes
- Clean API is more maintainable long-term

### Public API Changes in pina

| Current Re-export                                                          | New Re-export                                                             |
| -------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| `pub use pinocchio::account_info::AccountInfo`                             | `pub use pinocchio::AccountView`                                          |
| `pub use pinocchio::entrypoint`                                            | `pub use pinocchio::entrypoint` (unchanged)                               |
| `pub use pinocchio::program_entrypoint`                                    | `pub use pinocchio::program_entrypoint` (unchanged)                       |
| `pub use pinocchio::instruction::{AccountMeta, Instruction, Seed, Signer}` | `pub use pinocchio::instruction::*` (behind `cpi` feature)                |
| `pub use pinocchio::program_error::ProgramError`                           | `pub use pinocchio::ProgramResult` + `solana_program_error::ProgramError` |
| `pub use pinocchio::pubkey::Pubkey`                                        | `pub use pinocchio::Address`                                              |
| `pub use pinocchio::sysvars`                                               | `pub use pinocchio::sysvars` (unchanged)                                  |
| `pub use pinocchio_pubkey::*`                                              | `pub use solana_address::*` or `pub use pinocchio::address::*`            |
| `pub use pinocchio_system as system`                                       | `pub use pinocchio_system as system` (unchanged)                          |
| `pub use pinocchio_log::{Logger, log_cu_usage}`                            | `pub use solana_program_log::{Logger, log_cu_usage}`                      |
| `pub use pinocchio_token as token`                                         | `pub use pinocchio_token as token` (unchanged)                            |
| `pub use pinocchio_token_2022 as token_2022`                               | `pub use pinocchio_token_2022 as token_2022` (unchanged)                  |

### Trait Signature Changes

All traits using `AccountInfo` must use `AccountView`:

- `TryFromAccountInfos::try_from_account_infos(&'a [AccountView])`
- `AccountInfoValidation` → rename to `AccountViewValidation`
- `AsAccount::as_account` implementations on `AccountView`
- `LamportTransfer` implementations on `AccountView`
- `CloseAccountWithRecipient` implementations on `AccountView`

All traits using `Pubkey` must use `Address`:

- `AccountInfoValidation::assert_owner(&Address)`
- `AccountInfoValidation::assert_address(&Address)`
- `AsAccount::as_account(program_id: &Address)`
- `HasDiscriminator` (no Pubkey usage, unaffected)

### Generated Code Changes (pina_macros)

The proc macros generate code using `::pina::` paths. Since pina re-exports the new types, the macros themselves don't need changes to import paths. However, the generated type references must update:

- `::pina::AccountInfo` → `::pina::AccountView`
- `::pina::ProgramError` → stays `::pina::ProgramError` (re-exported)
- `::pina::TryFromAccountInfos` → rename trait

---

## R8: nostd_entrypoint! Macro Update

### Decision

Update the `nostd_entrypoint!` macro to use the new pinocchio 0.10.x entrypoint types.

### Current Implementation

```rust
macro_rules! nostd_entrypoint {
	($process_instruction:ident) => {
		pinocchio::program_entrypoint!($process_instruction);
		pinocchio::no_allocator!();
		pinocchio::nostd_panic_handler!();
	};
	($process_instruction:ident, $max_accounts:expr) => {
		pinocchio::program_entrypoint!($process_instruction, $max_accounts);
		pinocchio::no_allocator!();
		pinocchio::nostd_panic_handler!();
	};
}
```

### Migration

The macro body should remain structurally the same. The change is in the expected function signature:

**Old:**

```rust
fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult
```

**New:**

```rust
fn process_instruction(
    program_id: &Address,
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult
```

The `pinocchio::program_entrypoint!` macro handles the signature enforcement internally, so pina's wrapper macro needs no structural changes — only the documentation and downstream usage examples update.

---

## R9: Escrow Example Migration

### Decision

Update the escrow example to use the new pinocchio 0.10.x types.

### Impact Analysis (escrow_program/src/lib.rs)

- `Pubkey` → `Address` (used in EscrowState fields, function params)
- `PodU64` → unchanged (pina Pod types)
- `AccountInfo` → `AccountView` (used in account struct references)
- `token_2022::instructions::TransferChecked` → updated API
- `associated_token_account::instructions::Create` → updated API
- `token_2022::instructions::CloseAccount` → updated API
- `pinocchio_token::ID` / `pinocchio_token_2022::ID` → now `Address`
- All validation chain methods → unchanged names, but operate on `AccountView` now

### Specific Changes

- `EscrowState.maker: Pubkey` → `EscrowState.maker: Address`
- `EscrowState.mint_a: Pubkey` → `EscrowState.mint_a: Address`
- `EscrowState.mint_b: Pubkey` → `EscrowState.mint_b: Address`
- Account struct field types: `&'a AccountInfo` → `&'a AccountView`
- `SPL_PROGRAM_IDS: [Pubkey; 2]` → `SPL_PROGRAM_IDS: [Address; 2]`
