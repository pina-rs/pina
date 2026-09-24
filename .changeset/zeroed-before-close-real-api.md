---
pina_lints: fix
pina: docs
pina_cli: docs
---

# Fix zeroed-before-close help text and aliasing

The lint told users to call `account.zeroed()?` before closing, but no `zeroed()` method exists in Pina or upstream, so following the lint's own advice produced a compile error. Its only accepted proof was a call to that nonexistent method, matched by the receiver's text, so `let a = &mut state; a.zeroed()?; state.close()?` was a bypass (#515).

The help text now points at the zeroing APIs Pina actually ships: `close_account_zeroed(&ID, recipient)` or the `CloseAccountZeroed` builder, which zero and close in one step and are never flagged, or — when the close must stay separate — `account.try_borrow_mut()?.fill(0);` before `close_with_recipient()` or `close()`. That separate step is accepted only as a `core` slice `fill(0)` over the whole buffer returned by `try_borrow_mut()?`, chained or through a `let` binding; partial fills, non-zero fills, and same-named methods are not proofs. No runtime API was added and no compute units change.

Receivers now resolve through the shared fact collector's alias chains (`CallInfo` gains a `receiver_binding`), so zeroing or closing through an alias of the same account is recognized, zeroing a different account is still flagged, and a binding reassigned after its `let` never proves anything. The `CloseAccountWithRecipient` docs, the token-escrow tutorial, the close guidance, the `pina docs` overview, and `pina lint --explain require_zeroed_before_close` no longer mention `zeroed()`.
