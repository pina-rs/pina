# `pina`

<p align="center">
	<img src="https://raw.githubusercontent.com/pina-rs/pina/main/.github/assets/logo.png" alt="The Pina logo: a low-poly origami pineapple" width="140">
</p>

<br>

Core runtime crate for building Solana programs on top of [`pinocchio`](https://github.com/anza-xyz/pinocchio).

It provides zero-copy account loaders, discriminator-aware account/instruction/event modeling, account validation traits, and `no_std` entrypoint helpers.

<!-- {=crateReadmeBadgeRow:"pina"} -->

[![Crates.io](https://img.shields.io/badge/crates.io-pina-orange?logo=rust)](https://crates.io/crates/pina) [![Docs.rs](https://img.shields.io/badge/docs.rs-pina-1f425f?logo=docs.rs)](https://docs.rs/pina/) [![CI](https://github.com/pina-rs/pina/actions/workflows/ci.yml/badge.svg)](https://github.com/pina-rs/pina/actions/workflows/ci.yml) [![Coverage](https://codecov.io/gh/pina-rs/pina/branch/main/graph/badge.svg)](https://codecov.io/gh/pina-rs/pina) [![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://opensource.org/license/apache-2.0)

<!-- {/crateReadmeBadgeRow} -->

## Installation

<br>

```bash
cargo add pina
```

Enable optional token helpers:

```bash
cargo add pina --features token
```

## What This Crate Includes

<br>

- `nostd_entrypoint!` for `no_std` Solana entrypoint wiring.
- `#[account]`, `#[instruction]`, `#[event]`, `#[error]`, `#[discriminator]`, and `#[derive(Accounts)]` integration via the default `derive` feature.
- Validation chains on `AccountView` (`assert_signer`, `assert_writable`, `assert_owner`, PDA checks, sysvar checks, and more).
- Zero-copy POD wrappers (`PodU*`, `PodI*`, `PodBool`) for stable on-chain layouts.
- Bounded `String`, `Vec`, and `Option` fields in fixed or compact schemas.
- CPI helpers for system/token operations.

## Fixed accounts

An ordinary `#[account]` reserves every field's full capacity:

```rust
#[account(discriminator = AccountType)]
pub struct Profile {
	pub authority: Address,
	pub name: String<32>,
	pub scores: Vec<u64, 8>,
	pub delegate: Option<Address>,
	pub note: Option<String<64>>,
}
```

PinaPod initializes the complete fixed representation and validates active nested values before a loader returns `ProfileZc`. Use `PodString<N, PFX>` or `PodVec<T, N, PFX>` when a layout needs an explicit `1`, `2`, `4`, or `8` byte prefix.

Typed fixed-account creation uses `invoke::<T>()` when the discriminator plus otherwise zeroed fields is valid. Use `invoke_with::<T>(initialize)` to configure `&mut T::Zc` before final validation. The signed equivalents are `invoke_signed` and `invoke_signed_with`.

Fixed accounts declared with `#[pda(bump = ...)]` also generate `load_pda` and `load_pda_mut`. Prefer these when a handler needs a typed guard: they check the account boundary and stored-bump PDA address in one pass.

## Compact accounts

<br>

<!-- {=compactAccountQuickstart} -->

Compact mode stores a fixed header followed by one or more bounded tails. Enable `compact` for the schema and checked loaders. Enable `account-resize` to apply a patch and adjust rent in one operation:

```toml
[dependencies]
pina = { version = "...", features = ["compact", "account-resize"] }
```

The `compact` feature also enables `derive`. Declare fixed fields first, then the compact tails. `String<N>` uses a one-byte length prefix, and `Vec<T, N>` uses a two-byte length prefix. Use `PodString<N, PFX>` or `PodVec<T, N, PFX>` when the schema needs an explicit prefix width. `PFX` is a byte count and must be `1`, `2`, `4`, or `8`.

```rust
#[account(discriminator = AccountType, compact)]
pub struct Journal {
	pub bump: u8,
	pub authority: Address,
	pub revision: u32,
	pub featured_entry: Option<u64>,
	pub title: String<24>,
	pub entries: Vec<u64, 8>,
	pub markers: PodVec<u8, 8, 8>,
	pub note: Option<String<64>>,
}
```

The compact grammar accepts these tail forms:

- `String<N>`
- `Vec<T, N>` where `T` has a fixed PinaPod representation
- `Option<T>` where `T` has a fixed PinaPod representation
- `Option<String<N>>`
- `Option<Vec<T, N>>` where `T` has a fixed PinaPod representation
- `Vec<String<M>, N>`

`Option<T>` for fixed `T` stays in the header. The other forms use tail storage. A compact schema can contain several tails, but it cannot place a fixed field after the first tail. The macro rejects unsupported nesting and prints the accepted forms in its error.

The macro generates `JournalHeader`, `JournalRef`, and `JournalPatch`. It also generates `HEADER_SIZE`, `MIN_SIZE`, `MAX_SIZE`, checked reads, initialization, projected-size calculation, and atomic updates. `MIN_SIZE` equals `HEADER_SIZE`. Tail prefixes live in the header except for a present `Option<String<N>>` or `Option<Vec<T, N>>`, whose payload retains its own prefix. Each active element of `Vec<String<M>, N>` occupies the fixed `String<M>` footprint, although each string keeps its own logical length.

Pina uses PinaPod for validated alignment-one storage. PinaPod initializes inactive collection capacity and validates each active nested value before Pina returns safe access.

When a compact account also declares `#[pda(..., bump = bump)]`, the macro generates `Type::with_pda`. This closure-scoped loader checks owner, compact data, the canonical stored bump, and the derived account address while one runtime borrow remains active.

<!-- {/compactAccountQuickstart} -->

<!-- {=compactAccountResizeOrdering} -->

Apply all compact changes through one patch:

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

`UpdateResizableAccount` validates the complete patch and calculates the final encoded length before it changes the account. It grows the allocation before applying a longer representation. For a shorter representation, it applies the patch before shrinking the allocation. If the allocation stays the same size, the builder skips the resize. It adjusts the rent balance through `rent_account` and clears bytes removed by the patch. If validation or size calculation fails, both account data and lamport balances remain unchanged.

Use `invoke_signed::<Journal>(signers)` when `rent_account` is a PDA that must sign the system transfer used for growth. The patch and resize ordering stay the same.

The `rent_account` field has the same meaning across `UpdateResizableAccount`, `ReallocAccount`, `ReallocAccountZeroed`, and `ReallocCompactAccount`: it funds growth and receives a shrink refund. The lower-level builders take an explicit `target_size`; the high-level builder derives it from the patch.

The generated patch owns the update plan, so callers do not coordinate `set_*`, `commit`, and `ReallocCompactAccount`. Use `Journal::with_pda` to read a stored-bump compact PDA without a separate `assert_compact_type` or `assert_seeds` pass. End the closure before invoking `UpdateResizableAccount`.

<!-- {/compactAccountResizeOrdering} -->

See `examples/compact_accounts` for a complete lifecycle with unit and Surfpool coverage.

`CreateCompactProgramAccount` and `CreateCompactProgramAccountWithBump` require a generated `patch` field. Pass the account's generated patch, such as `JournalPatch::new()`, for an all-zero, empty-tail default, or set the initial header and tail values in that patch.

## Token account loaders

Enable the `token` feature to load SPL Token and Token-2022 state. The loader name selects the canonical owner that Pina requires:

```rust
let legacy_mint = mint.as_token_mint()?;
let legacy_account = account.as_token_account()?;
let token_2022_mint = mint_2022.as_token_2022_mint()?;
let token_2022_account = account_2022.as_token_2022_account()?;
```

Use `as_token_mint_for_program()` and `as_token_account_for_program()` when the instruction accepts either canonical token program at runtime. These methods reject every other program ID and require the account owner to match the selected program.

Load an existing ATA with `as_associated_token_account()`. This method verifies the runtime owner, derived ATA address, stored wallet, and stored mint before returning a guard-backed token view. Use `assert_associated_token_address()` only when a validation-only path does not need token data.

## Feature Flags

<br>

<!-- {=pinaFeatureFlags} -->

| Feature          | Default | Description                                                  |
| ---------------- | ------- | ------------------------------------------------------------ |
| `derive`         | Yes     | Enables proc macros (`#[account]`, `#[instruction]`, etc.)   |
| `logs`           | Yes     | Enables on-chain logging via `solana-program-log`            |
| `compact`        | No      | Enables compact schemas, checked loaders, and typed APIs     |
| `validation`     | No      | Enables declarative, allocation-free application validation  |
| `token`          | No      | Enables SPL token / token-2022 helpers and ATA utilities     |
| `memo`           | No      | Enables memo program helpers via `pina::memo`                |
| `account-resize` | No      | Enables raw account reallocation and safe Pinocchio resizing |

<!-- {/pinaFeatureFlags} -->

## Feature selection tips

<br>

<!-- {=pinaFeatureSelectionTips} -->

- `derive` is the normal choice for program crates; disable it only when you want the low-level runtime traits without the proc macros.
- `compact` enables `#[account(compact)]`, `PinaCompactAccount`, generated patch types, checked compact loaders, and `pina::String` and `pina::Vec`. It also enables `derive`.
- `validation` enables `PinaValidate` and `#[pina(validate(...))]` rules on accounts, instructions, events, and derived account lists. It also enables `derive`.
- `logs` is useful during **initial development and debugging**, testing, and audits. Disable it when you want the smallest possible binary or completely silent runtime failures.
- `token` enables `pina::token`, `pina::token_2022`, `pina::associated_token_account`, and the `TokenAccount` compatibility aliases over the upstream renamed account types.
- `memo` is separate from `token`, so memo CPI support can be enabled without pulling in the token helper surface.
- `account-resize` enables `ReallocAccount` and `ReallocAccountZeroed`. Enable it together with `compact` for `UpdateResizableAccount`, `ReallocCompactAccount`, and the compact creation builders. Close helpers still do not implicitly resize or zero account data.

<!-- {/pinaFeatureSelectionTips} -->

## Minimal Program Skeleton

<br>

```rust
#![no_std]

use pina::*;

declare_id!("YourProgramId11111111111111111111111111111111");

#[discriminator]
pub enum Instruction {
	Initialize = 0,
}

#[instruction(discriminator = Instruction, variant = Initialize)]
pub struct InitializeInstruction {}

nostd_entrypoint!(process_instruction);

fn process_instruction(
	program_id: &Address,
	accounts: &mut [AccountView],
	data: &[u8],
) -> ProgramResult {
	let ix: Instruction = parse_instruction(program_id, &ID, data)?;
	match ix {
		Instruction::Initialize => {
			let _ = InitializeInstruction::try_from_bytes(data)?;
			let _ = accounts;
			Ok(())
		}
	}
}
```

## PDAs, accounts, and validation in practice

`examples/counter_program` wires the full loop: discriminator-first instructions, a PDA-seeded account, and a validation chain that ends in a checked zero-copy mutation.

```rust
#[account(discriminator = CounterAccountType)]
#[pda(seeds = [SEED_COUNTER, authority: Address], bump = bump)]
pub struct CounterState {
	pub bump: u8,
	pub count: u64,
}

#[derive(Accounts, Debug)]
pub struct IncrementAccounts<'a> {
	pub authority: &'a AccountView,
	pub counter: &'a mut AccountView,
}

impl<'a> ProcessAccountInfos<'a> for IncrementAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = IncrementInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;
		self.counter
			.assert_not_empty()?
			.assert_type::<CounterState>(&ID)?;

		// Verify the account is the PDA for the authority, using the stored
		// bump field (avoids re-deriving the canonical bump on-chain).
		CounterState::assert_seeds(self.counter, self.authority.address(), &ID)?;

		// Mutate state
		let mut counter = self.counter.as_account_mut::<CounterState>(&ID)?;
		let next = counter
			.count
			.get()
			.checked_add(1)
			.ok_or(ProgramError::ArithmeticOverflow)?;
		counter.count.set(next);

		Ok(())
	}
}
```

For the complete program — `CreateProgramAccountWithBump`, `log!`, and the isolated Surfpool tests — see [`examples/counter_program`](https://github.com/pina-rs/pina/tree/main/examples/counter_program).

## Instruction authoring tips

<br>

<!-- {=pinaInstructionAuthoringTips} -->

- Entry points should accept `&mut [AccountView]` and dispatch with `Accounts::try_from((program_id, accounts))?.process(data)`.
- Use `&AccountView` for read-only accounts and `&mut AccountView` only when you need mutable loaders, direct lamport mutation, `close_*` helpers, or writable IDL inference.
- `&mut AccountView` declares and enforces a writable slot. Use `assert_writable()` or `#[pina(validate(writable))]` only when a shared `&AccountView` must arrive writable.
- `as_account()` / `as_account_mut()` return `Ref<T>` / `RefMut<T>` borrow guards. Copy out the fields you need and `drop(...)` the guard before CPIs or later mutable borrows.
- Keep validation chains direct inside `process(self, ...)` when possible. That makes audits easier and gives `pina idl` the clearest signal for signer, writable, PDA, and default-account inference.

<!-- {/pinaInstructionAuthoringTips} -->

## Related Crates

<br>

- [`pina_macros`](https://docs.rs/pina_macros): proc-macro implementations for the attributes and derives used here.
- [`pina_cli`](https://docs.rs/pina_cli): CLI/library used to generate Codama IDLs from Pina programs.
- [`pina_sdk_ids`](https://docs.rs/pina_sdk_ids): shared Solana program/sysvar IDs.

## Codama IDLs

<br>

`pina` models are designed to be extracted into Codama IDLs through `pina_cli`.

```bash
pina idl --path ./my_program --output ./idls/my_program.json
```

From there you can generate JS clients with Codama renderers, or Pina-style Rust clients using this repository's `pina_codama_renderer` tool.
