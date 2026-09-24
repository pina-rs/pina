---
pina_lints: fix
pina_cli: docs
---

# Require full-balance drain guards to behave like guards

`require_guarded_full_balance_drain` no longer accepts a drain just because a guard-named call appears before it. A call now counts as the guard only when all of these hold:

- Its failure stops the handler. A `Result`/`Option` guard must be propagated with `?`, extracted with `unwrap()`/`expect()`, returned, or tested by a `match`, `if let`, or `let ... else`. Every arm that can receive the failure must return `Err`/`None`, return the scrutinee's own binding, or panic. Arms are read in order, so `_` after an unguarded `Err(_)` arm only sees success. Failure-preserving adapters are followed: `map_err`, `inspect_err`, `map`, `and_then`, `and`, `ok`, `ok_or`, `or(Err(..))`, a failing `or_else`, `clone()`, and `into()`/`From::from` into a `Result`. A conversion into `Option<Result<..>>` hides the failure and does not count. `guard() == Ok(..)` and `!=` are read with their polarity, and a guard-named unit method that panics on failure (an `assert!`-style guard) counts. Failing branches are recognized by their shape, so a tail `return Err(..)`, a `panic!`, `assert!`, or `return reject()` (a local helper that can only fail) counts.
- Polarity is checked. `if guard().is_ok() { return Ok(()) }`, `match guard() { Ok(()) => return Ok(()), Err(_) => {} }`, `let Err(()) = guard() else { return Ok(()) }`, and `assert_eq!(guard().is_err(), true)` no longer satisfy the lint. A branch that returns `Ok` is never a failure, including `if state.is_paused() { return Ok(()) }` in a handler.
- Its receiver or an argument is derived from a handler parameter. Zero-argument calls, literal-only calls, and literals routed through a local are rejected.
- Its name contains a pause or cap term, or it delegates to a named guard. A differently named local wrapper returning `Result`/`Option` counts when its body enforces a named guard in its outermost scope before any early success `return` or labeled `break`, up to three wrappers deep. Closures, fn pointers, generic callables, and `bool` wrappers never count. A generic wrapper is judged by the concrete impl its caller instantiates it with. Inside a wrapper, an unresolvable trait call or a `return` of a value the lint cannot see into is treated as unknown, never as a guard or a failure.
- It is not a constant success. A local callee that can only return a literal `Ok(..)`/`Some(..)` or `bool` literal, even through a `let` binding or behind an `if false` error branch, is not a guard.

Trait calls are judged by the implementation that runs, never by a default body the implementation overrides. The right operand of `&&`/`||` is now treated as conditional, so `bypass || { guard()?; true }` no longer gates a later drain. Plain-function guards such as `assert_within_cap(remaining)?` are recognized alongside method-call guards.
