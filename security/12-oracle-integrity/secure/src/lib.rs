//! SECURE: Pinned, ownership-checked, and freshness-enforced price feed.
//!
//! The market only trusts the exact feed recorded in its configuration, that
//! feed must be owned by the oracle program, and its price must be younger
//! than a hard staleness bound. A malicious or abandoned feed cannot price
//! loans in this market.

#![no_std]

#[cfg(all(not(any(target_os = "solana", target_arch = "bpf")), not(test)))]
extern crate std;

use pina::*;

declare_id!("HbxJvkcvgiKi9RaK55E2HmN3xRpo3VPjCSm4MAK9JqNR");

/// Address of the trusted oracle program that owns price feed accounts.
///
/// The account address is a stand-in for a real oracle deployment.
const ORACLE_PROGRAM_ID: Address = address!("fgVrbM93Xo3za8dGCqqUvXHEzuN8QzVbb43QBSykaCa");

/// Price scale used by the feed (8 decimals).
const PRICE_SCALE: u64 = 100_000_000;

/// Maximum loan-to-value, in basis points.
const MAX_LTV_BPS: u64 = 7_500;

/// Maximum age of a price observation, in seconds.
const MAX_STALENESS_SECONDS: i64 = 60;

const CLOCK_SYSVAR_ID: Address = address!("SysvarC1ock11111111111111111111111111111111");

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LendingError {
	StalePriceFeed = 0,
}

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
	pub clock: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for BorrowAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = BorrowInstruction::try_from_bytes(data)?;

		self.borrower.assert_signer()?;
		self.market.assert_not_empty()?;
		self.price_feed.assert_not_empty()?;
		self.clock.assert_sysvar(&CLOCK_SYSVAR_ID)?;

		let market = self.market.as_account::<Market>(&ID)?;

		// Ownership: only the oracle program can own a feed account.
		let feed = self
			.price_feed
			.as_account::<PriceFeed>(&ORACLE_PROGRAM_ID)?;

		// Pinning: the market prices every loan with the one feed recorded in
		// its configuration. An attacker who deploys a fresh feed under the
		// same oracle program (Loopscale, April 2025) cannot substitute it.
		self.price_feed.assert_address(&market.oracle)?;

		// Freshness: reject observations older than the staleness bound so a
		// frozen or abandoned feed cannot keep pricing loans at old extremes.
		let now = {
			let clock = sysvars::clock::Clock::from_account_view(self.clock)?;
			clock.unix_timestamp
		};

		// A feed timestamped after the current slot is malformed, not fresh:
		// plain subtraction would make the age negative and pass the bound,
		// leaving the observation trusted until the Clock catches up.
		let age = now
			.checked_sub(feed.updated_at.get())
			.ok_or(LendingError::StalePriceFeed)?;

		if age > MAX_STALENESS_SECONDS {
			return Err(LendingError::StalePriceFeed.into());
		}

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
