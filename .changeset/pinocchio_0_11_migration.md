---
default: major
---

Upgrade the workspace to Pinocchio 0.11 and migrate Pina's core account APIs to the new mutable `AccountView` model.

Breaking changes include:

- entrypoints, `TryFromAccountInfos`, and downstream account parsing now use `&mut [AccountView]`
- `ProcessAccountInfos::process` now consumes `self`
- `AsAccount::as_account` and `as_account_mut` now return guard-backed `Ref` / `RefMut` values instead of bare references
- `#[derive(Accounts)]` now supports mutable account refs and slices, and writable IDL inference now follows mutable fields
- close helpers no longer implicitly zero or resize account data; callers can keep the explicit `zeroed()` flow or use the new `close_account_zeroed()` helper when stale bytes must be cleared before close

This release also upgrades the Pinocchio companion crates, adds the standalone `memo` and `account-resize` features, preserves token account compatibility aliases, refreshes docs/examples/security guidance for the new borrow model, and regenerates the affected Codama IDLs and generated clients.
