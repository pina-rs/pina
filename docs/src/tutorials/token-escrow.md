# Token Escrow Tutorial

<br>

This tutorial walks through the `examples/escrow_program` step by step. The program implements a trustless token exchange between two parties using a PDA-owned vault account.

## How the escrow works

<br>

1. **Make** -- the maker deposits token A into a PDA-owned vault and records the desired amount of token B in an escrow state account.
2. **Take** -- the taker sends token B to the maker, the vault releases token A to the taker, and the escrow is closed with rent returned to the maker.

No party needs to trust the other. The program enforces the exchange atomically: either both transfers happen or neither does.

## Project setup

<br>

The escrow program enables the `token` feature for SPL token helpers:

```toml
[dependencies]
pina = { workspace = true, features = ["logs", "token", "derive"] }

[dev-dependencies]
mollusk-svm = { workspace = true }
```

The `token` feature unlocks CPI wrappers for SPL Token, Token-2022, and Associated Token Account operations.

## Program ID and discriminators

<br>

```rust
use pina::*;

declare_id!("4ibrEMW5F6hKnkW4jVedswYv6H6VtwPN6ar6dvXDN1nT");

#[discriminator]
pub enum EscrowInstruction {
	Make = 1,
	Take = 2,
}

#[discriminator]
pub enum EscrowAccount {
	EscrowState = 1,
}
```

Two discriminator enums serve different purposes. `EscrowInstruction` tags instruction data so the entrypoint can dispatch to the right handler. `EscrowAccount` tags on-chain account data so the program can verify it is reading the correct account type.

## Custom errors

<br>

The `#[error]` macro converts an enum into a set of `ProgramError::Custom` error codes:

```rust
#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscrowError {
	OfferKeyMismatch = 0,
	TokenAccountMismatch = 1,
}
```

Each variant's numeric value becomes the custom error code. You can return these from any processor via `Err(EscrowError::OfferKeyMismatch.into())`.

## Escrow state account

<br>

The `#[account]` macro defines the on-chain state layout:

```rust
#[account(discriminator = EscrowAccount)]
pub struct EscrowState {
	pub maker: Address,
	pub mint_a: Address,
	pub mint_b: Address,
	pub amount_a: u64,
	pub amount_b: u64,
	pub seed: u64,
	pub bump: u8,
}
```

The macro auto-injects a discriminator field as the first byte (set to `EscrowAccount::EscrowState`) and derives zeropod's native-schema machinery. `EscrowStateZc` is the generated storage view: its integer fields are alignment-one little-endian wrappers, and account loaders return that validated view without copying.

The `seed` and `bump` fields are stored so that PDA derivation can be verified on subsequent instructions without re-computing it.

## Instruction data

<br>

```rust
#[instruction(discriminator = EscrowInstruction::Make)]
pub struct MakeInstruction {
	pub seed: u64,
	pub amount_a: u64,
	pub amount_b: u64,
	pub bump: u8,
}

#[instruction(discriminator = EscrowInstruction::Take)]
pub struct TakeInstruction {}
```

`MakeInstruction` carries all the parameters needed to set up the escrow. `TakeInstruction` has no payload beyond its discriminator byte -- the taker just needs to invoke the instruction with the right accounts.

## PDA seeds

<br>

The escrow PDA is derived from a prefix, the maker's address, and a user-chosen seed. Pina's `#[pda]` attribute declares the typed seeds once on the account struct:

```rust
#[account(discriminator = EscrowAccount)]
#[pda(seeds = [SEED_PREFIX, maker: Address, seed: u64], bump = bump)]
pub struct EscrowState {
	pub maker: Address,
	// ...
	pub seed: PodU64,
	pub bump: u8,
}

/// Seed prefix for escrow PDAs.
const SEED_PREFIX: &[u8] = b"escrow";
```

The attribute generates:

- `EscrowState::seeds(maker, seed)` -- a typed seeds struct with `as_slices()` (without bump) and `with_bump(bump)` (with bump).
- `EscrowState::try_find_pda(...)` / `find_pda(...)` -- canonical PDA derivation.
- `EscrowState::assert_seeds(account, ...)` -- verifies an existing account against the stored `bump` field, avoiding a canonical bump search on-chain.

Supported seed types are `Address`, `u8`, `u16`, `u32`, `u64`, `[u8; N]`, and `const &[u8]` references.

## Make: accounts and validation

<br>

```rust
#[derive(Accounts, Debug)]
pub struct MakeAccounts<'a> {
	pub maker: &'a AccountView,
	pub mint_a: &'a AccountView,
	pub mint_b: &'a AccountView,
	pub maker_ata_a: &'a AccountView,
	pub escrow: &'a mut AccountView,
	pub vault: &'a AccountView,
	pub system_program: &'a AccountView,
	pub token_program: &'a AccountView,
}
```

