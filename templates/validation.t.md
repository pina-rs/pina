<!-- {@pinaValidationOverview} -->

Pina's opt-in `validation` feature adds allocation-free application validation to `#[account]`, `#[instruction]`, `#[event]`, and `#[derive(Accounts)]`. Add it to the program dependency:

```toml
[dependencies]
pina = { version = "0.15", features = ["validation"] }
```

Each annotated macro generates a `PinaValidate` implementation with `fn validate(&self) -> ProgramResult`. Validation fails fast with the first Solana `ProgramError`; it does not allocate, collect an error tree, deserialize into a second value, or use dynamic dispatch.

Pina runs generated validation automatically after structural decoding in `try_from_bytes`, after fixed or compact initialization, and after `#[derive(Accounts)]` parses the received account slice. Call `.validate()` directly when validating an already-borrowed value. Mutating a view can invalidate a previously checked rule, so validate again before emitting an event or committing application state when the mutation itself must be checked.

<!-- {/pinaValidationOverview} -->

<!-- {@pinaValueValidationRules} -->

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

<!-- {@pinaAccountValidationRules} -->

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

<!-- {@pinaValidationAlternativesAndCodegen} -->

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
