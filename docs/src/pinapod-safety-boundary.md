# PinaPod integration and safety boundary

## Architecture

Pina uses PinaPod's native-schema model:

```rust
#[account(discriminator = ProfileAccountType)]
pub struct ProfileState {
	pub bump: u8,
	pub name: pina::String<32>,
	pub tags: pina::Vec<u64, 8>,
	pub active: bool,
	pub note: Option<String<64>>,
}
```

The source struct is a native schema. `PinaPod` generates `ProfileStateZc`, whose fields use alignment-one storage representations. Pina's account loaders validate the runtime byte slice and return a borrow of that generated view. Callers use the generated field accessors rather than treating the native struct as account memory.

## Why there is no `to_bytes()`

Pina accepts bytes through a validation boundary:

```text
initialized runtime bytes -> PinaPod validation -> borrowed TypeZc view
```

It does not expose this operation:

```text
schema or TypeZc object representation -> borrowed byte slice
```

The first direction validates external storage. The second would make Rust object representation part of Pina's API and would weaken future layout changes.

## Initialization and clearing

Fixed account, instruction, and event schemas have an `initialize` helper. The caller supplies an exact mutable byte slice. PinaPod zeros the slice, lets the caller set the generated view, validates the finished value, and zeros the slice again if initialization fails.

```rust,ignore
let mut storage = vec![0u8; ProfileState::SIZE];
let profile = ProfileState::initialize(&mut storage, |profile| {
	profile.bump = bump;
	profile.name.try_set("alice")?;
	profile.tags.try_set([1, 2, 3])?;
	profile.active.set(true);
	Ok(())
})?;
```

Fixed containers initialize their complete capacity. Shortening or clearing a string, vector, or option zeros the removed payload. Compact patches also clear bytes removed from a tail before Pina releases or shrinks the backing account storage.

### Fixed account creation

`CreateProgramAccount` and `CreateProgramAccountWithBump` expose the same initialization boundary:

```rust,ignore
CreateProgramAccountWithBump {
	account,
	payer,
	owner: &ID,
	seeds,
	bump,
}
.invoke_with::<ProfileState>(|profile| {
	profile.bump = bump;
	profile.name.try_set("alice")?;
	profile.tags.try_set([1, 2, 3])?;
	profile.active.set(true);
	Ok(())
})?;
```

`invoke` and `invoke_signed` use an empty initializer. They succeed only when the discriminator plus an otherwise all-zero representation is valid. `invoke_with` and `invoke_signed_with` configure the generated view before final validation, so callers can establish required nonzero values without exposing a partially initialized typed account between creation and mutation.

The initializer returns `Result<(), PinaPodError>`. PinaPod clears the complete destination before the closure, Pina writes the discriminator, and PinaPod validates the finished value once. A closure or validation error clears the bytes again and becomes `ProgramError::InvalidAccountData`. In an on-chain instruction, returning that error also causes Solana to roll back the account-creation CPI with the rest of the transaction.

Advanced manual `PinaAccount` implementations can contain a storage enum whose valid discriminants start above zero. Such a type must use `invoke_with` and set the enum before final validation. Pina's closed `#[account]` grammar does not accept arbitrary custom enum fields, so this is not an extension of the audited macro grammar.

### Fixed PDA loading

For a fixed account with a stored PDA bump, `Type::load_pda` and `Type::load_pda_mut` validate the owner, size, discriminator, active nested values, and derived account address before exposing a guard. The mutable form also requires writability. This is one validation boundary: it does not call `assert_type`, generated `assert_seeds`, and `as_account_mut` as three separate passes.

The loaded guard still owns the runtime account-data borrow. End its scope or call `drop` before a CPI that may access the same account. Pina's `deny_account_borrows_across_cpi` lint recognizes guards returned by `load_pda_mut`.

### Compact PDA loading

For a compact account with a stored PDA bump, `Type::with_pda` validates the owner, discriminator, size, every active tail, canonical bump, and derived account address before it runs the closure. The runtime borrow guard remains active for the closure and is released when the closure returns.

Use `Type::with_pda` when the handler needs compact data. Do not call `assert_compact_type`, generated `assert_seeds`, and `with_compact_account` first. Those calls repeat compact parsing and PDA derivation. Keep `assert_compact_type` or generated `assert_seeds` for validation-only code.

### Compact account creation

Compact creation does not use an initializer closure. `CreateCompactProgramAccount` and `CreateCompactProgramAccountWithBump` require a generated `patch` field:

```rust,ignore
CreateCompactProgramAccountWithBump {
	account,
	payer,
	owner: &ID,
	seeds,
	bump,
	patch: JournalPatch::new().bump(bump),
	space: Journal::HEADER_SIZE,
}
.invoke::<Journal>()?;
```

The patch is the typed initialization plan. The builder validates `space`, allocates the account, applies the patch, and writes the discriminator. Pass `JournalPatch::new()` for an all-zero, empty-tail default, but do not omit the `patch` field.

Generated clients allocate zeroed instruction buffers, configure private generated views, validate them, and move the buffers into Solana instructions. They do not expose a general-purpose `to_bytes()` method.

## Ownership of unsafe code

PinaPod owns its unsafe traits and byte-to-view pointer conversions. Pina's schema macros derive `PinaPod`; Pina does not provide a second generic cast.

Pina is responsible for:

- exact fixed length or valid compact bounds;
- the expected discriminator;
- the runtime borrow-guard lifetime;
- owner, signer, writable, and PDA validation;
- ending a data borrow before a runtime resize;
- changing rent and bytes only after compact patch preflight succeeds.

`PinaPodFixed` is an unsafe trait. A manual implementation must uphold the complete PinaPod contract for every input byte slice.

## Verification requirements

The integration keeps regression coverage at each boundary:

- compile-time tests pin the generated schema, view, and patch APIs;
- negative tests reject invalid booleans, enum values, UTF-8, prefixes, tags, capacities, and nested active elements;
- fixed-create tests prove that an invalid all-zero default fails, `invoke_with` can establish a required nonzero value, and failed initialization clears the destination;
- account loader tests keep runtime borrow guards alive;
- Miri exercises initialization, aliasing, inactive capacity, and removed safe accessors;
- generated-client tests assert exact bytes and reject over-capacity input;
- IDL drift tests compare Rust, TypeScript, and Dart layouts;
- SBF tests exercise account creation, growth, shrink, rent adjustment, and failed-update rollback.

Pina validates and borrows initialized external bytes. It never treats an arbitrary in-memory schema value as raw account bytes.