Accounts are listed in the order clients must provide them. The `#[derive(Accounts)]` macro maps each positional `AccountView` from the mutable entrypoint slice into its named field.

The processor validates every account before performing any mutation:

```rust
const SPL_PROGRAM_IDS: [Address; 2] = [token::ID, token_2022::ID];

impl<'a> ProcessAccountInfos<'a> for MakeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = MakeInstruction::try_from_bytes(data)?;
		let maker_address = *self.maker.address();
		let escrow_seeds = EscrowState::seeds(&maker_address, u64::from(args.seed));
		let escrow_seeds_with_bump = escrow_seeds.with_bump(args.bump);

		// Validate all accounts before mutating anything.
		self.token_program.assert_addresses(&SPL_PROGRAM_IDS)?;
		self.maker.assert_signer()?;
		self.mint_a.assert_owners(&SPL_PROGRAM_IDS)?;
		self.mint_b.assert_owners(&SPL_PROGRAM_IDS)?;
		self.maker_ata_a.assert_associated_token_address(
			self.maker.address(),
			self.mint_a.address(),
			self.token_program.address(),
		)?;
		self.escrow
			.assert_empty()?
			.assert_writable()?
			.assert_seeds_with_bump(&escrow_seeds_with_bump.as_slices(), &ID)?;
		self.vault
			.assert_empty()?
			.assert_writable()?
			.assert_associated_token_address(
				self.escrow.address(),
				self.mint_a.address(),
				self.token_program.address(),
			)?;

		// ... create accounts and transfer tokens ...
		Ok(())
	}
}
```

Key validation patterns:

- `assert_addresses` checks that the token program is either SPL Token or Token-2022.
- `assert_signer` ensures the maker signed the transaction.
- `assert_owners` verifies mint accounts are owned by a token program.
- `assert_associated_token_address` derives the expected ATA address and compares.
- `assert_empty` + `assert_writable` + `assert_seeds_with_bump` validates the PDA is fresh and derivable.

Validation methods return the same reference type they receive, so mutable chains stay mutable all the way to `as_account_mut()`.

## Make: creating the escrow

<br>

After validation the processor creates the PDA account and initializes its state:

```rust
create_program_account_with_bump::<EscrowState>(
	self.escrow,
	self.maker,
	&ID,
	&escrow_seeds.as_slices(),
	args.bump,
)?;

let mut escrow = self.escrow.as_account_mut::<EscrowState>(&ID)?;
*escrow = EscrowState::builder()
	.maker(*self.maker.address())
	.mint_a(*self.mint_a.address())
	.mint_b(*self.mint_b.address())
	.amount_a(args.amount_a)
	.amount_b(args.amount_b)
	.seed(args.seed)
	.bump(args.bump)
	.build();
drop(escrow);
```

`create_program_account_with_bump` issues a `CreateAccount` CPI to the system program, allocating `size_of::<EscrowState>()` bytes and setting the owner to this program.

`as_account_mut` reinterprets the raw account bytes as a guard-backed `RefMut<EscrowState>`. The builder (generated by the `#[account]` macro) provides a type-safe way to populate all fields.

## Make: token operations via CPI

<br>

With the escrow account created, the program creates the vault ATA and transfers tokens:

```rust
associated_token_account::instructions::Create {
	account: self.vault,
	funding_account: self.maker,
	wallet: self.escrow,
	mint: self.mint_a,
	system_program: self.system_program,
	token_program: self.token_program,
}
.invoke()?;

let token_program = *self.token_program.address();
let decimals = self
	.mint_a
	.as_token_mint_for_program(&token_program)?
	.decimals();
drop(
	self.mint_b
		.as_token_mint_for_program(&token_program)?,
);
drop(self.maker_ata_a.as_associated_token_account_checked(
	self.maker.address(),
	self.mint_a.address(),
	&token_program,
)?);
token::instructions::TransferChecked::new(
	self.maker_ata_a,
	self.mint_a,
	self.vault,
	self.maker,
	args.amount_a.into(),
	decimals,
)
.invoke_with_program(&token_program)?;
```

Pina's `token` feature provides typed CPI instruction builders. Construct the instruction with `new()` and invoke it through the validated token program account. The shared loader validates either the fixed SPL Token layout or a complete Token-2022 extension layout according to the selected program.

The vault is an ATA owned by the escrow PDA. This means only the escrow program (signing with the PDA seeds) can later release the tokens.

## Take: completing the exchange

<br>

The Take instruction performs two token transfers and cleans up:

