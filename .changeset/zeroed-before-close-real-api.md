---
pina_lints: breaking
pina: docs
pina_cli: docs
---

# Fix zeroed-before-close help text and aliasing

The lint told users to call `account.zeroed()?` before closing, but no `zeroed()` method exists in Pina or upstream, so following the lint's own advice produced a compile error. Its only accepted proof was a call to that nonexistent method on a receiver with the same text, so zeroing through an alias (`let a = &mut state; a.zeroed()?; state.close()?`) was flagged even though it names the same account, while unrelated receivers that collapse to the same text were accepted (#515).

The help text now points at the zeroing APIs Pina actually ships: `close_account_zeroed(&ID, recipient)` or the `CloseAccountZeroed` builder, which zero and close in one step and are never flagged, or — when the close must stay separate — `account.try_borrow_mut()?.fill(0);` before `close_with_recipient()` or `close()`. That separate step is accepted only as a `core` slice `fill(0)` over the whole buffer returned by `try_borrow_mut()?`, chained or through a `let` binding; partial fills, non-zero fills, and same-named methods are not proofs. No runtime API was added and no compute units change.

"Same account" is now decided by resolving both receivers to a local binding plus field path instead of comparing text. A `let` alias is followed only when its initializer is a plain place (`x`, `&mut x`, `&mut *x`, `*x`, `x.field`); a binding initialized by a call is its own account. Receivers reached through indexing or a method or function call, and bindings that are assigned, lent as a slot, or captured by a closure, have no identity. Mutably lending the account's place or anything it is reached through before the close voids the proof: a `&mut` borrow, a `ref mut` or default-binding pattern, a `&mut` argument, a `&mut self` method outside the account crates, or a closure capturing it mutably. Lending a sibling field does not. A lend or data borrow the lint cannot place is assumed to reach every account. The zeroing must also run on every path to the close, including past any `break` out of a loop or labeled block and outside a `let ... else` block, and stay the last write before it. Fully qualified closes (`AccountView::close(state)`) are checked like method calls. Every close the lint accepts is therefore preceded by a recognized zero fill of the same place, and every close `main` accepted after a `zeroed()` call is now flagged. Writes after the fill through trusted `pina`/`pinocchio`/`solana_account_view` methods (such as `as_account_mut()`), through a CPI, or through a separately obtained handle to the same account are documented limits.

This is a breaking change for a deny-by-default lint. Code that passed by calling a user-defined `zeroed()` before `close()` or `close_with_recipient()` now fails. So do the other closes the tightened rules no longer prove, such as a zeroing in one branch, a lend of the account between the zeroing and the close, or a close through an indexed receiver. Close with `close_account_zeroed()`, or clear the whole buffer with `account.try_borrow_mut()?.fill(0);` immediately before the close.

A user function that only shares a close name (`fn close(..)`) is still checked as a close, but it is not trusted: a `&mut` it receives is a lend to every other close.

The `CloseAccountWithRecipient` docs, the token-escrow tutorial, the close guidance, the `pina docs` overview, and `pina lint --explain require_zeroed_before_close` no longer mention `zeroed()`.
