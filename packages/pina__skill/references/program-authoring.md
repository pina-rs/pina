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

### Declarative validation

<!-- {=pinaValidationOverview} -->

Pina's opt-in `validation` feature adds allocation-free application validation to `#[account]`, `#[instruction]`, `#[event]`, and `#[derive(Accounts)]`. Add it to the program dependency:

```toml
[dependencies]
pina = { version = "0.15", features = ["validation"] }
```

Each annotated macro generates a `PinaValidate` implementation with `fn validate(&self) -> ProgramResult`. Validation fails fast with the first Solana `ProgramError`; it does not allocate, collect an error tree, deserialize into a second value, or use dynamic dispatch.

Pina runs generated validation automatically after structural decoding in `try_from_bytes`, after fixed or compact initialization, and after `#[derive(Accounts)]` parses the received account slice. Call `.validate()` directly when validating an already-borrowed value. Mutating a view can invalidate a previously checked rule, so validate again before emitting an event or committing application state when the mutation itself must be checked.

<!-- {/pinaValidationOverview} -->

<!-- {=pinaValueValidationRules} -->

Use `#[pina(validate(...))]` on fields of `#[account]`, `#[instruction]`, and `#[event]` structs:

| Rule               | Accepted fields                                     | Meaning                                     |
| ------------------ | --------------------------------------------------- | ------------------------------------------- |
| `min = EXPR`       | Fixed-width integers and Pina `Pod*` integer fields | Inclusive numeric lower bound               |
| `max = EXPR`       | Fixed-width integers and Pina `Pod*` integer fields | Inclusive numeric upper bound               |
| `min_len = EXPR`   | `String`, `PodString`, `Vec`, `PodVec`, and arrays  | Inclusive minimum byte or element count     |
| `max_len = EXPR`   | `String`, `PodString`, `Vec`, `PodVec`, and arrays  | Inclusive maximum byte or element count     |
| `exact_len = EXPR` | `String`, `PodString`, `Vec`, `PodVec`, and arrays  | Exact byte or element count                 |
| `error = ERROR`    | One validation group                                | Replaces the macro's default `ProgramError` |

String lengths are UTF-8 byte lengths. Vector and array lengths are element counts. `exact_len` cannot share a group with `min_len` or `max_len`.

Use `validate(with = function)` in the outer macro for cross-field or domain validation. Fixed schemas pass their generated `*Zc` view; compact accounts pass their generated `*Ref<'_>` view. The function must return `ProgramResult`.

```rust
#[instruction(
	discriminator = Instruction::Transfer,
	validate(with = validate_transfer)
)]
pub struct TransferInstruction {
	#[pina(validate(min = 1, max = 1_000_000, error = TransferError::InvalidAmount))]
	pub amount: u64,

	#[pina(validate(max_len = 64))]
	pub memo: String<64>,
}

fn validate_transfer(value: &TransferInstructionZc) -> ProgramResult {
	if value.amount() == value.memo().len() as u64 {
		return Err(TransferError::AmbiguousTransfer.into());
	}

	Ok(())
}
```

For accounts, the default error is `ProgramError::InvalidAccountData`. Instructions and events default to `ProgramError::InvalidInstructionData`. Put `error = ...` in a validation group when callers need a domain-specific error.

<!-- {/pinaValueValidationRules} -->

<!-- {=pinaAccountValidationRules} -->

Fields in `#[derive(Accounts)]` accept these rules:

| Rule                    | Generated check                                                      |
| ----------------------- | -------------------------------------------------------------------- |
| `signer`                | Requires the transaction signer flag                                 |
| `writable`              | Requires the writable flag on a shared `&AccountView` field          |
| `executable`            | Requires an executable account                                       |
| `address = EXPR`        | Requires one exact address                                           |
| `addresses = EXPR`      | Accepts any address in a slice or array                              |
| `owner = EXPR`          | Requires one exact owner                                             |
| `owners = EXPR`         | Accepts any owner in a slice or array                                |
| `program = EXPR`        | Requires both the program address and executable flag                |
| `sysvar = EXPR`         | Requires both the canonical sysvar address and sysvar owner          |
| `empty`                 | Requires empty account data                                          |
| `not_empty`             | Requires non-empty account data                                      |
| `data_len = EXPR`       | Requires an exact account-data length                                |
| `distinct_from = FIELD` | Requires two present account fields to have different addresses      |
| `error = ERROR`         | Replaces the standard error for every check in that validation group |

