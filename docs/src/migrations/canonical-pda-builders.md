# Migrate to canonical PDA builders

Pina's typed PDA creation builders now own canonical bump validation. Remove `assert_canonical_bump` and `assert_seeds_with_bump` when they occur immediately before `CreateProgramAccount`, `CreateProgramAccountWithBump`, `CreateCompactProgramAccount`, or `CreateCompactProgramAccountWithBump`. All four validate the PDA: the explicit-bump builders reject a noncanonical supplied bump, and the bump-free builders derive and sign with the canonical one.

Keep `assert_empty` before creation. It preserves the distinct `AccountAlreadyInitialized` error and remains part of Pina's initialization lint contract.

## Migrate a supplied bump

Previously, a handler derived the PDA three times: once to find the canonical bump, once to validate the explicit bump, and once inside the creation builder.

```rust
let canonical_bump = account.assert_canonical_bump(&seeds.as_slices(), &ID)?;
if canonical_bump != args.bump {
	return Err(ProgramError::InvalidSeeds);
}

let seeds_with_bump = seeds.with_bump(args.bump);
account.assert_empty()?;
account.assert_seeds_with_bump(&seeds_with_bump.as_slices(), &ID)?;

CreateProgramAccountWithBump {
	account,
	payer,
	owner: &ID,
	seeds: &seeds.as_slices(),
	bump: args.bump,
}
.invoke_with::<State>(|state| {
	state.bump = args.bump;
	Ok(())
})?;
```

Now the builder performs one canonical search, verifies both the account address and supplied bump, and reuses that result for the PDA signer:

```rust
account.assert_empty()?;

CreateProgramAccountWithBump {
	account,
	payer,
	owner: &ID,
	seeds: &seeds.as_slices(),
	bump: args.bump,
}
.invoke_with::<State>(|state| {
	state.bump = args.bump;
	Ok(())
})?;
```

The explicit-bump builder now rejects a valid but noncanonical PDA with `ProgramError::InvalidSeeds`. This is an intentional security change.

## Derive and store the bump

If the instruction does not need to carry a bump, let the canonical builder derive it and pass it into the initializer:

```rust
let (_address, bump) = CreateProgramAccount {
	account,
	payer,
	owner: &ID,
	seeds: &seeds.as_slices(),
}
.invoke_with_bump::<State>(|state, bump| {
	state.bump = bump;
	Ok(())
})?;
```

`invoke_signed_with_bump` provides the same initializer and also accepts signer seeds for a PDA payer.

## Migrate compact creation

`CreateCompactProgramAccount` no longer stores a `patch` field. Pass an ordinary patch to `invoke`, or construct it from the derived bump with `invoke_with_bump`:

```rust
CreateCompactProgramAccount {
	account: journal,
	payer,
	owner: &ID,
	seeds: &Journal::seeds(authority).as_slices(),
	space: Journal::MIN_SIZE,
}
.invoke_with_bump::<Journal, _>(|bump| {
	JournalPatch::new().authority(*authority).bump(bump)
})?;
```

Code that already has a patch but does not store a bump uses `.invoke::<Journal>(patch)`. Both signed forms take the additional signer slice before the patch or bump-aware patch factory.

`CreateCompactProgramAccountWithBump` keeps its `patch` field, but now rejects a noncanonical supplied bump before allocation.

## Keep generated-client PDA resolution

The IDL extractor recognizes all four typed creation builders as canonical PDA checks:

- `CreateProgramAccount`
- `CreateProgramAccountWithBump`
- `CreateCompactProgramAccount`
- `CreateCompactProgramAccountWithBump`

Removing the redundant assertions does not make the target account required in generated clients. JavaScript callers can keep using the asynchronous builder and omit the PDA account:

```typescript
const instruction = await getInitializeInstructionAsync({
	authority,
	bump,
});
```

The generated client derives the account from the PDA metadata in the IDL. You can still pass the account explicitly when needed.

## Use noncanonical allocation only for compatibility

`AllocateAccountWithBump` is deprecated. Its replacement is `AllocateAccountWithNonCanonicalBump`, which makes the relaxed guarantee clear in code review. It accepts any valid PDA bump and only allocates untyped bytes. Use it for an existing address scheme that cannot migrate to canonical bumps. Use `AllocateAccount` for new PDA namespaces.
