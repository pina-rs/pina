# Migrate to hardened security lints

This release adds a deny-by-default check for unchecked mutable remaining accounts and makes existing lint analysis follow more source-level control flow. A project that previously passed `pina lint` can therefore fail until it makes duplicate-account and compute bounds explicit.

## Reject duplicate mutable accounts by default

Replace direct `remaining_mut()` calls with the distinct loader:

```rust
let remaining = cursor.remaining_mut_distinct()?;
```

The lint covers method syntax, UFCS, stored function items, and calls produced by local or dependency macros. If duplicate mutable addresses are part of the protocol, prefer the typed escape hatch and explain the invariant where reviewers can see it:

```rust
#[derive(Accounts)]
struct WeightedAccounts<'a> {
	/// Duplicate entries intentionally apply the same account's weight more than once.
	#[pina(remaining, distinct = false)]
	positions: &'a mut [AccountView],
}
```

For a manual parser, contain `remaining_mut()` in the smallest reviewed helper and add a local lint allowance with a specific safety comment. Do not disable the lint for the crate.

## Re-establish bounds after mutation

The remaining-account bound check now follows local aliases. Reassignment, mutable borrowing, a mutable method receiver, a mutable-reference function argument, or a closure that can replace a checked binding invalidates the earlier proof. Move the guard after the last possible mutation:

```rust
replace_remaining(&mut remaining, replacement);

if remaining.len() > MAX_REMAINING_ACCOUNTS {
	return Err(ProgramError::InvalidArgument);
}

for account in remaining {
	process(account)?;
}
```

A constant `.take(MAX)` is also accepted, including when the bounded iterator is stored in a local variable. Use it only when silently ignoring surplus accounts is part of the instruction contract:

```rust
let bounded = remaining.iter().take(MAX_REMAINING_ACCOUNTS);
for account in bounded {
	process(account)?;
}
```

Adapters that cannot increase the number of items, such as `filter`, `map`, and `enumerate`, preserve the bound. Apply `take` after `flat_map`, `flatten`, `cycle`, or another adapter that can expand its input.

Every remaining-account source in a chained iterator needs its own dominating length check. The lint only accepts the built-in length of a slice or array as evidence; a custom method named `len` cannot establish a security bound.

## Review unused borrow-guard warnings

The unused-borrow-guard lint now classifies each binding's inferred type instead of matching loader names inside an expression. It therefore catches `Ref` and `RefMut` results returned through aliases or function pointers, including guards nested in tuple and `let ... else` patterns, while no longer treating an unrelated wrapper result as a guard merely because the wrapper consumed one. If code intentionally stores a guard, read through the binding; otherwise discard the loader result immediately with `?` so the runtime borrow ends at the statement boundary. Write `let _ = account.try_borrow()?;` at the creation site when an explicit discard reads better. Do not write `let _ = guard;` after binding a guard: Rust's wildcard pattern does not move that local, so the borrow remains live and the lint continues to warn.

## Refresh the managed lint driver

No manual cache cleanup is normally required. The CLI now keys the installed lint driver by the exact `rustc` commit and rebuilds it when the active compiler changes. It installs the driver as `RUSTC_WORKSPACE_WRAPPER`, so Cargo can retain an existing outer `RUSTC_WRAPPER` such as `sccache`. If `PINA_LINT_DRIVER_PATH` is set for local lint development, make sure it points to a driver built by the same toolchain as the project.

Run the complete catalog after migrating:

```bash
pina lint
```

Review every lint-level override in `pina.toml`. Temporary `allow` entries can unblock an incremental migration, but restore deny-level security checks before deployment.
