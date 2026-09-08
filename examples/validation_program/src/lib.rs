//! Complete declarative-validation example for Pina.
//!
//! The program validates each boundary independently:
//!
//! - `#[instruction]` validates client-supplied values after decoding.
//! - `#[derive(Accounts)]` validates the incoming account list after parsing.
//! - `#[account]` validates stored state after initialization and loading.
//! - `#[event]` validates an event before the program can emit or transport it.

#![allow(clippy::inline_always)]
#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("GKYaKKaAJvuzkH2GKkaEFAqESh9NEobZ3V2Ub7qbpVYn");

const POLICY_SEED: &[u8] = b"validation-policy";
const MAX_POLICY_AMOUNT: u64 = 1_000_000;

#[discriminator]
pub enum ValidationInstruction {
	InitializePolicy = 0,
	CheckPolicy = 1,
}

#[discriminator]
pub enum ValidationAccountType {
	PolicyState = 1,
}

#[discriminator]
pub enum ValidationEventType {
	PolicyChecked = 1,
}

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationError {
	/// The lower policy bound must not exceed the upper bound.
	InvalidPolicyRange = 1,
	/// An instruction amount is outside the absolute limits of this program.
	InvalidAmount = 2,
	/// A human-readable memo is too short or too long.
	InvalidMemo = 3,
	/// A check must contain exactly two different approval codes.
	InvalidApprovals = 4,
	/// The validated account list violates a relationship between accounts.
	InvalidAccounts = 5,
	/// The amount does not fall inside the bounds stored in the policy account.
	AmountOutsidePolicy = 6,
	/// The event would describe an invalid policy check.
	InvalidEvent = 7,
}

#[account(
	discriminator = ValidationAccountType::PolicyState,
	validate(with = validate_policy_state)
)]
#[pda(seeds = [POLICY_SEED, authority: Address], bump = bump)]
pub struct PolicyState {
	pub bump: u8,

	#[pina(validate(min = 1, max = MAX_POLICY_AMOUNT))]
	pub minimum: u64,

	#[pina(validate(min = 1, max = MAX_POLICY_AMOUNT))]
	pub maximum: u64,

	#[pina(validate(min = 1, max = 4))]
	pub required_approvals: u8,
}

fn validate_policy_state(value: &PolicyStateZc) -> ProgramResult {
	if value.minimum() > value.maximum() {
		return Err(ValidationError::InvalidPolicyRange.into());
	}

	Ok(())
}

#[instruction(
	discriminator = ValidationInstruction::InitializePolicy,
	validate(with = validate_initialize_policy)
)]
pub struct InitializePolicyInstruction {
	pub bump: u8,

	#[pina(validate(
		min = 1,
		max = MAX_POLICY_AMOUNT,
		error = ValidationError::InvalidAmount
	))]
	pub minimum: u64,

	#[pina(validate(
		min = 1,
		max = MAX_POLICY_AMOUNT,
		error = ValidationError::InvalidAmount
	))]
	pub maximum: u64,

	#[pina(validate(min = 1, max = 4, error = ValidationError::InvalidApprovals))]
	pub required_approvals: u8,
}

fn validate_initialize_policy(value: &InitializePolicyInstructionZc) -> ProgramResult {
	if value.minimum() > value.maximum() {
		return Err(ValidationError::InvalidPolicyRange.into());
	}

	Ok(())
}

#[instruction(
	discriminator = ValidationInstruction::CheckPolicy,
	validate(with = validate_check_policy)
)]
pub struct CheckPolicyInstruction {
	#[pina(validate(
		min = 1,
		max = MAX_POLICY_AMOUNT,
		error = ValidationError::InvalidAmount
	))]
	pub amount: u64,

	#[pina(validate(
		min_len = 3,
		max_len = 64,
		error = ValidationError::InvalidMemo
	))]
	pub memo: String<64>,

	#[pina(validate(exact_len = 2, error = ValidationError::InvalidApprovals))]
	pub approvals: Vec<u8, 4>,
}

