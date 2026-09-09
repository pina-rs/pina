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

## Refresh the managed lint driver

No manual cache cleanup is normally required. The CLI now keys the installed lint driver by the exact `rustc` commit and rebuilds it when the active compiler changes. If `PINA_LINT_DRIVER_PATH` is set for local lint development, make sure it points to a driver built by the same toolchain as the project.

Run the complete catalog after migrating:

```bash
pina lint
```

Review every lint-level override in `pina.toml`. Temporary `allow` entries can unblock an incremental migration, but restore deny-level security checks before deployment.
