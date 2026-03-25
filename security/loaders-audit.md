# `crates/pina/src/loaders.rs` audit

_Audit date: 2026-03-23_

## Scope

This audit covers only:

- `crates/pina/src/loaders.rs`

Line numbers below refer to the current working tree at the time of writing.

## Executive summary

`crates/pina/src/loaders.rs` is mostly disciplined about validation, logging, and arithmetic checks. The main issue is **not** the presence of `unsafe` by itself. The real problem is that several loader APIs construct references from guard-backed account borrows and then return those references **after the temporary borrow guard has already been dropped**.

That pattern appears in:

- generic account loaders:
  - `crates/pina/src/loaders.rs:342-358`
  - `crates/pina/src/loaders.rs:362-382`
- token loaders:
  - `crates/pina/src/loaders.rs:469-476`
  - `crates/pina/src/loaders.rs:492-497`
  - `crates/pina/src/loaders.rs:513-518`
  - `crates/pina/src/loaders.rs:534-540`
  - `crates/pina/src/loaders.rs:559-574`

The short-lived `unsafe { self.owner() }` reads in `assert_owner` and `assert_owners` are much lower risk and are reasonable to keep for performance with the current upstream API shape.

## Findings

| ID | Severity | Status | Summary                                                                                |
| -- | -------- | ------ | -------------------------------------------------------------------------------------- |
| F1 | High     | Open   | `as_account` / `as_account_mut` return references that outlive temporary borrow guards |
| F2 | High     | Open   | Token loaders repeat the same escaped-borrow lifetime pattern                          |
| F3 | Low      | Accept | `unsafe { self.owner() }` is acceptable here as a short-lived runtime read             |
| F4 | Low      | Open   | Lamport mutation helpers rely on caller-enforced ownership preconditions               |

---

## F1. Escaped borrow lifetimes in generic account loaders

**Affected code**

- `crates/pina/src/loaders.rs:342-358` — `AsAccount::as_account`
- `crates/pina/src/loaders.rs:362-382` — `AsAccount::as_account_mut`

### Why this matters

Both functions borrow account data through runtime borrow guards:

- `self.try_borrow()?`
- `self.try_borrow_mut()?`

They then extract raw pointers from those temporary guards, rebuild slices with:

- `from_raw_parts(...)`
- `from_raw_parts_mut(...)`

and finally return `&T` or `&mut T`.

The problem is that the borrow guard is a temporary expression value. It is dropped before the returned reference stops being used. That means the returned reference is no longer tied to the runtime borrow lifetime that originally justified the pointer access.

### Why this is a real soundness issue

Once the guard is gone, nothing prevents later code from:

- taking a second mutable borrow of the same account,
- taking an immutable borrow while a mutable typed reference is still live,
- resizing or otherwise mutating the underlying account buffer through another path.

At that point the returned `&T` / `&mut T` can become aliased or stale, which is a Rust aliasing violation and therefore potential undefined behavior.

### Why the existing checks are not enough

These checks are still useful, but they do not solve the lifetime issue:

- `self.assert_owner(program_id)?`
- `self.assert_data_len(size_of::<T>())?`
- `T::try_from_bytes(...)`
- `T::try_from_bytes_mut(...)`

They validate ownership, size, and discriminator/layout expectations. They do **not** keep the underlying borrow guard alive for as long as the returned reference exists.

### Performance-aware recommendation

Keep the zero-copy model, but change the API shape so the borrow guard is retained.

A good long-term fix is to return guard-backed wrapper types, for example:

- `LoadedAccount<'a, T>`
- `LoadedAccountMut<'a, T>`
- token-specific guard-backed wrappers if needed

Those wrappers should:

- store the borrow guard,
- store or derive the typed pointer/reference,
- implement `Deref` / `DerefMut` as appropriate.

That preserves the performance benefit of zero-copy access while restoring sound lifetime coupling.

### Suggested priority

Highest priority item in this file.

---

## F2. Escaped borrow lifetimes in token loaders

**Affected code**

Unchecked loaders:

- `crates/pina/src/loaders.rs:469-476` — `as_token_mint`
- `crates/pina/src/loaders.rs:492-497` — `as_token_account`
- `crates/pina/src/loaders.rs:513-518` — `as_token_2022_mint`
- `crates/pina/src/loaders.rs:534-540` — `as_token_2022_account`
- `crates/pina/src/loaders.rs:559-574` — `as_associated_token_account`

Checked wrappers delegating into the same pattern:

- `crates/pina/src/loaders.rs:478-490` — `as_token_mint_checked*`
- `crates/pina/src/loaders.rs:499-511` — `as_token_account_checked*`
- `crates/pina/src/loaders.rs:520-532` — `as_token_2022_mint_checked*`
- `crates/pina/src/loaders.rs:543-557` — `as_token_2022_account_checked*`
- `crates/pina/src/loaders.rs:576-585` — `as_associated_token_account_checked`

### Why this matters

These functions call `self.check_borrow()?` and then use unchecked token deserializers like:

- `crate::token::state::Mint::from_account_view_unchecked(self)`
- `crate::token::state::TokenAccount::from_account_view_unchecked(self)`
- `crate::token_2022::state::Mint::from_account_view_unchecked(self)`
- `crate::token_2022::state::TokenAccount::from_account_view_unchecked(self)`

