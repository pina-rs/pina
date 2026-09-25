//! Downstream re-export verification: a consumer with only a `pina`
//! dependency gets the complete fixed-point schema stack.
//!
//! - `pina::pinapod` re-exports the pinned pinapod, so the closed `account`
//!   grammar and the pod types are nameable without a direct pinapod
//!   dependency.
//! - `pina::fixed` re-exports the exact `fixed` instance pinapod's
//!   `ZcField` implementations are written for, so schema fields derive
//!   against the only build that can work.

use pina::account;
use pina::discriminator;
use pina::fixed::FixedI64;
use pina::fixed::FixedU64;
use pina::fixed::types::extra::U3;
use pina::fixed::types::extra::U16;
use pina::*;

#[discriminator(crate = ::pina, primitive = u8, final)]
enum LedgerKind {
	Ledger = 21,
}

#[account(crate = ::pina, discriminator = LedgerKind, variant = Ledger)]
struct Ledger {
	pub price: FixedU64<U16>,
	pub fee: FixedI64<U3>,
}

#[test]
fn fixed_schema_fields_round_trip_through_the_pina_reexports() {
	let mut bytes = [0u8; Ledger::SIZE];
	Ledger::initialize(&mut bytes, |ledger| {
		ledger.price.set(42 << 16);
		ledger.fee.set(3 << 3);
		Ok(())
	})
	.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));

	let view = Ledger::try_from_bytes(&bytes)
		.unwrap_or_else(|error| panic!("validation failed: {error:?}"));

	assert_eq!(view.price.get(), 42 << 16);
	assert_eq!(
		FixedU64::<U16>::from_bits(view.price.get()).to_num::<u32>(),
		42
	);
	assert_eq!(FixedI64::<U3>::from_bits(view.fee.get()).to_num::<i32>(), 3);
}
