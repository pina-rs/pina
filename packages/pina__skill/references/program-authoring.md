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

Fixed-layout storage fields must satisfy PinaPod's representation and validation rules. Pina schemas accept native numeric and boolean fields, `Address`, byte arrays, bounded `String<N>` and `Vec<T, N>`, and fixed `Option<T>` values. The derive maps native fields to alignment-one storage wrappers.

Use `PodString<N, PFX>` or `PodVec<T, N, PFX>` when a wire layout needs an explicit prefix width. `PFX` is `1`, `2`, `4`, or `8` bytes. Keep the prefix in the type declaration instead of adding a macro attribute.

Compact accounts place fixed fields first and one or more dynamic tails last. They support `Option<T>` for fixed `T`, `String<N>`, `Vec<T, N>` for fixed `T`, `Option<String<N>>`, `Option<Vec<T, N>>` for fixed `T`, and `Vec<String<M>, N>`. Apply compact changes through the generated patch and `UpdateResizableAccount`; do not coordinate a mutable view, `commit`, and raw reallocation at the call site.

```rust
UpdateResizableAccount {
	account: self.journal,
	rent_account: self.authority,
	program_id: &ID,
	patch: JournalPatch::new()
		.revision(next_revision)
		.replace_entries(&entries)
		.note(Some("Updated")),
}
.invoke::<Journal>()?;
```

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

| Operation                                              | Builder                               |
| ------------------------------------------------------ | ------------------------------------- |
| Create a regular account                               | `CreateAccount`                       |
| Derive and create a typed canonical PDA                | `CreateProgramAccount`                |
| Validate an explicit bump and create a typed PDA       | `CreateProgramAccountWithBump`        |
| Derive and create a compact canonical PDA from a patch | `CreateCompactProgramAccount`         |
| Create a compact PDA with an explicit bump and patch   | `CreateCompactProgramAccountWithBump` |
| Derive and allocate an untyped canonical PDA           | `AllocateAccount`                     |
| Validate an explicit bump and allocate an untyped PDA  | `AllocateAccountWithBump`             |
| Reallocate while balancing rent                        | `ReallocAccount`                      |
| Reallocate with explicit zero-initialization intent    | `ReallocAccountZeroed`                |
| Apply a checked compact patch and adjust rent          | `UpdateResizableAccount`              |
| Close and return lamports                              | `CloseAccount`                        |
| Zero bytes, close, and return lamports                 | `CloseAccountZeroed`                  |

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

Choose the fixed-account invocation method by initialization contract:

- `invoke::<T>()` and `invoke_signed::<T>(signers)` write the discriminator and leave all other bytes at zero. Use them only if final validation accepts that representation.
- `invoke_with::<T>(initialize)` and `invoke_signed_with::<T>(signers, initialize)` configure `&mut T::Zc` before final validation. The closure returns `Result<(), PinaPodError>`.

Prefer the closure form when the account has required nonzero initial values. It is mandatory for an advanced manual `PinaAccount` whose storage includes a nonzero-only enum. Do not infer from this escape hatch that Pina's `#[account]` macro accepts arbitrary custom enum fields; the macro grammar remains closed.

Compact creation uses a generated patch instead of an initializer closure. Always supply the required `patch` field, including for a header-only default:

```rust
CreateCompactProgramAccountWithBump {
	account: journal,
	payer,
	owner: &ID,
	seeds,
	bump,
	patch: JournalPatch::new().bump(bump),
	space: Journal::HEADER_SIZE,
}
.invoke::<Journal>()?;
```

Canonical PDA builders derive and validate the target address and return `(Address, u8)`. Explicit-bump builders verify the supplied bump before moving lamports. Both forms automatically append the target PDA signer to additional signers supplied by the caller. Use `u64` for create/allocation `space`; reallocation `new_size` remains `usize`.

Close builders intentionally expose only `.invoke()`. They perform checked direct account mutation rather than a CPI, so signer seeds would have no effect.

`UpdateResizableAccount` uses `rent_account` for the account that funds growth and receives shrink refunds. Existing low-level reallocation builders keep their `payer` fields.

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