`check_borrow()` only proves that the account is currently borrowable at the instant of the check. It does **not** keep a borrow guard alive for the lifetime of the returned reference.

So these functions appear to have the same structural issue as F1: they return plain references whose validity depends on a borrow discipline that is no longer actively enforced after return.

### Ownership checking is not the core issue here

Some wrappers correctly add ownership or ATA-address checks first:

- `assert_owners(...)`
- `assert_owner(token_program)?`
- `assert_associated_token_address(...)`

Those checks are good and should stay. But they address authorization and account identity, not the escaped-borrow lifetime problem.

### Performance-aware recommendation

Use the same fix direction as F1:

- return guard-backed token wrappers instead of bare `&Mint` / `&TokenAccount`
- keep the runtime borrow object alive inside the wrapper
- keep owner/address validation in the checked variants

### Suggested priority

Same class of issue as F1. Fix in the same redesign.

---

## F3. `unsafe { self.owner() }` is acceptable here

**Affected code**

- `crates/pina/src/loaders.rs:151-169` — `assert_owner`
- `crates/pina/src/loaders.rs:172-186` — `assert_owners`

### Assessment

This `unsafe` is short-lived, localized, and used only to read the account owner for immediate comparison and logging.

Given the current `pinocchio 0.10.x` API shape, this looks like a reasonable performance tradeoff:

- no reference escapes beyond the function,
- no aliasing is created,
- the runtime guarantees the account backing memory for the duration of instruction execution.

### Recommendation

Keep this code unless:

- upstream exposes an equally cheap safe accessor, or
- a concrete bug is found in the raw owner read path.

This is **not** the part of `loaders.rs` I would prioritize changing.

---

## F4. Lamport mutation helpers rely on caller-enforced ownership preconditions

**Affected code**

- `crates/pina/src/loaders.rs:614-645` — `LamportTransfer::send`
- `crates/pina/src/loaders.rs:662-680` — `CloseAccountWithRecipient::close_with_recipient`

### Assessment

These helpers do several good things:

- require writable accounts,
- reject self-send / self-close,
- use checked arithmetic,
- log before returning failures.

However, they do not themselves enforce the full set of safety/security preconditions implied by direct lamport and account-state mutation.

Examples:

- `send` checks writability, but not that the debited account is owned by the executing program.
- `close_with_recipient` checks writability, but also relies on the broader runtime/account model for correctness.

This is not the same class of issue as F1/F2. It is more of a public API footgun: the helpers are safe only if callers have already established the expected ownership and account-role invariants.

### Recommendation

At minimum:

- keep the doc comments explicit about caller preconditions,
- consider adding checked variants that also assert owner/program expectations,
- consider a custom lint for this misuse pattern.

### Suggested priority

Lower than F1/F2.

---

## Areas that look good

### Validation helpers are generally straightforward and correct

These functions are simple, defensive, and log failures clearly:

- `assert_signer` — `crates/pina/src/loaders.rs:30-42`
- `assert_writable` — `crates/pina/src/loaders.rs:45-57`
- `assert_executable` — `crates/pina/src/loaders.rs:60-69`
- `assert_data_len` — `crates/pina/src/loaders.rs:72-84`
- `assert_empty` — `crates/pina/src/loaders.rs:87-96`
- `assert_not_empty` — `crates/pina/src/loaders.rs:99-108`
- `assert_program` — `crates/pina/src/loaders.rs:111-113`
- `assert_sysvar` — `crates/pina/src/loaders.rs:146-148`
- PDA helpers — `crates/pina/src/loaders.rs:217-337`

### `assert_type` keeps the borrow scoped correctly

- `crates/pina/src/loaders.rs:115-143`

Unlike the loader functions, `assert_type` keeps the borrowed data in a local binding and does not return a typed reference derived from it. That structure is much less concerning.

### Arithmetic helpers are well-contained and tested

- `checked_send_balances` — `crates/pina/src/loaders.rs:588-600`
- `checked_close_balance` — `crates/pina/src/loaders.rs:603-607`
- tests — `crates/pina/src/loaders.rs:684+`

These functions use checked arithmetic and have targeted unit tests for edge cases.

## Recommended follow-up plan

1. **Redesign the loader APIs** to return guard-backed wrapper types instead of bare references.
2. Apply the same design to token and token-2022 loaders.
3. Add regression tests around overlapping borrow attempts once the guard-backed API exists.
4. Optionally add stricter checked lamport-helper variants or a custom lint for program-owned lamport mutation preconditions.

## Suggested future security lints

These would complement the existing Solana-focused lints well:

- `require_program_owned_before_lamport_mutation`
- `require_canonical_bump_before_pda_write`
- `require_sysvar_check_before_sysvar_use`
- `require_close_zeroization_before_close`
- `deny_duplicate_mut_loader_calls`

## Bottom line

- **Keep** the short-lived `unsafe` owner reads.
- **Prioritize fixing** the loader APIs that return references after borrow guards are dropped.
- **Preserve performance** by using guard-backed zero-copy wrappers rather than replacing everything with copied account state.
