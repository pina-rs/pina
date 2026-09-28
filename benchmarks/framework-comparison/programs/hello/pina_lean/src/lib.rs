//! Lean-entrypoint variant of the hello comparison.
//!
//! Identical program semantics to `hello/pina` with exactly one difference:
//! the entrypoint's account budget. The stock fixture uses the default 255
//! accounts, so its entrypoint carries unrolled account-walking code for 255
//! accounts. This program accepts at most 1 — its only instruction uses one
//! account — and measures 4,680 → 2,736 bytes (−41.5%) at +6 compute units.
//!
//! This fixture is the measurement bed for the lean-entrypoint exploration
//! (`docs/src/adrs/0010-lean-entrypoint-strategy.md`); it is not part of the
//! published comparison table, which shows the stock default.
#![no_std]

use pina::*;

declare_id!("DCF5KBmtQ9ryDC7mQezKLwuJHem6coVUCmKkw37M9J4A");

#[discriminator]
pub enum HelloInstruction {
	Hello = 0,
}

#[instruction(discriminator = HelloInstruction::Hello)]
pub struct HelloInstructionData {}

#[derive(Accounts, Debug)]
pub struct HelloAccounts<'a> {
	pub user: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for HelloAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = HelloInstructionData::try_from_bytes(data)?;
		self.user.assert_signer()?;
		log!("Hello, Solana!");
		Ok(())
	}
}

nostd_entrypoint!(process_instruction, 1);

#[inline(always)]
pub fn process_instruction(
	program_id: &Address,
	accounts: &mut [AccountView],
	data: &[u8],
) -> ProgramResult {
	let instruction: HelloInstruction = parse_instruction(program_id, &ID, data)?;

	match instruction {
		HelloInstruction::Hello => HelloAccounts::try_from((program_id, accounts))?.process(data),
	}
}
