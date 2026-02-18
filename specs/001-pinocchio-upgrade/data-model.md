# Data Model: Pinocchio Ecosystem Upgrade

**Date**: 2026-02-15 **Branch**: `001-pinocchio-upgrade`

## Entity Changes

This feature is a dependency upgrade and crate removal, not a new data model. The "entities" here are the type renames and API surface changes.

### E1: AccountView (replaces AccountInfo)

**Source**: `solana-account-view` crate, re-exported via `pinocchio` **Pina re-export**: `pina::AccountView`

**Unchanged methods**: `is_signer()`, `is_writable()`, `executable()`, `lamports()`, `owner()`, `data_len()`, `data_is_empty()`, `assign()`, `close()`

**Renamed methods**:

- `key()` → `address()` (returns `&Address`)
- `try_borrow_data()` → `try_borrow()` (returns borrowed data)
- `try_borrow_mut_data()` → `try_borrow_mut()` (returns mutable data)
- `realloc()` → `resize()` / `resize_unchecked()`

**Impact on pina traits**:

- `AccountInfoValidation` trait: all methods now impl on `AccountView`
- `AsAccount` trait: impl on `AccountView`
- `AsTokenAccount` trait: impl on `AccountView`
- `LamportTransfer` trait: impl on `AccountView`
- `CloseAccountWithRecipient` trait: impl on `AccountView`
- `TryFromAccountInfos` trait: `&'a [AccountView]` parameter

### E2: Address (replaces Pubkey)

**Source**: `solana-address` crate, re-exported via `pinocchio` **Pina re-export**: `pina::Address`

**Type**: `[u8; 32]` wrapper (same as Pubkey)

**Key methods**:

- `find_program_address(&[&[u8]], &Address) -> (Address, u8)`
- `try_find_program_address(&[&[u8]], &Address) -> Option<(Address, u8)>`
- `create_program_address(&[&[u8]], &Address) -> Result<Address, _>`
- `new_from_array([u8; 32]) -> Self`
- `to_bytes() -> [u8; 32]`
- `as_array() -> &[u8; 32]`

**Impact on pina traits**:

- All `&Pubkey` parameters become `&Address`
- `assert_owner(&Address)`, `assert_address(&Address)`, etc.
- `as_account(program_id: &Address)`

### E3: InstructionView (replaces Instruction)

**Source**: `solana-instruction-view`, behind pinocchio `cpi` feature **Pina re-export**: `pina::InstructionView` (via `pinocchio::instruction`)

**Companion type changes**:

- `AccountMeta` → `InstructionAccount`
- `Signer` → `solana_instruction_view::cpi::Signer`
- `Seed` → `solana_instruction_view::Seed`

**Impact on pina CPI helpers**:

- `cpi.rs` uses these types for account creation and allocation
- All `invoke_signed` calls use new path

### E4: Crate Removal — pina_token_2022_extensions

**Entities removed**: All 27 extension type structs, the `Extension` trait, TLV parsing utilities, and CPI instruction builders.

**No replacement needed**: Upstream pinocchio-token-2022 is adding equivalent functionality. Users should migrate to upstream once available.

## Dependency Graph (Post-Upgrade)

```text
pina
├── pinocchio ^0.10 (features: cpi)
├── solana-address ^2.0 (features: decode)
├── solana-program-log ^1.1 (optional, "logs" feature)
├── pinocchio-system ^0.5
├── pinocchio-token ^0.5 (optional, "token" feature)
├── pinocchio-token-2022 ^0.2 (optional, "token" feature)
├── pinocchio-associated-token-account ^0.3 (optional, "token" feature)
├── bytemuck ^1
├── paste ^1
├── typed-builder ^0.23
└── pina_macros ^1.0 (optional, "derive" feature)

pina_macros
├── darling ^0.21
├── heck ^0.5
├── proc-macro2 ^1
├── quote ^1
└── syn ^2

pina_sdk_ids
└── solana-address ^2.0 (features: decode)

escrow_program
└── pina (features: logs, token, derive)
```
