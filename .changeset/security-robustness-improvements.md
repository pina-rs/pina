---
pina: minor
---

Security and robustness improvements from codebase audit:

**Critical fixes:**

- `discriminator_from_bytes` now returns `Err(ProgramError::InvalidAccountData)` instead of panicking when the input slice is shorter than the discriminator size. This prevents on-chain aborts from malformed instruction data.
- `matches_discriminator` now returns `false` instead of panicking on short input slices.
- `as_account` and `as_account_mut` now check `data_len() < size_of::<T>()` before creating a raw-parts slice, returning `ProgramError::AccountDataTooSmall` instead of reading out-of-bounds memory.
- `parse_instruction` now validates data length before calling `discriminator_from_bytes` for defense-in-depth.

**Security improvements:**

- `close_account` now zeroes account data via `resize(0)` before closing, matching the behavior of `close_with_recipient` and preventing stale data from being read by subsequent transactions.
- Added checked token cast methods: `as_checked_token_mint()`, `as_checked_token_account()`, `as_checked_token_2022_mint()`, `as_checked_token_2022_account()` that verify token program ownership before casting.
- Deprecated `find_program_address` in favor of `try_find_program_address` which returns `Option` instead of panicking on-chain.

**New error variants** (non-breaking, `#[non_exhaustive]` enum):

- `PinaProgramError::DataTooShort` — data shorter than expected minimum.
- `PinaProgramError::InvalidAccountSize` — account size mismatch.
- `PinaProgramError::InvalidTokenOwner` — account not owned by expected token program.
- `PinaProgramError::SeedsTooMany` — too many PDA seeds provided.

**New Pod types:**

- `PodI32` — alignment-safe `i32` wrapper for `#[repr(C)]` account structs.
- `PodI128` — alignment-safe `i128` wrapper for `#[repr(C)]` account structs.
