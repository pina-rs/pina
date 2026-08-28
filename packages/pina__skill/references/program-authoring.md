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

## Account-management instruction builders

Pina models account creation, PDA allocation, reallocation, and close operations as values. Construct the documented struct with every input visible, then call `.invoke()` for transaction-level signers or `.invoke_signed(signers)` when another CPI account must sign through program-derived seeds.

```rust
CreateAccount {
	from: payer,
	to: new_account,
	space: 128,
	owner: program_id,
}
.invoke()?;
```

Do not introduce wrapper functions around removed helpers such as `create_account(...)`, `create_program_account::<T>(...)`, or `realloc_account(...)`. Use the matching builder:

| Operation                                             | Builder                        |
| ----------------------------------------------------- | ------------------------------ |
| Create a regular account                              | `CreateAccount`                |
| Derive and create a typed canonical PDA               | `CreateProgramAccount`         |
| Validate an explicit bump and create a typed PDA      | `CreateProgramAccountWithBump` |
| Derive and allocate an untyped canonical PDA          | `AllocateAccount`              |
| Validate an explicit bump and allocate an untyped PDA | `AllocateAccountWithBump`      |
| Reallocate while balancing rent                       | `ReallocAccount`               |
| Reallocate with explicit zero-initialization intent   | `ReallocAccountZeroed`         |
| Close and return lamports                             | `CloseAccount`                 |
| Zero bytes, close, and return lamports                | `CloseAccountZeroed`           |

Typed PDA creation places the account type on the invocation method:

```rust
let (address, bump) = CreateProgramAccount {
	account: state_account,
	payer,
	owner: program_id,
	seeds,
}
.invoke::<State>()?;
```

Canonical PDA builders derive and validate the target address and return `(Address, u8)`. Explicit-bump builders verify the supplied bump before moving lamports. Both forms automatically append the target PDA signer to additional signers supplied by the caller. Use `u64` for create/allocation `space`; reallocation `new_size` remains `usize`.

Close builders intentionally expose only `.invoke()`. They perform checked direct account mutation rather than a CPI, so signer seeds would have no effect.

Generated CPI modules follow the same shape. Construct the generated instruction struct using its documented public account and data fields, then invoke it with the validated program account:

```rust
instructions::Update {
	accounts,
	new_price,
}
.invoke_signed(&program, signers)?;
```

Do not add free convenience constructors to generated CPI modules. Keeping accounts and instruction data visible at construction makes privilege and wire-data review possible at the call site.

## Initialization, resize, and close

Initialization must prove that the target is empty and that its derived address is correct before invoking a create or allocation builder. Resize operations must validate authority, owner, address, writability, and the requested bounds before invoking a reallocation builder.

When closing an account, choose `CloseAccount` or `CloseAccountZeroed` according to the data-erasure requirement. Zero account data before transferring lamports when stale bytes must not remain observable.

## Compatibility review

Regenerate and inspect the IDL after changing:

- a public account, instruction, event, error, or PDA declaration;
- discriminator values;
- field order, field type, or fixed capacity;
- account ordering, signer/writable constraints, or known addresses.

If a change moves bytes or addresses, state that explicitly and require the user's approval when it was not already part of the request.
