---
pina: major
---

Migrate to pinocchio 0.10.x with breaking type changes across the entire API surface:

- **`AccountInfo` → `AccountView`** — all trait signatures, implementations, and generated code now use `AccountView` from pinocchio 0.10.x.
- **`Pubkey` → `Address`** — the 32-byte public key type is now `Address` from `solana-address` (with `bytemuck` feature for `Pod`/`Zeroable`/`Copy` derives). All function parameters, struct fields, and return types updated.
- **`pinocchio-pubkey` → `solana-address`** — replaced the `pinocchio-pubkey` dependency with `solana-address ^2.0` for `declare_id!`, `address!`, and address constants.
- **`pinocchio-log` → `solana-program-log`** — replaced `pinocchio-log` with `solana-program-log ^1.1` for on-chain logging. The `log!` macro now uses `$crate`-based paths for cross-crate compatibility.
- **Method renames** — `key()` → `address()`, `try_borrow_data()` → `try_borrow()`, `try_borrow_mut_data()` → `try_borrow_mut()`, `realloc()` → `resize()`, `data_is_empty()` → `is_data_empty()`, `minimum_balance()` → `try_minimum_balance()`.
- **Module renames** — `pinocchio::pubkey` → `pinocchio::address`, `pinocchio::account_info` → `pinocchio::account`, `pinocchio::program_error` → `pinocchio::error`.
- **`owner()` is now unsafe** — `AccountView::owner()` requires an `unsafe` block as it reads from raw account memory. All validation methods updated accordingly.
- **PDA functions cfg-gated** — `try_find_program_address`, `find_program_address`, and `create_program_address` are now behind `#[cfg(target_os = "solana")]` with native stubs returning `None`/`Err`.
- **Dependency bumps** — pinocchio-system ^0.5, pinocchio-token ^0.5, pinocchio-token-2022 ^0.2, pinocchio-associated-token-account ^0.3, pinocchio-memo ^0.3.