1. Transfer token B from taker to maker (authorized by the taker's signature).
2. Transfer token A from vault to taker (authorized by the escrow PDA via `invoke_signed`).
3. Close the vault account and return rent to the maker.
4. Zero and close the escrow state account.

```rust
impl<'a> ProcessAccountInfos<'a> for TakeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = TakeInstruction::try_from_bytes(data)?;

		// ... validation omitted for brevity ...

		let (maker, seed, bump, amount_b) = {
			let escrow = self.escrow.as_account::<EscrowState>(&ID)?;
			(escrow.maker, escrow.seed, escrow.bump, escrow.amount_b)
		};
		let token_program = *self.token_program.address();
		let decimals_a = self
			.mint_a
			.as_token_mint_for_program(&token_program)?
			.decimals();
		let decimals_b = self
			.mint_b
			.as_token_mint_for_program(&token_program)?
			.decimals();

		// Verify the escrow is the PDA for the maker and seed, using the
		// stored bump field (avoids re-deriving the canonical bump on-chain).
		EscrowState::assert_seeds(self.escrow, &maker, u64::from(seed), &ID)?;

		// Transfer token B: taker -> maker
		token::instructions::TransferChecked::new(
			self.taker_ata_b,
			self.mint_b,
			self.maker_ata_b,
			self.taker,
			u64::from(amount_b),
			decimals_b,
		)
		.invoke_with_program(&token_program)?;

		// Transfer token A: vault -> taker (PDA-signed)
		let escrow_seeds = EscrowState::seeds(&maker, u64::from(seed));
		let escrow_seeds_with_bump = escrow_seeds.with_bump(bump);
		let seed_values = escrow_seeds_with_bump.as_slices().map(Seed::from);
		let escrow_signer = Signer::from(&seed_values);
		let signers = [escrow_signer];

		let vault_amount = self
			.vault
			.as_associated_token_account_checked(
				self.escrow.address(),
				self.mint_a.address(),
				&token_program,
			)?
			.amount();
		token::instructions::TransferChecked::new(
			self.vault,
			self.mint_a,
			self.taker_ata_a,
			self.escrow,
			vault_amount,
			decimals_a,
		)
		.invoke_signed_with_program(&signers, &token_program)?;

		// Close vault and escrow
		token::instructions::CloseAccount::new(self.vault, self.maker, self.escrow)
			.invoke_signed_with_program(&signers, &token_program)?;

		self.escrow.as_account_mut::<EscrowState>(&ID)?.zeroed();
		self.escrow.close_with_recipient(self.maker)
	}
}
```

The PDA signer is constructed from the same seeds used to derive the escrow address. `invoke_signed_with_program` passes these seeds to the selected SPL Token program so the runtime can verify the PDA signature.

`close_with_recipient` transfers the remaining lamports to the maker and closes the account. Use `zeroed()` first when the account data must be wiped before close.

## Entrypoint

<br>

The entrypoint ties everything together with a simple match:

```rust
#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use pina::*;

	use super::*;

	nostd_entrypoint!(process_instruction);

	#[inline(always)]
	pub fn process_instruction(
		program_id: &Address,
		accounts: &mut [AccountView],
		data: &[u8],
	) -> ProgramResult {
		let instruction: EscrowInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			EscrowInstruction::Make => MakeAccounts::try_from(accounts)?.process(data),
			EscrowInstruction::Take => TakeAccounts::try_from(accounts)?.process(data),
		}
	}
}
```

## Testing

<br>

Unit tests verify discriminator stability, seed construction, and program ID validation:

```rust
#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn instruction_discriminators_are_stable() {
		assert_eq!(EscrowInstruction::Make as u8, 1);
		assert_eq!(EscrowInstruction::Take as u8, 2);
	}

	#[test]
	fn seeds_build_expected_seed_arrays() {
		let maker = Address::new_from_array([3u8; 32]);
		let seed = PodU64::from(42);
		let bump = 7u8;

		let seeds = EscrowState::seeds(&maker, u64::from(seed));
		assert_eq!(seeds.as_slices().len(), 3);

		let seeds_with_bump = seeds.with_bump(bump);
		assert_eq!(seeds_with_bump.as_slices().len(), 4);
	}

	#[test]
	fn parse_instruction_rejects_program_id_mismatch() {
		let wrong_program_id: Address = [9u8; 32].into();
		let data = [EscrowInstruction::Make as u8];
		let result = parse_instruction::<EscrowInstruction>(&wrong_program_id, &ID, &data);
		assert!(matches!(result, Err(ProgramError::IncorrectProgramId)));
	}
}
```

For full integration tests, use `mollusk-svm` to simulate transactions with real token accounts and verify the entire Make/Take flow end-to-end.

## Key takeaways

<br>

- **PDA vaults** hold tokens on behalf of the program. Only the program can sign for them using `invoke_signed`.
- **Validation-first** -- check every account before performing any mutation.
- **Typed CPI builders** in the `token` feature eliminate raw account-meta boilerplate.
- **Zero-copy state** with `#[account]` avoids serialization overhead.
- **Feature-gated entrypoints** let the same crate serve as both an on-chain program and a testable library.