fn validate_check_policy(value: &CheckPolicyInstructionZc) -> ProgramResult {
	let approvals = value.approvals();
	if approvals[0] == approvals[1] {
		return Err(ValidationError::InvalidApprovals.into());
	}

	Ok(())
}

#[event(
	discriminator = ValidationEventType::PolicyChecked,
	validate(with = validate_policy_checked)
)]
#[derive(Debug)]
pub struct PolicyChecked {
	#[pina(validate(
		min = 1,
		max = MAX_POLICY_AMOUNT,
		error = ValidationError::InvalidEvent
	))]
	pub amount: u64,

	#[pina(validate(
		min_len = 3,
		max_len = 64,
		error = ValidationError::InvalidEvent
	))]
	pub memo: String<64>,

	#[pina(validate(exact_len = 2, error = ValidationError::InvalidEvent))]
	pub approvals: Vec<u8, 4>,

	#[pina(validate(min = 1, max = 4, error = ValidationError::InvalidEvent))]
	pub required_approvals: u8,
}

fn validate_policy_checked(value: &PolicyCheckedZc) -> ProgramResult {
	let approvals = value.approvals();
	if approvals[0] == approvals[1] {
		return Err(ValidationError::InvalidEvent.into());
	}

	Ok(())
}

#[derive(Accounts, Debug)]
#[pina(validate(with = validate_initialize_accounts))]
pub struct InitializeAccounts<'a> {
	#[pina(validate(signer))]
	pub authority: &'a mut AccountView,

	#[pina(validate(empty, distinct_from = authority))]
	pub policy: &'a mut AccountView,

	#[pina(validate(program = system::ID))]
	pub system_program: &'a AccountView,
}

fn validate_initialize_accounts(accounts: &InitializeAccounts<'_>) -> ProgramResult {
	if accounts.authority.address() == accounts.system_program.address() {
		return Err(ValidationError::InvalidAccounts.into());
	}

	Ok(())
}

#[derive(Accounts, Debug)]
#[pina(validate(with = validate_check_accounts))]
pub struct CheckAccounts<'a> {
	#[pina(validate(signer))]
	pub authority: &'a AccountView,

	#[pina(validate(owner = ID, not_empty, distinct_from = authority))]
	pub policy: &'a AccountView,

	/// A shared account may still require the transaction's writable flag.
	#[pina(validate(writable, distinct_from = policy))]
	pub audit: &'a AccountView,

	#[pina(validate(program = system::ID))]
	pub system_program: &'a AccountView,
}

fn validate_check_accounts(accounts: &CheckAccounts<'_>) -> ProgramResult {
	if accounts.audit.address() == accounts.system_program.address() {
		return Err(ValidationError::InvalidAccounts.into());
	}

	Ok(())
}

impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		// Decoding runs every instruction field rule and the cross-field hook.
		let args = InitializePolicyInstruction::try_from_bytes(data)?;
		let authority = *self.authority.address();
		let seeds = PolicyState::seeds(&authority);
		let seeds_with_bump = seeds.with_bump(args.bump);

		// PDA derivation creates state, so it remains explicit lifecycle logic.
		let canonical_bump = self.policy.assert_canonical_bump(&seeds.as_slices(), &ID)?;
		if canonical_bump != args.bump {
			return Err(ProgramError::InvalidSeeds);
		}
		self.policy
			.assert_seeds_with_bump(&seeds_with_bump.as_slices(), &ID)?;

		CreateProgramAccountWithBump {
			account: self.policy,
			payer: self.authority,
			owner: &ID,
			seeds: &seeds.as_slices(),
			bump: args.bump,
		}
		.invoke_with::<PolicyState>(|policy| {
			policy.bump = args.bump;
			policy.minimum = args.minimum;
			policy.maximum = args.maximum;
			policy.required_approvals = args.required_approvals;
			Ok(())
		})?;

		log!("Validated policy initialized");
		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for CheckAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = CheckPolicyInstruction::try_from_bytes(data)?;
		let policy = PolicyState::load_pda(self.policy, self.authority.address(), &ID)?;
		let amount = args.amount();

		// This rule depends on both instruction data and loaded account state.
		// Keeping it explicit makes the boundary between generated and business
		// validation visible.
		if amount < policy.minimum() || amount > policy.maximum() {
			return Err(ValidationError::AmountOutsidePolicy.into());
		}

		let mut event_bytes = [0u8; PolicyChecked::SIZE];
		PolicyChecked::initialize(&mut event_bytes, |event| {
			event.amount = args.amount;
			event.memo = args.memo;
			event.approvals = args.approvals;
			event.required_approvals = policy.required_approvals;
			Ok(())
		})?;

		log!("Validated policy check accepted");
		Ok(())
	}
}

