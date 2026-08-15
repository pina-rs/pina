---
pina: major
---

Enforce writability at account parse time: `AccountsCursor::next_mut` now validates that the account is marked writable in the instruction before returning a `&mut AccountView`, and `remaining_mut` validates every trailing account. A `&mut AccountView` (or `&mut [AccountView]`) field is now the single source of truth for writable accounts — the separate `assert_writable()` call is no longer required for mutable fields.

## Migration guide

- **Remove redundant `assert_writable()` calls.** Any `assert_writable()` invoked on a field declared as `&'a mut AccountView` (or inside a `&'a mut [AccountView]` remaining slice) is now redundant and can be deleted. The check happens once, during `try_from`/`try_from_account_infos`, before any instruction processing.
- **Keep `assert_writable()` on immutable fields.** Accounts declared as `&'a AccountView` that must be writable (for example, CPI targets the program never mutates directly) still require an explicit `assert_writable()` call.
- **`remaining_mut` now returns `Result`.** `AccountsCursor::remaining_mut` changed from `&'a mut [AccountView]` to `Result<&'a mut [AccountView], ProgramError>` and rejects the call when any remaining account is not writable. The `#[derive(Accounts)]` expansion was updated accordingly; manual cursor users must add `?`.
- **Behavior change.** Programs that previously declared `&mut` fields without asserting writability will now fail with `ProgramError::InvalidAccountData` when a non-writable account is passed. This is the intended fix: a mutable view of a non-writable account is never legitimate.
