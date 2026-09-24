---
pina_lints: fix
pina: docs
pina_cli: docs
---

# Fix zeroed-before-close help text and aliasing

The lint told users to call `account.zeroed()?` before closing, but no `zeroed()` method exists in Pina or upstream, so following the lint's own advice produced a compile error. Its only accepted proof was a call to that nonexistent method on a receiver with the same text, so zeroing through an alias (`let a = &mut state; a.zeroed()?; state.close()?`) was flagged even though it names the same account, while unrelated receivers that collapse to the same text were accepted (#515).

The help text now points at the zeroing APIs Pina actually ships: `close_account_zeroed(&ID, recipient)` or the `CloseAccountZeroed` builder, which zero and close in one step and are never flagged, or — when the close must stay separate — `account.try_borrow_mut()?.fill(0);` before `close_with_recipient()` or `close()`. That separate step is accepted only as a `core` slice `fill(0)` over the whole buffer returned by `try_borrow_mut()?`, chained or through a `let` binding; partial fills, non-zero fills, and same-named methods are not proofs. No runtime API was added and no compute units change.

"Same account" is now decided by resolving both receivers to a local binding plus field path instead of comparing text. A `let` alias is followed only when its initializer is a plain place (`x`, `&mut x`, `&mut *x`, `*x`, `x.field`); a binding initialized by a call is its own account. Receivers reached through indexing or a method or function call, and bindings that are assigned, lent as a slot, or captured by a closure, have no identity. Mutably lending the account's place or anything it is reached through before the close (a `&mut` borrow, a `ref mut` or default-binding `match`, a `&mut` argument, or a `&mut self` method outside the account crates) voids the proof, while lending a sibling field does not. The zeroing must also run on every path to the close, including past any `break` out of a loop or labeled block, and stay the last write before it. Every close the lint accepts is therefore preceded by a recognized zero fill of the same place; every close `main` accepted after a `zeroed()` call is now flagged. Writes through trusted `pina`/`pinocchio`/`solana_account_view` methods (such as `as_account_mut()`) or a CPI after the fill are a documented limit.

The `CloseAccountWithRecipient` docs, the token-escrow tutorial, the close guidance, the `pina docs` overview, and `pina lint --explain require_zeroed_before_close` no longer mention `zeroed()`.
