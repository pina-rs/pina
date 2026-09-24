---
pina_lints: fix
pina_cli: docs
---

# Require full-balance drain guards to behave like guards

`require_guarded_full_balance_drain` no longer accepts a drain because a guard-named call appears before it. A call now counts as the guard only when its failure stops the handler before the drain (`?`, `unwrap()`/`expect()`, or an `if`, `match`, or `let ... else` that returns on failure) and it reads the state it checks through a receiver or an argument that refers to a local binding. A discarded `let _ = state.check_limits();`, a guard that cannot fail, and a zero-argument or literal-only `check_limits()?` are flagged.

The pause/cap name remains the signal for what kind of check a call performs, because behavior alone cannot tell a withdrawal cap from `assert_signer()?`. A differently named local wrapper such as `enforce_withdrawal_policy(&config)?` is now accepted when its own body enforces such a guard on every path. Free-function guards such as `assert_within_cap(remaining)?` are recognized too; before this change only method-call guards were.