#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use super::*;

	nostd_entrypoint!(process_instruction);

	#[inline(always)]
	pub fn process_instruction(
		program_id: &Address,
		accounts: &mut [AccountView],
		data: &[u8],
	) -> ProgramResult {
		let instruction: ValidationInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			ValidationInstruction::InitializePolicy => {
				InitializeAccounts::try_from((program_id, accounts))?.process(data)
			}
			ValidationInstruction::CheckPolicy => {
				CheckAccounts::try_from((program_id, accounts))?.process(data)
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn account_state_validation_checks_fields_and_cross_field_rules() {
		let mut bytes = [0u8; PolicyState::SIZE];
		let state = PolicyState::initialize(&mut bytes, |state| {
			state.bump = 254;
			state.minimum.set(100);
			state.maximum.set(1_000);
			state.required_approvals = 2;
			Ok(())
		})
		.unwrap();

		state.validate().unwrap();

		let error = PolicyState::initialize(&mut bytes, |state| {
			state.bump = 254;
			state.minimum.set(1_000);
			state.maximum.set(100);
			state.required_approvals = 2;
			Ok(())
		})
		.err()
		.unwrap();
		assert_eq!(error, ValidationError::InvalidPolicyRange.into());
	}

	#[test]
	fn instruction_validation_rejects_invalid_bounds_and_payloads() {
		let mut initialize_bytes = [0u8; InitializePolicyInstruction::SIZE];
		let error = InitializePolicyInstruction::initialize(&mut initialize_bytes, |args| {
			args.bump = 254;
			args.minimum.set(1_000);
			args.maximum.set(100);
			args.required_approvals = 2;
			Ok(())
		})
		.err()
		.unwrap();
		assert_eq!(error, ValidationError::InvalidPolicyRange.into());

		let mut check_bytes = [0u8; CheckPolicyInstruction::SIZE];
		let error = CheckPolicyInstruction::initialize(&mut check_bytes, |args| {
			args.amount.set(500);
			args.memo.try_set("ok")?;
			args.approvals.try_set([1, 2])?;
			Ok(())
		})
		.err()
		.unwrap();
		assert_eq!(error, ValidationError::InvalidMemo.into());

		let error = CheckPolicyInstruction::initialize(&mut check_bytes, |args| {
			args.amount.set(500);
			args.memo.try_set("reviewed")?;
			args.approvals.try_set([1, 1])?;
			Ok(())
		})
		.err()
		.unwrap();
		assert_eq!(error, ValidationError::InvalidApprovals.into());
	}

	#[test]
	fn event_validation_runs_before_the_event_is_available() {
		let mut bytes = [0u8; PolicyChecked::SIZE];
		let event = PolicyChecked::initialize(&mut bytes, |event| {
			event.amount.set(500);
			event.memo.try_set("reviewed")?;
			event.approvals.try_set([1, 2])?;
			event.required_approvals = 2;
			Ok(())
		})
		.unwrap();
		event.validate().unwrap();

		let error = PolicyChecked::initialize(&mut bytes, |event| {
			event.amount.set(500);
			event.memo.try_set("reviewed")?;
			event.approvals.try_set([1, 2])?;
			event.required_approvals = 0;
			Ok(())
		})
		.err()
		.unwrap();
		assert_eq!(error, ValidationError::InvalidEvent.into());
	}
}
