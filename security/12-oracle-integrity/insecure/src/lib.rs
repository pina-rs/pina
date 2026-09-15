//! INSECURE: Borrowing power derived from an unvalidated price feed.
//!
//! The price account is checked for ownership by the oracle program, but the
//! market never pins *which* feed it trusts and never checks freshness. Any
//! feed created under the oracle program — including one the attacker just
//! deployed against a wash-traded token — prices every loan in the market.

#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("6BWQWeFN75hwyAj4vURT9bn4uHkc1RCKDk4zE4dZ2ysm");

/// Address of the trusted oracle program that owns price feed accounts.
///
/// The account address is a stand-in for a real oracle deployment.
const ORACLE_PROGRAM_ID: Address = address!("fgVrbM93Xo3za8dGCqqUvXHEzuN8QzVbb43QBSykaCa");

/// Price scale used by the feed (8 decimals).
const PRICE_SCALE: u64 = 100_000_000;

/// Maximum loan-to-value, in basis points.
const MAX_LTV_BPS: u64 = 7_500;

#[discriminator]
pub enum LendingInstruction {
	Borrow = 0,
}

#[discriminator]
pub enum LendingAccount {
	Market = 1,
	PriceFeed = 2,
}

#[account(discriminator = LendingAccount)]
pub struct Market {
	pub oracle: Address,
}

#[account(discriminator = LendingAccount)]
pub struct PriceFeed {
	pub price: u64,
	pub updated_at: i64,
}

#[instruction(discriminator = LendingInstruction, variant = Borrow)]
pub struct BorrowInstruction {
	pub collateral_amount: u64,
	pub borrow_amount: u64,
}

#[derive(Accounts, Debug)]
pub struct BorrowAccounts<'a> {
	pub borrower: &'a AccountView,
	pub market: &'a AccountView,
	pub price_feed: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for BorrowAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = BorrowInstruction::try_from_bytes(data)?;

		self.borrower.assert_signer()?;
		self.market.assert_not_empty()?;
		self.price_feed.assert_not_empty()?;

		// The market's designated feed is loaded and then ignored below.
		let _ = self.market.as_account::<Market>(&ID)?;

		// Owner check passes: the feed really is owned by the oracle program.
		let feed = self
			.price_feed
			.as_account::<PriceFeed>(&ORACLE_PROGRAM_ID)?;

		// BUG: The market never checks that this feed is *its* designated
		// oracle (`market.oracle`). Loopscale (April 2025) fell exactly here:
		// the attacker deployed a legitimate-looking feed under the real
		// oracle program pricing a token they controlled, then borrowed
		// against it. The stale `updated_at` field is ignored too, so an
		// attacker can also replay an old extreme price.
		let price = feed.price.get();

		let collateral_value = args
			.collateral_amount
			.get()
			.checked_mul(price)
			.and_then(|value| value.checked_div(PRICE_SCALE))
			.ok_or(ProgramError::ArithmeticOverflow)?;

		let max_borrow = collateral_value
			.checked_mul(MAX_LTV_BPS)
			.and_then(|value| value.checked_div(10_000))
			.ok_or(ProgramError::ArithmeticOverflow)?;

		if args.borrow_amount.get() > max_borrow {
			return Err(ProgramError::InsufficientFunds);
		}

		Ok(())
	}
}
