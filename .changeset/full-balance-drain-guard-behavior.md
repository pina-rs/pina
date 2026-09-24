---
pina_lints: fix
pina_cli: docs
---

# Require full-balance drain guards to behave like guards

`require_guarded_full_balance_drain` no longer accepts a drain just because a guard-named call appears before it. A call now counts as the guard only when all of these hold:

- Its failure stops the handler. A `Result`/`Option` guard must be propagated with `?`, extracted with `unwrap()`/`expect()`, returned, or tested by a `match`, `if let`, or `let ... else` whose failing branches return `Err`/`None` or panic. A `bool` guard, or `is_err()`/`is_ok()` of a fallible one, must gate an `if` whose failing branch returns `Err`/`None` or panics; this includes `assert!`, `assert_eq!`, and `pina::assert(..)?`. Polarity is checked: a branch that returns `Ok` swallows the failure, so `if guard().is_ok() { return Ok(()) }`, `match guard() { Ok(()) => return Ok(()), Err(_) => {} }`, and `let Err(()) = guard() else { return Ok(()) }` no longer satisfy the lint. Failing branches are recognized by their shape, so a tail `return Err(..)` or `panic!(..)` counts even when its type was coerced to `()`.
- Its receiver or an argument is derived from a handler parameter. Zero-argument calls, literal-only calls, and literals routed through a local are rejected.
- Its name contains a pause or cap term, or it delegates to a named guard. A differently named local wrapper returning `Result`/`Option` counts when its body enforces a named guard in its outermost scope before any non-error `return`, up to three wrappers deep. `bool` wrappers are not followed.
- It can fail. A local callee that can only return a constant `Ok(..)`/`Some(..)` or `bool` literal is not a guard.

Trait calls are judged by the implementation that runs, never by a default body the implementation overrides. The right operand of `&&`/`||` is now treated as conditional, so `bypass || { guard()?; true }` no longer gates a later drain. Plain-function guards such as `assert_within_cap(remaining)?` are recognized alongside method-call guards.
