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
	pub amount_a: PodU64,
	pub amount_b: PodU64,
	pub seed: PodU64,
	pub bump: u8,
}
```

The macro auto-injects a discriminator field as the first byte (set to `EscrowAccount::EscrowState`). It also derives `Pod`, `Zeroable`, `HasDiscriminator`, and `TypedBuilder`. All fields use fixed-size types (`Address` is 32 bytes, `PodU64` is 8 bytes little-endian) so the struct has a stable `#[repr(C)]` layout suitable for zero-copy reads.

The `seed` and `bump` fields are stored so that PDA derivation can be verified on subsequent instructions without re-computing it.

## Instruction data

<br>

```rust
#[instruction(discriminator = EscrowInstruction, variant = Make)]
pub struct MakeInstruction {
	pub seed: PodU64,
	pub amount_a: PodU64,
	pub amount_b: PodU64,
	pub bump: u8,
}

#[instruction(discriminator = EscrowInstruction, variant = Take)]
pub struct TakeInstruction {}
```

`MakeInstruction` carries all the parameters needed to set up the escrow. `TakeInstruction` has no payload beyond its discriminator byte -- the taker just needs to invoke the instruction with the right accounts.

## PDA seeds

<br>

The escrow PDA is derived from a prefix, the maker's address, and a user-chosen seed:

```rust
const SEED_PREFIX: &[u8] = b"escrow";

macro_rules! seeds_escrow {
	($maker:expr, $seed:expr) => {
		&[SEED_PREFIX, $maker, $seed]
	};
	($maker:expr, $seed:expr, $bump:expr) => {
		&[SEED_PREFIX, $maker, $seed, &[$bump]]
	};
}
```

The seed macro generates the PDA seeds array in both forms: without bump (for `create_program_account_with_bump`) and with bump (for `assert_seeds_with_bump`).

## Make: accounts and validation

<br>

```rust
#[derive(Accounts, Debug)]
pub struct MakeAccounts<'a> {
	pub maker: &'a AccountView,
	pub mint_a: &'a AccountView,
	pub mint_b: &'a AccountView,
	pub maker_ata_a: &'a AccountView,
	pub escrow: &'a AccountView,
	pub vault: &'a AccountView,
	pub system_program: &'a AccountView,
	pub token_program: &'a AccountView,
}
```

Accounts are listed in the order clients must provide them. The `#[derive(Accounts)]` macro maps each positional `AccountView` to its named field.

The processor validates every account before performing any mutation:

```rust
const SPL_PROGRAM_IDS: [Address; 2] = [token::ID, token_2022::ID];

impl<'a> ProcessAccountInfos<'a> for MakeAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = MakeInstruction::try_from_bytes(data)?;
		let escrow_seeds = seeds_escrow!(self.maker.address().as_ref(), &args.seed.0);
		let escrow_seeds_with_bump =
			seeds_escrow!(self.maker.address().as_ref(), &args.seed.0, args.bump);

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
			.assert_seeds_with_bump(escrow_seeds_with_bump, &ID)?;
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

Validation methods return `Result<&AccountView>` so they chain naturally with `?`.

## Make: creating the escrow

<br>

After validation the processor creates the PDA account and initializes its state:

```rust
create_program_account_with_bump::<EscrowState>(
	self.escrow,
	self.maker,
	&ID,
	escrow_seeds,
	args.bump,
)?;

let escrow = self.escrow.as_account_mut::<EscrowState>(&ID)?;
*escrow = EscrowState::builder()
	.maker(*self.maker.address())
	.mint_a(*self.mint_a.address())
	.mint_b(*self.mint_b.address())
	.amount_a(args.amount_a)
	.amount_b(args.amount_b)
	.seed(args.seed)
	.bump(args.bump)
	.build();
```

`create_program_account_with_bump` issues a `CreateAccount` CPI to the system program, allocating `size_of::<EscrowState>()` bytes and setting the owner to this program.

`as_account_mut` reinterprets the raw account bytes as a mutable reference to `EscrowState`. The builder (generated by the `#[account]` macro) provides a type-safe way to populate all fields.

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

let decimals = self.mint_a.as_token_mint()?.decimals();
token_2022::instructions::TransferChecked {
	from: self.maker_ata_a,
	to: self.vault,
	authority: self.maker,
	amount: args.amount_a.into(),
	mint: self.mint_a,
	decimals,
	token_program: self.token_program.address(),
}
.invoke()?;
```

Pina's `token` feature provides typed CPI instruction builders. You fill in the struct fields and call `.invoke()` -- the framework handles account meta construction and the CPI call.

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
	fn process(&self, data: &[u8]) -> ProgramResult {
		let _ = TakeInstruction::try_from_bytes(data)?;

		// ... validation omitted for brevity ...

		let EscrowState {
			maker,
			seed,
			bump,
			amount_b,
			..
		} = self.escrow.as_account::<EscrowState>(&ID)?;

		// Transfer token B: taker -> maker
		token_2022::instructions::TransferChecked {
			from: self.taker_ata_b,
			mint: self.mint_b,
			to: self.maker_ata_b,
			authority: self.taker,
			amount: (*amount_b).into(),
			decimals: self.mint_b.as_token_2022_mint()?.decimals(),
			token_program: self.token_program.address(),
		}
		.invoke()?;

		// Transfer token A: vault -> taker (PDA-signed)
		let bump_as_seeds = [*bump];
		let escrow_seeds =
			seeds_escrow!(true, self.maker.address().as_ref(), &seed.0, &bump_as_seeds);
		let escrow_signer = Signer::from(&escrow_seeds);
		let signers = [escrow_signer];

		token_2022::instructions::TransferChecked {
			from: self.vault,
			mint: self.mint_a,
			to: self.taker_ata_a,
			authority: self.escrow,
			amount: self.vault.as_token_2022_account()?.amount(),
			decimals: self.mint_a.as_token_2022_mint()?.decimals(),
			token_program: self.token_program.address(),
		}
		.invoke_signed(&signers)?;

		// Close vault and escrow
		token_2022::instructions::CloseAccount {
			account: self.vault,
			destination: self.maker,
			authority: self.escrow,
			token_program: self.token_program.address(),
		}
		.invoke_signed(&signers)?;

		self.escrow.as_account_mut::<EscrowState>(&ID)?.zeroed();
		self.escrow.close_with_recipient(self.maker)
	}
}
```

The PDA signer is constructed from the same seeds used to derive the escrow address. `invoke_signed` passes these seeds to the runtime so it can verify the PDA signature.

`close_with_recipient` transfers remaining lamports to the maker and zeros the account data, reclaiming the rent.

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
		accounts: &[AccountView],
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
	fn seeds_macro_builds_expected_seed_arrays() {
		let maker = [3u8; 32];
		let seed = PodU64::from_primitive(42);
		let bump = 7u8;

		let seeds = seeds_escrow!(&maker, &seed.0);
		assert_eq!(seeds.len(), 3);

		let seeds_with_bump = seeds_escrow!(&maker, &seed.0, bump);
		assert_eq!(seeds_with_bump.len(), 4);
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