Use `&mut AccountView` or `Option<&mut AccountView>` to declare a writable slot. Parsing already enforces writability for those types, so adding `writable` is a compile-time error with a suggested fix. Use the annotation only when a shared reference must still arrive writable.

```rust
#[derive(Accounts)]
#[pina(validate(with = validate_transfer_accounts))]
pub struct TransferAccounts<'a> {
	#[pina(validate(signer))]
	pub authority: &'a AccountView,

	#[pina(validate(owner = ID, not_empty))]
	pub source: &'a mut AccountView,

	#[pina(validate(owner = ID, not_empty, distinct_from = source))]
	pub destination: &'a mut AccountView,

	#[pina(validate(program = token::ID))]
	pub token_program: &'a AccountView,
}

fn validate_transfer_accounts(accounts: &TransferAccounts<'_>) -> ProgramResult {
	if accounts.authority.address() == accounts.destination.address() {
		return Err(TransferError::InvalidAuthority.into());
	}

	Ok(())
}
```

Generated account validation has a stable order: account-slice parsing and implicit writable/duplicate checks; signer, writable, and executable checks; address and owner checks; data checks; cross-field relationships; nested `Accounts` validation; then the struct-level hook. This puts cheap header checks before account-data borrows and gives custom hooks a fully validated input.

Constraints that perform lifecycle work—account creation, PDA discovery, realloc, and close—remain explicit builders or validation calls. They are not hidden in `.validate()`.

<!-- {/pinaAccountValidationRules} -->

<!-- {=pinaValidationAlternativesAndCodegen} -->

The annotations are syntax sugar, not a separate validation engine. Every account rule delegates to the existing `AccountInfoValidation` method with the same name or meaning. You can keep direct validation chains without enabling `validation` or using the new annotations:

```rust
self.authority.assert_signer()?;
self.state
	.assert_owner(&ID)?
	.assert_not_empty()?
	.assert_writable()?;
self.system_program.assert_program(&system::ID)?;
```

You can also write an ordinary function returning `ProgramResult`, call it at the boundary, or manually implement `PinaValidate` when the `validation` feature is enabled. Prefer the form that keeps the security contract easiest to audit.

Codama generation supports both styles. `pina idl` reads declarative `signer` and `writable` rules plus known `address`, `program`, and `sysvar` constants from `#[derive(Accounts)]`. Existing direct `assert_signer`, `assert_writable`, `assert_address`, and PDA validation-chain inference remains supported. Runtime-only value bounds, owners, data lengths, relationships, and custom hooks do not have Codama account-meta equivalents; they stay on-chain constraints and do not prevent IDL or client generation.

<!-- {/pinaValidationAlternativesAndCodegen} -->

<!-- {=pinaValidationExampleGuide} -->

## Complete Boundary-Validation Example

The `examples/validation_program` project uses the feature across every supported macro boundary:

| Boundary             | Example coverage                                                                             |
| -------------------- | -------------------------------------------------------------------------------------------- |
| Instruction data     | Numeric bounds, bounded strings, exact vector lengths, custom errors, and a cross-field hook |
| Instruction accounts | Signer, writable, owner, program, empty, non-empty, distinct-account rules, and struct hooks |
| Stored account state | Numeric bounds and a hook that keeps the minimum no greater than the maximum                 |
| Events               | Numeric, string, and vector constraints plus a hook that rejects duplicate approvals         |

The processor also keeps one policy rule explicit because it combines decoded instruction data with loaded account state. That distinction is intentional: annotations validate one received value or account list, while ordinary Rust remains the clearest place for rules spanning multiple boundaries.

