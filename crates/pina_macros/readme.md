# `pina_macros`

<p align="center">
	<img src="https://raw.githubusercontent.com/pina-rs/pina/main/.github/assets/logo.png" alt="The Pina logo: a low-poly origami pineapple" width="140">
</p>

<br>

Procedural macros for building Pina programs with less boilerplate.

This crate powers the attributes/derives re-exported by `pina`.

<!-- {=crateReadmeBadgeRow:"pina_macros"} -->

[![Crates.io](https://img.shields.io/badge/crates.io-pina__macros-orange?logo=rust)](https://crates.io/crates/pina_macros) [![Docs.rs](https://img.shields.io/badge/docs.rs-pina__macros-1f425f?logo=docs.rs)](https://docs.rs/pina_macros/) [![CI](https://github.com/pina-rs/pina/actions/workflows/ci.yml/badge.svg)](https://github.com/pina-rs/pina/actions/workflows/ci.yml) [![Coverage](https://codecov.io/gh/pina-rs/pina/branch/main/graph/badge.svg)](https://codecov.io/gh/pina-rs/pina) [![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://opensource.org/license/apache-2.0)

<!-- {/crateReadmeBadgeRow} -->

## Installation

<br>

Most projects should depend on `pina` and use the re-exported macros.

Compact accounts are opt-in:

```bash
cargo add pina --features compact
```

Declarative validation is opt-in:

```bash
cargo add pina --features validation
```

For fixed account macros needed directly:

```bash
cargo add pina_macros
```

Add `--features compact` or `--features validation` when using the corresponding annotations through a direct `pina_macros` dependency.

## Macros

<br>

- `#[discriminator]`: defines a typed discriminator enum (`u8`, `u16`, `u32`, `u64`).
- `#[account]`: defines discriminator-first fixed or compact account POD structs and generated builders.
- `#[instruction]`: defines discriminator-first instruction data POD structs.
- `#[event]`: defines discriminator-first event POD structs.
- `#[pda]`: defines typed PDA seed, derivation, validation, and one-pass fixed-account loader helpers.
- `#[error]`: maps custom enums to `ProgramError::Custom(code)`.
- `#[derive(Accounts)]`: parses `&mut [AccountView]` into a named struct of shared and/or mutable account references.

## Common Usage

<br>

```rust
use pina::*;

#[discriminator]
pub enum Instruction {
	Initialize = 0,
}

#[instruction(discriminator = Instruction::Initialize)]
pub struct InitializeInstruction {
	pub bump: u8,
}

#[derive(Accounts)]
pub struct InitializeAccounts<'a> {
	pub payer: &'a AccountView,
	pub state: &'a mut AccountView,
	pub system_program: &'a AccountView,
}

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExampleError {
	InvalidAuthority = 6000,
}
```

## Attribute Options

<br>

### `#[discriminator(...)]`

<br>

- `primitive = u8|u16|u32|u64`
- `crate = ::pina` (defaults to `::pina`)
- `final` (omits `#[non_exhaustive]`)

### `#[account(...)]`, `#[instruction(...)]`, `#[event(...)]`

<br>

- `discriminator = PathToEnum`
- `variant = EnumVariant` (optional; defaults to inferred struct name; cannot be combined with a `discriminator` path that includes a variant)
- `crate = ::pina` (optional)
- `compact` (requires the crate's `compact` feature; generates checked read and patch APIs for one or more trailing compact fields)
- `validate(with = function)` (requires `validation`; runs after generated field rules)

### `#[error(...)]`

<br>

- `crate = ::pina` (optional)
- `final` (omits `#[non_exhaustive]`)

### `#[derive(Accounts)]`

<br>

- Supports one lifetime parameter.
- Supports `&'a AccountView`, `&'a mut AccountView`, `&'a [AccountView]`, and `&'a mut [AccountView]` fields.
- Supports `#[pina(remaining)]` on a single trailing field to capture remaining accounts. Mutable trailing addresses are distinct by default; `#[pina(remaining, distinct = false)]` permits duplicates only for intentionally documented instruction contracts.
- Supports `#[pina(crate = ::pina)]` on the struct to override the crate path.
- Supports `#[pina(validate(with = function))]` on the struct and `#[pina(validate(...))]` on account fields when `validation` is enabled.

## Declarative Validation

<!-- {=pinaValidationOverview} -->

Pina's opt-in `validation` feature adds allocation-free application validation to `#[account]`, `#[instruction]`, `#[event]`, and `#[derive(Accounts)]`. Add it to the program dependency:

```toml
[dependencies]
pina = { version = "0.15", features = ["validation"] }
```

Each annotated macro generates a `PinaValidate` implementation with `fn validate(&self) -> ProgramResult`. Validation fails fast with the first Solana `ProgramError`; it does not allocate, collect an error tree, deserialize into a second value, or use dynamic dispatch.

Pina runs generated validation automatically after structural decoding in `try_from_bytes`, after fixed or compact initialization, and after `#[derive(Accounts)]` parses the received account slice. Call `.validate()` directly when validating an already-borrowed value. Mutating a view can invalidate a previously checked rule, so validate again before emitting an event or committing application state when the mutation itself must be checked.

<!-- {/pinaValidationOverview} -->

### Value Rules

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

### Instruction Account Rules

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

### Manual Alternatives and Codama

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

## Notes

<br>

- Generated instructions, events, and ordinary accounts accept audited scalars, addresses, byte arrays, bounded `String` and `Vec` fields, and recursively fixed `Option` fields. PinaPod supplies alignment-one storage and load-bearing validation.
- `#[account(compact)]` accepts a final suffix made from `String<N>`, `Vec<T, N>` for fixed `T`, `Option<String<N>>`, `Option<Vec<T, N>>` for fixed `T`, and `Vec<String<M>, N>`. Fixed `Option<T>` fields stay in the header. Unsupported nesting produces a compile-time error that lists these forms.
- A fixed `#[account]` with `#[pda(bump = ...)]` generates `load_pda` and `load_pda_mut`. These methods validate the typed representation and stored-bump PDA address before returning a guard, without repeating recursive validation.
- A compact `#[account]` with `#[pda(bump = ...)]` generates `with_pda`. The closure-scoped helper validates compact data, the canonical bump, and the PDA address during one runtime borrow.
- Use `PodString<N, PFX>` or `PodVec<T, N, PFX>` for an explicit `1`, `2`, `4`, or `8` byte prefix. Prefix widths are const arguments, not macro attributes.
- The macros are designed for `no_std` Solana program crates.
- If you use `pina`, these macros are available directly without importing `pina_macros`.
