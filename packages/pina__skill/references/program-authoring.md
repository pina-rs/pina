# Program Authoring

## Data and instruction types

Use Pina's macros for their specific wire contracts:

- `#[discriminator]` defines explicit discriminator bytes.
- `#[account]` creates discriminator-first, validated zero-copy account storage.
- `#[instruction]` creates typed instruction data with a discriminator-first wire layout.
- `#[event]` creates typed event data.
- `#[error]` maps program errors without an ad hoc conversion layer.
- `#[pda]` defines PDA constructors and seed helpers.
- `#[derive(Accounts)]` converts the ordered account slice into a typed instruction account set.

Use explicit discriminator values. Reordering enum variants must not change existing wire values.

Fixed-layout storage fields must satisfy zeropod's representation and validation rules. Use Pina's POD wrappers for numeric and boolean storage. Keep text and collections in bounded fixed-capacity representations with explicit length validation.

## Account validation

Validate before reading or mutating account data. A typical chain is:

```rust
account
	.assert_signer()?
	.assert_writable()?
	.assert_owner(program_id)?;
```

Add address, program, sysvar, seed, emptiness, or type assertions when the instruction relies on them. Do not assume a typed cast proves ownership or that a signer proves authority over stored state.

Important cases:

- Check the invoked program's address before CPI.
- Check an account is empty before initialization.
- Check program ownership before changing its lamports.
- Require writability before resize or mutation.
- Validate the exact sysvar address before reading sysvar data.
- Reject duplicate mutable aliases unless the instruction explicitly supports them.
- Bind an authority signer to the authority stored in program state.

## PDAs

Use a stable, type-specific byte-string namespace as the first seed. Prefer canonical bump derivation and validation. Do not reuse one seed namespace for unrelated account types.

Seed changes alter addresses. Treat them as migrations, not refactors.

## Initialization, resize, and close

Initialization must prove that the target is empty and that its derived address is correct before allocating or writing state. Resize operations must validate authority, owner, address, writability, and the requested bounds before changing data length.

When closing an account, use the Pina close helper that matches the data-erasure requirement. Zero account data before transferring lamports when stale bytes must not remain observable.

## Compatibility review

Regenerate and inspect the IDL after changing:

- a public account, instruction, event, error, or PDA declaration;
- discriminator values;
- field order, field type, or fixed capacity;
- account ordering, signer/writable constraints, or known addresses.

If a change moves bytes or addresses, state that explicitly and require the user's approval when it was not already part of the request.
