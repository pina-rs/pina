# Token CPI Recipes

This page collects the token-program CPI patterns that changed or became more important with the Pinocchio 0.11 upgrade:

- token `Batch`
- token `UnwrapLamports`
- token `WithdrawExcessLamports`
- token-2022 `Reallocate`

All examples assume the `token` feature is enabled in your program crate:

```toml
pina = { version = "...", features = ["derive", "logs", "token"] }
```

## Before you invoke token CPIs

Keep the same runtime rules explicit in Pina:

- validate the token program account explicitly when it is passed in
- call `assert_writable()` on every account your instruction expects to mutate
- call `assert_signer()` on every authority that must authorize the CPI
- validate ATA addresses explicitly when the CPI expects a specific associated token account
- if you loaded account state with `.as_account()` or `.as_account_mut()`, copy out the fields you need and drop the guard before the CPI

That last point matters more now that `as_account()` and `as_account_mut()` return borrow guards instead of bare references.

## Token `Batch`

`pina::token::instructions::Batch` lets you serialize multiple SPL token instructions into one token-program batch CPI. This is useful when you already know the full instruction set up front and want one token-program invocation instead of several separate calls.

```rust
use core::mem::MaybeUninit;

use pina::InstructionAccount;
use pina::ProgramResult;
use pina::pinocchio::cpi::CpiAccount;
use pina::token::instructions::Batch;
use pina::token::instructions::InitializeAccount3;
use pina::token::instructions::InitializeMint2;
use pina::token::instructions::IntoBatch;

fn initialize_mint_and_vault(
	mint: &pina::AccountView,
	vault: &pina::AccountView,
	mint_authority: &pina::AccountView,
	vault_owner: &pina::AccountView,
) -> ProgramResult {
	mint.assert_writable()?;
	vault.assert_writable()?;
	mint_authority.assert_signer()?;

	let mut data = [MaybeUninit::<u8>::uninit(); Batch::MAX_DATA_LEN];
	let mut instruction_accounts =
		[MaybeUninit::<InstructionAccount>::uninit(); Batch::MAX_ACCOUNTS_LEN];
	let mut accounts = [MaybeUninit::<CpiAccount>::uninit(); Batch::MAX_ACCOUNTS_LEN];

	let mut batch = Batch::new(&mut data, &mut instruction_accounts, &mut accounts)?;

	InitializeMint2::new(
		mint,
		9,
		mint_authority.address(),
		Some(mint_authority.address()),
	)
	.into_batch(&mut batch)?;

	InitializeAccount3::new(vault, mint, vault_owner.address()).into_batch(&mut batch)?;

	batch.invoke()
}
```

Use `Batch` when:

- all instructions target the same token program
- you can prepare all required buffers up front
- you want the token program, not your program, to interpret the batched payload

## Token `UnwrapLamports`

`UnwrapLamports` transfers lamports out of a wrapped-native token account. Use `Amount::All` to unwrap everything or `Amount::Some(amount)` for a partial unwrap.

```rust
use pina::ProgramResult;
use pina::token::instructions::Amount;
use pina::token::instructions::UnwrapLamports;

fn unwrap_all_native_sol(
	source: &pina::AccountView,
	destination: &pina::AccountView,
	authority: &pina::AccountView,
) -> ProgramResult {
	source.assert_writable()?;
	destination.assert_writable()?;
	authority.assert_signer()?;

	UnwrapLamports::new(source, destination, authority, Amount::All).invoke()
}
```

This is the right helper when the source account is a wrapped SOL token account and you want to move lamports back out to a system account.

## Token `WithdrawExcessLamports`

`WithdrawExcessLamports` is the "rescue stray SOL" helper. It moves lamports that were sent to a token-owned account by mistake while leaving the required rent-exempt balance behind.

```rust
use pina::ProgramResult;
use pina::token::instructions::WithdrawExcessLamports;

fn rescue_stray_sol(
	source: &pina::AccountView,
	destination: &pina::AccountView,
	authority: &pina::AccountView,
) -> ProgramResult {
	source.assert_writable()?;
	destination.assert_writable()?;
	authority.assert_signer()?;

	WithdrawExcessLamports::new(source, destination, authority).invoke()
}
```

Reach for this when the source account is still a token-program-owned account and you want to keep it valid instead of closing it.

## Token-2022 `Reallocate`

`Reallocate` grows a token-2022 account so it can hold additional extension state. The payer funds the extra rent, the system program is passed explicitly, and the owner/delegate still authorizes the change.

```rust
use pina::ProgramResult;
use pina::system;
use pina::token_2022;
use pina::token_2022::instructions::ExtensionDiscriminator;
use pina::token_2022::instructions::Reallocate;

fn enable_token_extensions(
	account: &pina::AccountView,
	payer: &pina::AccountView,
	system_program: &pina::AccountView,
	owner: &pina::AccountView,
) -> ProgramResult {
	account.assert_writable()?;
	payer.assert_signer()?.assert_writable()?;
	system_program.assert_address(&system::ID)?;
	owner.assert_signer()?;

	let extensions = [
		ExtensionDiscriminator::MemoTransfer,
		ExtensionDiscriminator::TransferHook,
	];

	Reallocate::new(
		&token_2022::ID,
		account,
		payer,
		system_program,
		owner,
		&extensions,
	)
	.invoke()
}
```

Use this before initializing or relying on token-2022 extensions that require additional account space.

## Practical migration notes

When porting older code to the current Pina API, keep these patterns in mind:

- `&mut [AccountView]` entrypoints do **not** make writability checks implicit
- mutable account fields in `#[derive(Accounts)]` help the type system and IDL, but `assert_writable()` should still appear in the runtime validation chain
- borrow guards should stay short-lived around token CPIs
- token and token-2022 state loaders still work through `pina::token::state` and `pina::token_2022::state`, including the `TokenAccount` compatibility alias

For a larger end-to-end token flow, see the [`token-escrow` tutorial](./token-escrow.md).