Run its native and deployed-program tests from the repository root:

```bash
devenv shell -- cargo test -p validation_program
devenv shell -- pina test --project examples/validation_program
```

The existing `events_program` also enables `validation` and applies event rules without changing its transport-focused structure. It is the smaller reference for adding validation to an established program.

<!-- {/pinaValidationExampleGuide} -->

### Low-level validation

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
| Resize compact bytes without applying a patch          | `ReallocCompactAccount`               |
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
	space: Journal::MIN_SIZE,
}
.invoke::<Journal>()?;
```

Canonical PDA builders derive and validate the target address and return `(Address, u8)`. Explicit-bump builders verify the supplied bump before moving lamports. Both forms automatically append the target PDA signer to additional signers supplied by the caller. Use `u64` for create/allocation `space`; reallocation `target_size` remains `usize`.

<!-- {=accountReallocationContract} -->

`UpdateResizableAccount` derives the target allocation from its patch. Lower-level reallocation builders take an explicit `target_size`. Every reallocation builder uses `rent_account` for the account that funds growth or receives a shrink refund. When a compact account grows, `rent_account` funds the missing rent before Pina applies the patch. When it shrinks, Pina applies the shorter representation before returning excess rent to `rent_account`. The Solana runtime zero-initializes new bytes.

The Solana runtime limits account growth to `MAX_PERMITTED_DATA_INCREASE` bytes per top-level instruction. Pina rejects a single larger increase before it moves rent. Pinocchio does not expose the original serialized length, so cumulative growth from several reallocations in one instruction can still fail during `AccountView::resize`.

Propagate reallocation errors. If a later resize or update fails after rent moves, Solana restores the account only when the instruction returns that error.

<!-- {/accountReallocationContract} -->

<!-- {=accountReallocationLowLevelExample} -->

Use `ReallocAccount` when the bytes do not use a compact Pina schema:

```ignore
ReallocAccount {
	account,
	rent_account,
	target_size,
	program_id,
}
.invoke()?;
```

Use `invoke_signed` when `rent_account` is a PDA that must sign the system transfer used for growth:

```ignore
ReallocAccount {
	account,
	rent_account,
	target_size,
	program_id,
}
.invoke_signed(rent_account_signers)?;
```

`ReallocAccountZeroed` has the same field names. Its name records the caller's intent that newly allocated bytes start at zero. The current Solana runtime zero-initializes new bytes for both builders.

<!-- {/accountReallocationLowLevelExample} -->

<!-- {=updateResizableAccountExample} -->

Use `UpdateResizableAccount` for a compact update. Its generated patch distinguishes unchanged fields from replacements:

```ignore
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

The builder validates the complete patch and calculates the target size before changing bytes or lamports. It grows before applying a longer representation and applies a shorter representation before shrinking. It skips the resize when the allocation does not change. Use `invoke_signed::<Journal>(signers)` when `rent_account` is a PDA that funds growth.

<!-- {/updateResizableAccountExample} -->

<!-- {=reallocCompactAccountExample} -->

`ReallocCompactAccount` is the lower-level compact builder. It changes the physical allocation and does not apply a patch:

```ignore
ReallocCompactAccount {
	account,
	rent_account,
	target_size,
	program_id,
}
.invoke::<Journal>()?;
```

For growth, call `invoke` before writing the longer representation. For shrinkage, write a valid shorter representation, drop its mutable data borrow, and then call `invoke`. Use `invoke_signed` when a PDA rent account funds growth. Prefer `UpdateResizableAccount` unless the caller needs explicit allocation control.

<!-- {/reallocCompactAccountExample} -->

Close builders intentionally expose only `.invoke()`. They perform checked direct account mutation rather than a CPI, so signer seeds would have no effect.

All reallocation builders use `rent_account` for the account that funds growth and receives shrink refunds. Lower-level builders take an explicit `target_size`; `UpdateResizableAccount` derives it from the patch.

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
