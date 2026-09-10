# Migrate to internal creation emptiness checks

Pina's typed creation builders now enforce emptiness themselves and return `AccountAlreadyInitialized` when the target storage holds any nonzero byte. The deny-by-default `require_empty_before_init` lint is retired.

## Remove the lint from `pina.toml`

Delete the `[lints]` entry; the CLI now rejects the unknown name.

```toml
[lints]
# remove:
# require_empty_before_init = "deny"
```

## Remove manual pre-creation assertions

Delete `assert_empty()` calls that exist only immediately before a typed creation builder. Keep `assert_empty()` calls that guard other operations, such as raw `CreateAccount` allocations or token-account creation, where the typed builders are not involved.

```rust
// before
self.state.assert_empty()?;
CreateProgramAccountWithBump { account: self.state, /* .. */ }.invoke_with::<State>(/* .. */)?;

// after
CreateProgramAccountWithBump { account: self.state, /* .. */ }.invoke_with::<State>(/* .. */)?;
```

Error behavior is preserved: the builders return the same `AccountAlreadyInitialized` error the manual assertions produced.
