//! Scenario tests for the diagnosis, the report, and the RPC transport.

use std::collections::HashMap;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use serde_json::Value;
use serde_json::json;

use super::diagnose::CurrentState;
use super::diagnose::Diagnosis;
use super::diagnose::Failure;
use super::rpc::AccountState;
use super::source::SourceIndex;
use super::transaction::InstructionError;
use super::transaction::MessageAccount;
use super::transaction::MessageInstruction;
use super::transaction::Outcome;
use super::transaction::Transaction;
use super::*;
use crate::ir::ProgramIr;
use crate::parse::module_resolver::ResolvedFile;

const PROGRAM_ID: &str = "GKYaKKaAJvuzkH2GKkaEFAqESh9NEobZ3V2Ub7qbpVYn";
const SYSTEM: &str = "11111111111111111111111111111111";
const TOKEN: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const CLOCK: &str = "SysvarC1ock11111111111111111111111111111111";
const SYSVAR_OWNER: &str = "Sysvar1111111111111111111111111111111111111";

const FIXTURE: &str = r#"
use pina::*;

declare_id!("GKYaKKaAJvuzkH2GKkaEFAqESh9NEobZ3V2Ub7qbpVYn");

#[discriminator]
pub enum FixtureInstruction {
	Rules = 0,
	Optional = 1,
	Remaining = 2,
	Nested = 3,
	Opaque = 4,
	Unrouted = 5,
	Loose = 6,
	Shared = 7,
	Wrapped = 8,
	Chained = 9,
}

#[error]
pub enum FixtureError {
	/// The caller may not do this.
	Denied = 6000,
	Other,
}

#[instruction(discriminator = FixtureInstruction::Rules, validate(with = validate_rules_args))]
pub struct RulesInstruction {
	#[pina(validate(value >= 1, error = FixtureError::Denied))]
	pub amount: u64,
	#[pina(validate(value <= 9))]
	pub limit: u64,
}

#[instruction(discriminator = FixtureInstruction::Optional)]
pub struct OptionalInstruction {}

#[instruction(discriminator = FixtureInstruction::Remaining)]
pub struct RemainingInstruction {}

#[instruction(discriminator = FixtureInstruction::Nested)]
pub struct NestedInstruction {}

#[instruction(discriminator = FixtureInstruction::Opaque)]
pub struct OpaqueInstruction {}

#[instruction(discriminator = FixtureInstruction::Unrouted)]
pub struct UnroutedInstruction {}

#[instruction(discriminator = FixtureInstruction::Loose)]
pub struct LooseInstruction {}

#[instruction(discriminator = FixtureInstruction::Shared)]
pub struct SharedInstruction {}

#[instruction(discriminator = FixtureInstruction::Wrapped)]
pub struct WrappedInstruction {}

#[derive(Accounts)]
#[pina(validate(with = validate_rules))]
pub struct RulesAccounts<'a> {
	#[pina(validate(signer, executable))]
	pub authority: &'a AccountView,
	#[pina(validate(address = system::ID))]
	pub fixed: &'a AccountView,
	#[pina(validate(addresses = [ID, token::ID]))]
	pub either: &'a AccountView,
	#[pina(validate(owner = ID, not_empty, data_len = 8))]
	pub state: &'a mut AccountView,
	#[pina(validate(owners = &[ID]), validate(empty, error = FixtureError::Denied))]
	pub fresh: &'a AccountView,
	#[pina(validate(sysvar = sysvars::clock::ID))]
	pub clock: &'a AccountView,
	#[pina(validate(program = system::ID))]
	pub system_program: &'a AccountView,
	#[pina(validate(address = ADMIN, data_len = SIZE, error = ProgramError::InvalidArgument))]
	pub admin: &'a AccountView,
	#[pina(validate(owner = OTHER_OWNER, error = DENIED))]
	pub mystery: &'a AccountView,
	#[pina(validate(writable, distinct_from = authority, error = PinaProgramError::DataTooShort))]
	pub audit: &'a AccountView,
}

#[derive(Accounts)]
pub struct OptionalAccounts<'a> {
	pub payer: &'a mut AccountView,
	#[pina(validate(signer, distinct_from = payer))]
	pub witness: Option<&'a AccountView>,
	pub vault: Option<&'a mut AccountView>,
}

#[derive(Accounts)]
pub struct RemainingAccounts<'a> {
	pub authority: &'a AccountView,
	#[pina(remaining)]
	pub members: &'a mut [AccountView],
}

#[derive(Accounts)]
pub struct LooseAccounts<'a> {
	#[pina(remaining, distinct = false)]
	pub members: &'a mut [AccountView],
}

#[derive(Accounts)]
pub struct SharedAccounts<'a> {
	#[pina(remaining, distinct)]
	pub members: &'a [AccountView],
}

#[derive(Accounts)]
pub struct NestedAccounts<'a> {
	pub payer: &'a mut AccountView,
	pub inner: InnerAccounts<'a>,
}

#[derive(Accounts)]
pub struct InnerAccounts<'a> {
	#[pina(validate(signer))]
	pub owner: &'a AccountView,
}

#[derive(Accounts)]
pub struct OpaqueAccounts<'a> {
	pub payer: &'a AccountView,
	pub external: external::Accounts<'a>,
	pub after: &'a AccountView,
}

#[derive(Accounts)]
pub struct WrappedAccounts<'a> {
	pub inner: OpaqueInner<'a>,
	pub after: &'a AccountView,
}

#[derive(Accounts)]
pub struct OpaqueInner<'a> {
	pub external: external::Accounts<'a>,
}

#[instruction(discriminator = FixtureInstruction::Chained)]
pub struct ChainedInstruction {}

#[derive(Accounts)]
pub struct ChainedAccounts<'a> {
	pub vault: &'a AccountView,
	pub ledger: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for ChainedAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		self.vault.assert_data_len(8)?.assert_writable()?;
		self.ledger.assert_writable()?.assert_data_len(8)?;
		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for RulesAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _args = RulesInstruction::try_from_bytes(data)?;
		self.authority.assert_signer()?;
		self.audit.assert_writable()?;
		self.clock.assert_executable()?;
		self.fresh.assert_empty()?;
		self.state.assert_not_empty()?;
		self.state.assert_owner(&ID)?;
		let _ = self.state.address();
		self.ghost.assert_signer()?;
		if self.authority.address() == self.fixed.address() {
			return Err(FixtureError::Other.into());
		}
		Err(ProgramError::InvalidArgument)
	}
}

impl<'a> ProcessAccountInfos<'a> for OptionalAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		self.witness.assert_signer()?;
		Ok(())
	}
}

fn validate_rules(_accounts: &RulesAccounts<'_>) -> ProgramResult {
	Err(FixtureError::Denied.into())
}

fn validate_rules_args(_args: &RulesInstructionZc) -> ProgramResult {
	Err(ProgramError::InvalidInstructionData)
}

fn unrelated() -> ProgramResult {
	Err(FixtureError::Denied.into())
}

pub mod helpers {
	pub fn nested_unrelated() -> ProgramResult {
		Err(PinaProgramError::DataTooShort.into())
	}
}

pub fn process_instruction(
	program_id: &Address,
	accounts: &mut [AccountView],
	data: &[u8],
) -> ProgramResult {
	let instruction: FixtureInstruction = parse_instruction(program_id, &ID, data)?;

	match instruction {
		FixtureInstruction::Rules => RulesAccounts::try_from((program_id, accounts))?.process(data),
		FixtureInstruction::Optional => OptionalAccounts::try_from((program_id, accounts))?.process(data),
		FixtureInstruction::Remaining => RemainingAccounts::try_from((program_id, accounts))?.process(data),
		FixtureInstruction::Nested => NestedAccounts::try_from((program_id, accounts))?.process(data),
		FixtureInstruction::Opaque => OpaqueAccounts::try_from((program_id, accounts))?.process(data),
		FixtureInstruction::Unrouted => Ok(()),
		FixtureInstruction::Loose => LooseAccounts::try_from((program_id, accounts))?.process(data),
		FixtureInstruction::Shared => SharedAccounts::try_from((program_id, accounts))?.process(data),
		FixtureInstruction::Wrapped => WrappedAccounts::try_from((program_id, accounts))?.process(data),
		FixtureInstruction::Chained => ChainedAccounts::try_from((program_id, accounts))?.process(data),
	}
}
"#;

fn fixture() -> (ProgramIr, SourceIndex) {
	let file = syn::parse_file(FIXTURE).unwrap_or_else(|error| panic!("fixture parses: {error}"));
	let program = crate::parse::assemble_program_ir(&file, "fixture")
		.unwrap_or_else(|error| panic!("fixture assembles: {error}"));
	let index = source::index(
		vec![ResolvedFile {
			path: PathBuf::from("/project/src/lib.rs"),
			file,
		}],
		Path::new("/project"),
	)
	.unwrap_or_else(|error| panic!("fixture indexes: {error}"));

	(program, index)
}

/// A one-instruction transaction to the fixture program. Each key is
/// `(address, signer, writable)`; the program key is appended last.
fn tx(
	keys: &[(&str, bool, bool)],
	accounts: &[usize],
	data: &[u8],
	outcome: Outcome,
) -> Transaction {
	let mut all: Vec<MessageAccount> = keys
		.iter()
		.map(|(address, signer, writable)| {
			MessageAccount {
				address: (*address).to_owned(),
				signer: *signer,
				writable: *writable,
			}
		})
		.collect();
	all.push(MessageAccount {
		address: PROGRAM_ID.to_owned(),
		signer: false,
		writable: false,
	});

	Transaction {
		signature: "sig".to_owned(),
		slot: 9,
		accounts: all,
		instructions: vec![MessageInstruction {
			program_id: PROGRAM_ID.to_owned(),
			accounts: accounts.to_vec(),
			data: data.to_vec(),
		}],
		outcome,
		logs: Some(Vec::new()),
	}
}

fn failed(error: InstructionError) -> Outcome {
	Outcome::InstructionFailed { index: 0, error }
}

fn builtin(name: &str) -> InstructionError {
	InstructionError::Builtin(name.to_owned())
}

fn diagnose(
	transaction: &Transaction,
	error: &InstructionError,
	messages: &[String],
	state: CurrentState<'_>,
) -> Diagnosis {
	let (program, index) = fixture();

	diagnose::diagnose(
		&program,
		&index,
		&Failure {
			transaction,
			instruction: &transaction.instructions[0],
			error,
			messages,
			state,
		},
	)
}

fn state(owner: &str, executable: bool, data_len: Option<u64>) -> AccountState {
	AccountState {
		owner: owner.to_owned(),
		executable,
		data_len,
	}
}

fn summary(diagnosis: &Diagnosis) -> Vec<(Option<String>, String, Confidence, bool)> {
	diagnosis
		.candidates
		.iter()
		.map(|candidate| {
			(
				candidate.field.clone(),
				candidate.rule.clone(),
				candidate.confidence,
				candidate.log_confirmed,
			)
		})
		.collect()
}

fn entry(
	field: &str,
	rule: &str,
	confidence: Confidence,
	log: bool,
) -> (Option<String>, String, Confidence, bool) {
	(Some(field.to_owned()), rule.to_owned(), confidence, log)
}

/// Keys for the rules instruction: every check passes except where a test
/// changes a key.
fn rules_keys() -> Vec<(&'static str, bool, bool)> {
	vec![
		("Authority", true, false),
		(SYSTEM, false, false),
		(TOKEN, false, false),
		("State", false, true),
		("Fresh", false, false),
		(CLOCK, false, false),
		("Admin", false, false),
		("Mystery", false, false),
		("Audit", false, true),
	]
}

/// Instruction account indices for the rules instruction; `system_program`
/// reuses the `fixed` key.
const RULES_ACCOUNTS: [usize; 10] = [0, 1, 2, 3, 4, 5, 1, 6, 7, 8];

fn passing_states() -> HashMap<String, AccountState> {
	HashMap::from([
		("Authority".to_owned(), state(SYSTEM, true, Some(0))),
		(
			SYSTEM.to_owned(),
			state(
				"NativeLoader1111111111111111111111111111111",
				true,
				Some(14),
			),
		),
		(TOKEN.to_owned(), state(SYSTEM, true, Some(0))),
		("State".to_owned(), state(PROGRAM_ID, false, Some(8))),
		("Fresh".to_owned(), state(PROGRAM_ID, false, Some(0))),
		(CLOCK.to_owned(), state(SYSVAR_OWNER, false, Some(40))),
		("Admin".to_owned(), state(SYSTEM, false, Some(0))),
		("Mystery".to_owned(), state(SYSTEM, false, Some(0))),
		("Audit".to_owned(), state(SYSTEM, false, Some(0))),
	])
}

#[test]
fn a_proven_failure_hides_the_checks_that_never_ran() {
	let mut keys = rules_keys();
	keys[1] = ("NotSystem", false, false);
	let error = builtin("InvalidAccountData");
	let transaction = tx(&keys, &RULES_ACCOUNTS, &[0], failed(error.clone()));
	let mut states = passing_states();
	states.insert("Authority".to_owned(), state(SYSTEM, false, Some(0)));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::Fetched(&states));

	assert_eq!(
		summary(&diagnosis),
		[
			entry(
				"fixed",
				"address = system::ID",
				Confidence::Confirmed,
				false
			),
			entry(
				"authority",
				"executable",
				Confidence::CheckedAgainstCurrentState,
				false
			),
		]
	);
	assert_eq!(
		diagnosis.candidates[0].reason,
		"account #1 is NotSystem, expected 11111111111111111111111111111111"
	);
	assert_eq!(
		diagnosis.candidates[0].location.as_deref(),
		Some("src/lib.rs:64")
	);
	assert_eq!(
		diagnosis.instruction,
		Some(InstructionSummary {
			name: "rules".to_owned(),
			rust_name: "RulesInstruction".to_owned(),
			accounts_struct: Some("RulesAccounts".to_owned()),
		})
	);
	assert_eq!(
		diagnosis.accounts[6].field.as_deref(),
		Some("system_program")
	);
}

#[test]
fn a_matching_log_line_ranks_its_check_first() {
	let mut keys = rules_keys();
	keys[1] = ("NotSystem", false, false);
	keys[8] = ("Audit", false, false);
	let error = builtin("InvalidAccountData");
	let transaction = tx(&keys, &RULES_ACCOUNTS, &[0], failed(error.clone()));
	let messages = vec!["account address is invalid".to_owned()];
	let diagnosis = diagnose(&transaction, &error, &messages, CurrentState::NotRequested);

	// `audit`'s writable rule overrides its error, so it cannot have returned
	// `InvalidAccountData`; the executable rule needs state.
	assert_eq!(
		summary(&diagnosis),
		[
			entry("fixed", "address = system::ID", Confidence::Confirmed, true),
			entry("authority", "executable", Confidence::Possible, false),
		]
	);
	assert!(
		diagnosis.candidates[1]
			.reason
			.contains("pass --network or --rpc-url")
	);
}

#[test]
fn state_checks_report_what_the_current_state_shows() {
	let keys = rules_keys();
	let transaction = tx(
		&keys,
		&RULES_ACCOUNTS,
		&[0],
		failed(builtin("InvalidAccountOwner")),
	);
	let mut states = passing_states();
	states.insert("State".to_owned(), state(SYSTEM, false, Some(8)));
	states.insert(CLOCK.to_owned(), state(SYSTEM, false, Some(40)));
	let error = builtin("InvalidAccountOwner");
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::Fetched(&states));

	assert_eq!(
		summary(&diagnosis),
		[
			entry(
				"state",
				"owner = ID",
				Confidence::CheckedAgainstCurrentState,
				false
			),
			entry(
				"clock",
				"sysvar = sysvars::clock::ID",
				Confidence::CheckedAgainstCurrentState,
				false
			),
			entry("fresh", "owners = &[ID]", Confidence::Possible, false),
			entry("state", "assert_owner()", Confidence::Possible, false),
		]
	);
	assert_eq!(
		diagnosis.candidates[0].reason,
		"account #3 (State) is currently owned by 11111111111111111111111111111111, expected \
		 GKYaKKaAJvuzkH2GKkaEFAqESh9NEobZ3V2Ub7qbpVYn"
	);
	assert!(
		diagnosis.candidates[2]
			.reason
			.contains("passes against the current account state")
	);
}

#[test]
fn data_rules_read_lengths_from_current_state() {
	let keys = rules_keys();
	let mut states = passing_states();
	states.insert("State".to_owned(), state(PROGRAM_ID, false, Some(0)));
	states.insert("Fresh".to_owned(), state(PROGRAM_ID, false, Some(3)));

	let error = builtin("UninitializedAccount");
	let transaction = tx(&keys, &RULES_ACCOUNTS, &[0], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::Fetched(&states));
	assert_eq!(
		summary(&diagnosis),
		[
			entry(
				"state",
				"not_empty",
				Confidence::CheckedAgainstCurrentState,
				false
			),
			entry(
				"state",
				"assert_not_empty()",
				Confidence::CheckedAgainstCurrentState,
				false
			),
		]
	);
	assert_eq!(
		diagnosis.candidates[0].reason,
		"account #3 (State) currently holds no data"
	);

	let error = builtin("AccountAlreadyInitialized");
	let transaction = tx(&keys, &RULES_ACCOUNTS, &[0], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::Fetched(&states));
	assert_eq!(
		summary(&diagnosis),
		[entry(
			"fresh",
			"assert_empty()",
			Confidence::CheckedAgainstCurrentState,
			false
		)]
	);
	assert_eq!(
		diagnosis.candidates[0].reason,
		"account #4 (Fresh) currently holds 3 bytes"
	);

	let error = InstructionError::Custom(6000);
	let transaction = tx(&keys, &RULES_ACCOUNTS, &[0], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::Fetched(&states));
	assert_eq!(
		summary(&diagnosis),
		[
			entry(
				"fresh",
				"empty",
				Confidence::CheckedAgainstCurrentState,
				false
			),
			entry(
				"mystery",
				"owner = OTHER_OWNER",
				Confidence::Possible,
				false
			),
			entry("amount", "value >= 1", Confidence::Possible, false),
		]
	);

	let error = builtin("InvalidAccountData");
	let mut sized = passing_states();
	sized.insert("State".to_owned(), state(PROGRAM_ID, false, Some(9)));
	let transaction = tx(&keys, &RULES_ACCOUNTS, &[0], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::Fetched(&sized));
	assert_eq!(
		diagnosis.candidates[0].reason,
		"account #3 (State) currently holds 9 bytes, expected 8"
	);
}

#[test]
fn unknown_lengths_and_missing_state_stay_possible() {
	let keys = rules_keys();
	let error = builtin("UninitializedAccount");
	let transaction = tx(&keys, &RULES_ACCOUNTS, &[0], failed(error.clone()));
	let mut states = passing_states();
	states.insert("State".to_owned(), state(PROGRAM_ID, false, None));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::Fetched(&states));
	assert!(
		diagnosis.candidates[0]
			.reason
			.contains("did not report the account's data length")
	);

	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::Unavailable);
	assert!(
		diagnosis.candidates[0]
			.reason
			.contains("could not be fetched")
	);

	let empty = HashMap::new();
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::Fetched(&empty));
	assert!(diagnosis.candidates[0].reason.contains("was not fetched"));
}

#[test]
fn overridden_errors_are_matched_by_their_own_code() {
	let mut keys = rules_keys();
	keys[6] = ("Imposter", false, false);
	let error = builtin("InvalidArgument");
	let transaction = tx(&keys, &RULES_ACCOUNTS, &[0], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);
	assert_eq!(
		summary(&diagnosis),
		[
			entry("admin", "address = ADMIN", Confidence::Possible, false),
			entry("admin", "data_len = SIZE", Confidence::Possible, false),
		]
	);
	assert!(
		diagnosis.candidates[0]
			.reason
			.contains("`ADMIN` is not a constant")
	);
	assert!(
		diagnosis.candidates[1]
			.reason
			.contains("`SIZE` is not a literal")
	);

	let mut keys = rules_keys();
	keys[8] = ("Authority", true, false);
	let error = InstructionError::Custom(0xFFFF_FFFA);
	let transaction = tx(
		&keys,
		&[0, 1, 2, 3, 4, 5, 1, 6, 7, 0],
		&[0],
		failed(error.clone()),
	);
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);
	assert_eq!(
		summary(&diagnosis),
		[entry("audit", "writable", Confidence::Confirmed, false)]
	);

	// An override that names no known error can match any custom code.
	let error = InstructionError::Custom(77);
	let transaction = tx(&rules_keys(), &RULES_ACCOUNTS, &[0], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);
	assert_eq!(
		summary(&diagnosis),
		[entry(
			"mystery",
			"owner = OTHER_OWNER",
			Confidence::Possible,
			false
		)]
	);
	assert!(
		diagnosis.candidates[0]
			.reason
			.contains("`OTHER_OWNER` is not a constant")
	);
}

#[test]
fn signer_and_program_checks_use_the_message_flags() {
	let mut keys = rules_keys();
	keys[0] = ("Authority", false, false);
	let error = builtin("MissingRequiredSignature");
	let transaction = tx(&keys, &RULES_ACCOUNTS, &[0], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);
	assert_eq!(
		summary(&diagnosis),
		[entry("authority", "signer", Confidence::Confirmed, false)]
	);

	// The processor's own `assert_signer` is the only candidate once the
	// generated check passes.
	let mut keys = rules_keys();
	keys[0] = ("Authority", true, false);
	let error = builtin("InvalidAccountData");
	let mut accounts = RULES_ACCOUNTS;
	accounts[6] = 9;
	let mut transaction = tx(&keys, &accounts, &[0], failed(error.clone()));
	transaction.accounts[9].address = "NotTheSystemProgram".to_owned();
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);
	assert_eq!(
		summary(&diagnosis)[0],
		entry(
			"system_program",
			"program = system::ID",
			Confidence::Confirmed,
			false
		)
	);
}

#[test]
fn processor_signer_checks_follow_the_generated_ones() {
	let (program, index) = fixture();
	let error = builtin("MissingRequiredSignature");
	let mut keys = rules_keys();
	keys[0] = ("Authority", false, false);
	let transaction = tx(&keys, &RULES_ACCOUNTS, &[0], failed(error.clone()));
	let messages = Vec::new();
	let diagnosis = diagnose::diagnose(
		&program,
		&index,
		&Failure {
			transaction: &transaction,
			instruction: &transaction.instructions[0],
			error: &error,
			messages: &messages,
			state: CurrentState::NotRequested,
		},
	);
	assert_eq!(diagnosis.candidates.len(), 1);

	// With the generated signer rule passing, a processor site can still
	// fail, for example through an aliased account.
	let mut keys = rules_keys();
	keys[0] = ("Authority", true, false);
	let error = builtin("InvalidAccountData");
	keys[8] = ("Audit", false, false);
	let transaction = tx(&keys, &RULES_ACCOUNTS, &[0], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);
	let rules: Vec<_> = diagnosis
		.candidates
		.iter()
		.map(|candidate| candidate.rule.as_str())
		.collect();
	assert!(rules.contains(&"assert_writable()"), "{rules:?}");
}

#[test]
fn optional_slots_are_absent_at_the_end_or_when_they_hold_the_program_id() {
	let error = builtin("MissingRequiredSignature");
	let keys = [("Payer", true, true), ("Witness", false, false)];
	let transaction = tx(&keys, &[0, 1, 2], &[1], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);
	// The processor's own `assert_signer` runs after the generated check, so
	// the proven failure of the generated check hides it.
	assert_eq!(
		summary(&diagnosis),
		[entry("witness", "signer", Confidence::Confirmed, false)]
	);
	assert!(diagnosis.accounts[2].absent);
	assert_eq!(diagnosis.accounts[2].field.as_deref(), Some("vault"));

	// The program-ID filler marks the witness absent, so its rules are skipped.
	let transaction = tx(&keys, &[0, 2], &[1], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);
	assert!(diagnosis.candidates.is_empty());
	assert!(diagnosis.accounts[1].absent);

	// A present mutable optional must be writable and distinct.
	let error = builtin("InvalidAccountData");
	let keys = [("Payer", true, true), ("Vault", false, false)];
	let transaction = tx(&keys, &[0, 2, 1], &[1], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);
	assert_eq!(
		summary(&diagnosis),
		[entry(
			"vault",
			"writable (`&mut` field)",
			Confidence::Confirmed,
			false
		)]
	);

	let error = InstructionError::Custom(0xFFFF_FFF9);
	let transaction = tx(&keys, &[0, 2, 0], &[1], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);
	assert_eq!(
		summary(&diagnosis),
		[entry(
			"payer",
			"distinct mutable account",
			Confidence::Confirmed,
			false
		)]
	);
	assert_eq!(
		diagnosis.candidates[0].reason,
		"account #0 (Payer) is bound to a mutable field and appears again at #2"
	);

	let error = builtin("InvalidAccountData");
	let keys = [("Payer", true, true), ("Witness", true, false)];
	let transaction = tx(&keys, &[0, 0], &[1], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);
	assert_eq!(
		summary(&diagnosis),
		[entry(
			"witness",
			"distinct_from = payer",
			Confidence::Confirmed,
			false
		)]
	);
	let _ = transaction;
}

#[test]
fn account_counts_are_confirmed_from_the_instruction() {
	let error = builtin("NotEnoughAccountKeys");
	let keys = [("Payer", true, true)];
	let transaction = tx(&keys, &[], &[1], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);
	assert_eq!(
		summary(&diagnosis),
		[entry(
			"payer",
			"required account",
			Confidence::Confirmed,
			false
		)]
	);
	assert_eq!(
		diagnosis.candidates[0].reason,
		"the instruction passed 0 account(s), so `OptionalAccounts` has no slot #0 for `payer`"
	);

	let error = InstructionError::Custom(0xFFFF_FFFE);
	let keys = [
		("Payer", true, true),
		("Witness", true, false),
		("Vault", false, true),
		("Extra", false, false),
	];
	let transaction = tx(&keys, &[0, 1, 2, 3], &[1], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);
	assert_eq!(diagnosis.candidates.len(), 1);
	assert_eq!(diagnosis.candidates[0].scope, CandidateScope::Instruction);
	assert_eq!(diagnosis.candidates[0].account_index, Some(3));
	assert_eq!(
		diagnosis.candidates[0].reason,
		"the instruction passed 4 accounts, but `OptionalAccounts` reads at most 3 (extra: #3)"
	);
}

#[test]
fn remaining_slices_check_writability_and_duplicates() {
	let keys = [
		("Authority", false, false),
		("First", false, true),
		("Second", false, false),
	];
	let error = builtin("InvalidAccountData");
	let transaction = tx(&keys, &[0, 1, 2, 1], &[2], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);
	assert_eq!(
		summary(&diagnosis),
		[entry(
			"members[1]",
			"writable (`&mut [AccountView]` remaining slice)",
			Confidence::Confirmed,
			false
		)]
	);
	assert_eq!(diagnosis.accounts[3].field.as_deref(), Some("members[2]"));

	let error = InstructionError::Custom(0xFFFF_FFF9);
	let transaction = tx(&keys, &[0, 1, 2, 1], &[2], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);
	assert_eq!(
		summary(&diagnosis),
		[entry(
			"members[2]",
			"distinct remaining accounts",
			Confidence::Confirmed,
			false
		)]
	);
	assert_eq!(
		diagnosis.candidates[0].reason,
		"remaining account #3 (First) repeats #1"
	);

	// `distinct = false` allows aliases, and a shared slice checks nothing.
	for discriminator in [6, 7] {
		let transaction = tx(&keys, &[1, 1, 2], &[discriminator], failed(error.clone()));
		let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);
		assert!(diagnosis.candidates.is_empty(), "{discriminator}");
	}
}

#[test]
fn nested_structs_bind_their_fields_in_place() {
	let error = builtin("MissingRequiredSignature");
	let keys = [("Payer", true, true), ("Owner", false, false)];
	let transaction = tx(&keys, &[0, 1], &[3], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);
	assert_eq!(
		summary(&diagnosis),
		[entry("inner.owner", "signer", Confidence::Confirmed, false)]
	);
	assert!(diagnosis.flow.types.contains("InnerAccounts"));

	let error = builtin("NotEnoughAccountKeys");
	let transaction = tx(&keys, &[0], &[3], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);
	assert_eq!(
		summary(&diagnosis),
		[entry(
			"inner.owner",
			"required account",
			Confidence::Confirmed,
			false
		)]
	);
}

#[test]
fn an_unknown_nested_struct_stops_naming_accounts() {
	let error = builtin("InvalidAccountData");
	let keys = [("Payer", true, true), ("External", false, false)];
	let transaction = tx(&keys, &[0, 1], &[4], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);

	assert_eq!(diagnosis.accounts[0].field.as_deref(), Some("payer"));
	assert_eq!(diagnosis.accounts[1].field, None);
	assert_eq!(
		diagnosis.notes,
		[
			"`Accounts` is not a `#[derive(Accounts)]` struct of this program, so the accounts from \
		  `external` on are not named."
		]
	);

	// The extra-accounts rule cannot count past an unknown struct.
	let error = InstructionError::Custom(0xFFFF_FFFE);
	let transaction = tx(&keys, &[0, 1, 0, 1], &[4], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);
	assert!(diagnosis.candidates.is_empty());

	// An unknown struct nested deeper also stops the fields after its parent.
	let transaction = tx(&keys, &[1, 0], &[8], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);
	assert_eq!(diagnosis.accounts[0].field, None);
	assert_eq!(diagnosis.accounts[1].field, None);
	assert!(diagnosis.notes[0].contains("from `inner.external` on"));
}

/// The processor for this instruction runs
/// `self.vault.assert_data_len(8)?.assert_writable()?` and then
/// `self.ledger.assert_writable()?.assert_data_len(8)?`.
#[test]
fn chained_processor_checks_follow_execution_order() {
	let error = builtin("InvalidAccountData");
	let rules = |diagnosis: &Diagnosis| {
		diagnosis
			.candidates
			.iter()
			.map(|candidate| {
				(
					candidate.field.clone().unwrap_or_default(),
					candidate.rule.clone(),
					candidate.confidence,
				)
			})
			.collect::<Vec<_>>()
	};
	let entry =
		|field: &str, rule: &str, confidence| (field.to_owned(), rule.to_owned(), confidence);
	let layout = (
		String::new(),
		"`ChainedInstruction` data layout".to_owned(),
		Confidence::Possible,
	);

	// A read-only vault: `assert_data_len` is the earlier link, so its logged
	// failure ranks first and the later `assert_writable` ranks below it.
	let keys = [("Vault", false, false), ("Ledger", false, true)];
	let transaction = tx(&keys, &[0, 1], &[9], failed(error.clone()));
	let messages = vec!["account has an incorrect length".to_owned()];
	let diagnosis = diagnose(&transaction, &error, &messages, CurrentState::NotRequested);
	assert_eq!(
		rules(&diagnosis),
		[
			entry("vault", "assert_data_len()", Confidence::Possible),
			entry("vault", "assert_writable()", Confidence::Confirmed),
			layout.clone(),
		]
	);
	assert!(diagnosis.candidates[0].log_confirmed);

	// Without the log line the earlier link still ran first, so it stays a
	// candidate below the proven failure instead of being dropped.
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);
	assert_eq!(
		rules(&diagnosis),
		[
			entry("vault", "assert_writable()", Confidence::Confirmed),
			layout.clone(),
			entry("vault", "assert_data_len()", Confidence::Possible),
		]
	);

	// A read-only ledger fails its first link, so its later `assert_data_len`
	// never ran and is left out.
	let keys = [("Vault", false, true), ("Ledger", false, false)];
	let transaction = tx(&keys, &[0, 1], &[9], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);
	assert_eq!(
		rules(&diagnosis),
		[
			entry("ledger", "assert_writable()", Confidence::Confirmed),
			layout,
			entry("vault", "assert_data_len()", Confidence::Possible),
		]
	);
}

#[test]
fn an_unrouted_instruction_has_no_named_accounts() {
	let error = builtin("InvalidAccountData");
	let keys = [("Payer", true, true)];
	let transaction = tx(&keys, &[0], &[5], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);

	assert_eq!(diagnosis.accounts[0].field, None);
	assert_eq!(
		diagnosis
			.instruction
			.as_ref()
			.map(|instruction| instruction.accounts_struct.clone()),
		Some(None)
	);
	assert!(diagnosis.notes[0].contains("No `#[derive(Accounts)]` struct is known for `unrouted`"));
}

#[test]
fn an_unknown_discriminator_is_confirmed_from_the_data() {
	let error = builtin("InvalidInstructionData");
	let keys = [("Payer", true, true)];
	let transaction = tx(&keys, &[0], &[42], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);
	assert_eq!(diagnosis.instruction, None);
	assert_eq!(
		diagnosis.candidates[0].reason,
		"the data starts with discriminator 42, which no instruction of `fixture` declares"
	);

	let transaction = tx(&keys, &[0], &[], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);
	assert_eq!(
		diagnosis.candidates[0].reason,
		"the instruction data holds 0 byte(s), fewer than the 1-byte discriminator"
	);

	// A program without instructions has no discriminator width to report.
	let file = syn::parse_file(r#"declare_id!("GKYaKKaAJvuzkH2GKkaEFAqESh9NEobZ3V2Ub7qbpVYn");"#)
		.unwrap_or_else(|error| panic!("parses: {error}"));
	let program = crate::parse::assemble_program_ir(&file, "empty")
		.unwrap_or_else(|error| panic!("assembles: {error}"));
	let index = source::index(Vec::new(), Path::new("/project"))
		.unwrap_or_else(|error| panic!("indexes: {error}"));
	let messages = Vec::new();
	let diagnosis = diagnose::diagnose(
		&program,
		&index,
		&Failure {
			transaction: &transaction,
			instruction: &transaction.instructions[0],
			error: &error,
			messages: &messages,
			state: CurrentState::NotRequested,
		},
	);
	assert!(diagnosis.candidates.is_empty());
}

#[test]
fn instruction_data_rules_are_possible_causes() {
	let error = builtin("InvalidInstructionData");
	let transaction = tx(&rules_keys(), &RULES_ACCOUNTS, &[0], failed(error.clone()));
	let diagnosis = diagnose(&transaction, &error, &[], CurrentState::NotRequested);

	assert_eq!(
		summary(&diagnosis),
		[entry("limit", "value <= 9", Confidence::Possible, false)]
	);
	assert_eq!(diagnosis.candidates[0].scope, CandidateScope::Argument);
	assert!(diagnosis.flow.functions.contains("validate_rules_args"));
	assert!(diagnosis.flow.functions.contains("validate_rules"));
}

#[test]
fn an_instruction_the_source_index_lacks_is_still_named() {
	let (program, _) = fixture();
	let index = source::index(Vec::new(), Path::new("/project")).expect("an empty index");
	let error = builtin("InvalidAccountData");
	let transaction = tx(&[("Payer", true, true)], &[0], &[0], failed(error.clone()));
	let messages = Vec::new();
	let diagnosis = diagnose::diagnose(
		&program,
		&index,
		&Failure {
			transaction: &transaction,
			instruction: &transaction.instructions[0],
			error: &error,
			messages: &messages,
			state: CurrentState::NotRequested,
		},
	);

	assert_eq!(
		diagnosis.instruction.map(|instruction| instruction.name),
		Some("rules".to_owned())
	);
	assert!(diagnosis.candidates.is_empty());
}

#[test]
fn renders_sites_outside_the_failing_instruction_under_their_own_heading() {
	let transaction = tx(
		&[("Payer", true, true)],
		&[0],
		&[1],
		failed(InstructionError::Custom(6000)),
	);
	let text = report_for(&transaction, None).render_text();

	assert!(text.contains("Where the program returns the error:\n  src/lib.rs:"));
	assert!(text.contains(" in unrelated\n"));
}

fn report_for(transaction: &Transaction, endpoint: Option<&RpcEndpoint>) -> ExplainReport {
	let (program, index) = fixture();

	report(&program, &index, transaction, endpoint, "file".to_owned())
}

#[test]
fn reports_success_and_transaction_level_failures() {
	let mut transaction = tx(&[("Payer", true, true)], &[0], &[1], Outcome::Succeeded);
	let report = report_for(&transaction, None);
	assert_eq!(report.status, TransactionStatus::Succeeded);
	assert_eq!(
		report.render_text(),
		"Transaction sig succeeded at slot 9 (from file); there is nothing to explain.\n"
	);

	transaction.outcome =
		Outcome::TransactionFailed(json!({ "InsufficientFundsForRent": { "account_index": 1 } }));
	transaction.logs = None;
	let report = report_for(&transaction, None);
	let failure = report.failure.as_ref().unwrap_or_else(|| panic!("failure"));
	assert_eq!(failure.error.kind, ErrorKind::Transaction);
	assert_eq!(failure.error.name, "InsufficientFundsForRent");
	assert!(
		report
			.notes
			.iter()
			.any(|note| note.contains("returned no logs"))
	);
	assert!(
		report
			.render_text()
			.contains("The transaction failed outside its instructions.")
	);

	assert_eq!(
		transaction_error(&json!("AccountNotFound")).name,
		"AccountNotFound"
	);
	assert_eq!(transaction_error(&json!(["odd"])).name, "[\"odd\"]");
}

#[test]
fn reports_a_failure_in_another_program_without_candidates() {
	let mut transaction = tx(
		&[("Payer", true, true), (SYSTEM, false, false)],
		&[0],
		&[2, 0, 0, 0],
		failed(InstructionError::Custom(1)),
	);
	transaction.instructions[0].program_id = SYSTEM.to_owned();
	transaction.logs = Some(vec![
		format!("Program {SYSTEM} invoke [1]"),
		"Transfer: insufficient lamports".to_owned(),
		format!("Program {SYSTEM} failed: custom program error: 0x1"),
		"Log truncated".to_owned(),
	]);
	let report = report_for(&transaction, None);
	let failure = report.failure.as_ref().unwrap_or_else(|| panic!("failure"));

	assert!(!failure.targets_program);
	assert_eq!(failure.error.kind, ErrorKind::Unknown);
	assert!(
		failure
			.error
			.description
			.as_deref()
			.is_some_and(|text| text.contains(SYSTEM))
	);
	assert!(report.candidates.is_empty());
	assert_eq!(report.accounts[0].field, None);
	assert!(report.notes.iter().any(|note| note.contains("truncated")));
	assert!(report.render_text().contains(&format!(
		"Instruction #0 failed in {SYSTEM}, not in fixture"
	)));
}

#[test]
fn reports_a_cpi_failure_as_the_callee_error() {
	let mut transaction = tx(
		&[("Payer", true, true)],
		&[0, 1],
		&[1],
		failed(InstructionError::Custom(0)),
	);
	transaction.logs = Some(vec![
		format!("Program {PROGRAM_ID} invoke [1]"),
		format!("Program {SYSTEM} invoke [2]"),
		format!("Program {SYSTEM} failed: custom program error: 0x0"),
		format!("Program {PROGRAM_ID} failed: custom program error: 0x0"),
	]);
	let endpoint = RpcEndpoint::network(Network::Localnet);
	let report = report_for(&transaction, Some(&endpoint));
	let failure = report.failure.as_ref().unwrap_or_else(|| panic!("failure"));

	assert_eq!(
		failure.cpi.as_ref().map(|cpi| cpi.program_id.as_str()),
		Some(SYSTEM)
	);
	assert!(
		failure
			.error
			.description
			.as_deref()
			.is_some_and(|text| text.contains("invoked"))
	);
	assert!(report.candidates.is_empty());
	assert!(report.error_sites.is_empty());
	assert!(
		report
			.notes
			.iter()
			.any(|note| note.contains("inside a CPI"))
	);
	let text = report.render_text();
	assert!(text.contains(&format!("Failed inside a CPI to {SYSTEM} (depth 2)")));
}

#[test]
fn reports_construction_sites_of_the_failing_instruction_first() {
	let error = InstructionError::Custom(6000);
	let transaction = tx(&rules_keys(), &RULES_ACCOUNTS, &[0], failed(error));
	let report = report_for(&transaction, None);
	let sites: Vec<_> = report
		.error_sites
		.iter()
		.map(|site| (site.context.as_str(), site.in_failing_instruction))
		.collect();

	assert_eq!(
		sites,
		[
			("RulesInstruction", true),
			("RulesAccounts", true),
			("validate_rules", true),
			("unrelated", false),
		]
	);
	let text = report.render_text();
	assert!(text.contains("Where this instruction returns the error:"));
	assert!(text.contains(
		"Error: FixtureError::Denied (custom error 6000)\n  The caller may not do this."
	));
	assert!(!text.contains("unrelated"));

	let pina = transaction_with_error(InstructionError::Custom(0xFFFF_FFFA));
	let report = report_for(&pina, None);
	assert_eq!(report.error_sites.len(), 2);
	assert!(report.render_text().contains("(custom error 0xFFFFFFFA)"));

	let builtin_error = transaction_with_error(builtin("InvalidArgument"));
	let report = report_for(&builtin_error, None);
	let contexts: Vec<_> = report
		.error_sites
		.iter()
		.map(|site| site.context.as_str())
		.collect();
	assert_eq!(contexts, ["RulesAccounts", "RulesAccounts::process"]);

	let unknown = transaction_with_error(builtin("ArithmeticOverflow"));
	let report = report_for(&unknown, None);
	assert!(report.error_sites.is_empty());
	assert!(
		report
			.notes
			.iter()
			.any(|note| note.contains("No declared Pina check"))
	);
}

fn transaction_with_error(error: InstructionError) -> Transaction {
	tx(&rules_keys(), &RULES_ACCOUNTS, &[0], failed(error))
}

#[test]
fn renders_candidates_accounts_logs_and_escapes_control_characters() {
	let mut keys = rules_keys();
	keys[1] = ("NotSystem", false, false);
	let mut transaction = tx(
		&keys,
		&RULES_ACCOUNTS,
		&[0],
		failed(builtin("InvalidAccountData")),
	);
	transaction.logs = Some(vec![
		format!("Program {PROGRAM_ID} invoke [1]"),
		"Program log: account address is invalid\u{1b}[2J".to_owned(),
		format!("Program {PROGRAM_ID} failed: invalid account data for instruction"),
	]);
	let report = report_for(&transaction, None);
	let text = report.render_text();

	assert!(text.contains(
		"Most likely cause:\n  fixed: address = system::ID [confirmed] at src/lib.rs:64"
	));
	assert!(
		text.contains("Other candidates:\n  authority: executable [possible] at src/lib.rs:62")
	);
	assert!(text.contains("Program log: account address is invalid\\u{1b}[2J"));
	assert!(
		text.contains("  6   system_program  no      no        NotSystem"),
		"{text}"
	);
	assert!(!text.contains('\u{1b}'));

	let mut keys = rules_keys();
	keys[0] = ("Authority", true, false);
	let optional = tx(
		&[("Payer", true, true)],
		&[0, 1],
		&[1],
		failed(builtin("InvalidAccountData")),
	);
	let report = report_for(&optional, None);
	let text = report.render_text();
	assert!(text.contains("witness (absent)"));
	let _ = keys;

	let possible = tx(
		&rules_keys(),
		&RULES_ACCOUNTS,
		&[0],
		failed(builtin("InvalidInstructionData")),
	);
	let text = report_for(&possible, None).render_text();
	assert!(text.contains(
		"Possible causes (they need runtime values to check):\n  limit: value <= 9 [possible]"
	));

	let mut logged = tx(
		&rules_keys(),
		&RULES_ACCOUNTS,
		&[0],
		failed(builtin("InvalidAccountData")),
	);
	logged.logs = Some(vec![
		format!("Program {PROGRAM_ID} invoke [1]"),
		"Program log: account is not executable".to_owned(),
	]);
	let text = report_for(&logged, None).render_text();
	assert!(text.contains("authority: executable [possible, matches the program log]"));
}

#[test]
fn state_lookup_runs_only_for_the_program_s_own_checks() {
	let mut keys = rules_keys();
	keys[0] = ("Authority", true, false);
	let transaction = tx(
		&keys,
		&RULES_ACCOUNTS,
		&[0],
		failed(builtin("InvalidAccountOwner")),
	);
	let states = json!({
		"context": { "slot": 1 },
		"value": [
			{ "owner": SYSTEM, "executable": false, "space": 0, "lamports": 1, "data": ["", "base64"] },
			null,
			{ "owner": SYSTEM, "executable": true, "space": 0, "lamports": 1, "data": ["", "base64"] },
			{ "owner": SYSTEM, "executable": false, "space": 0, "lamports": 1, "data": ["", "base64"] },
			{ "owner": SYSTEM, "executable": false, "space": 0, "lamports": 1, "data": ["", "base64"] },
			{ "owner": SYSTEM, "executable": false, "space": 0, "lamports": 1, "data": ["", "base64"] },
			{ "owner": SYSTEM, "executable": false, "space": 0, "lamports": 1, "data": ["", "base64"] },
			{ "owner": SYSTEM, "executable": false, "space": 0, "lamports": 1, "data": ["", "base64"] },
			{ "owner": SYSVAR_OWNER, "executable": false, "space": 40, "lamports": 1, "data": ["", "base64"] }
		]
	});
	let (url, server) = serve(vec![(
		200,
		json!({ "jsonrpc": "2.0", "id": 1, "result": states }).to_string(),
	)]);
	let endpoint = RpcEndpoint::custom(&url).unwrap_or_else(|error| panic!("endpoint: {error}"));
	let report = report_for(&transaction, Some(&endpoint));
	let requests = server.join().unwrap_or_else(|_| panic!("server thread"));

	let body = request_body(&requests[0]);
	assert_eq!(body["method"], "getMultipleAccounts");
	assert_eq!(
		body["params"][1]["dataSlice"],
		json!({ "offset": 0, "length": 0 })
	);
	assert_eq!(body["params"][0].as_array().map(Vec::len), Some(9));
	assert_eq!(
		report.candidates[0].confidence,
		Confidence::CheckedAgainstCurrentState
	);
	assert!(
		report
			.notes
			.iter()
			.any(|note| note.contains("Account state was read from the custom RPC endpoint"))
	);
	assert!(
		report
			.render_text()
			.contains("[checked against current state]")
	);

	let (url, server) = serve(vec![(500, "boom".to_owned())]);
	let endpoint = RpcEndpoint::custom(&url).unwrap_or_else(|error| panic!("endpoint: {error}"));
	let report = report_for(&transaction, Some(&endpoint));
	let _ = server.join();

	assert!(
		report
			.notes
			.iter()
			.any(|note| note.contains("Current account state is unavailable"))
	);
	assert!(
		report
			.candidates
			.iter()
			.all(|candidate| candidate.confidence == Confidence::Possible)
	);
}

/// Serve one canned HTTP response per connection, in order, and return each
/// request.
fn serve(responses: Vec<(u16, String)>) -> (String, std::thread::JoinHandle<Vec<String>>) {
	let listener =
		std::net::TcpListener::bind("127.0.0.1:0").unwrap_or_else(|error| panic!("bind: {error}"));
	let port = listener
		.local_addr()
		.unwrap_or_else(|error| panic!("addr: {error}"))
		.port();
	let server = std::thread::spawn(move || {
		let mut requests = Vec::new();
		for (status, body) in responses {
			let (mut stream, _) = listener
				.accept()
				.unwrap_or_else(|error| panic!("accept: {error}"));
			requests.push(read_request(&mut stream));
			let reason = match status {
				200 => "OK",
				302 => "Found",
				_ => "Error",
			};
			let location = if status == 302 {
				"location: http://example.com/\r\n"
			} else {
				""
			};
			// Status 0 sends `body` as the complete raw response.
			let response = if status == 0 {
				body
			} else {
				format!(
					"HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\n{location}content-length: {}\r\nconnection: close\r\n\r\n{body}",
					body.len()
				)
			};
			stream
				.write_all(response.as_bytes())
				.unwrap_or_else(|error| panic!("write: {error}"));
			let _ = stream.shutdown(std::net::Shutdown::Write);
			let mut sink = [0_u8; 512];
			while matches!(stream.read(&mut sink), Ok(read) if read > 0) {}
		}
		requests
	});

	(format!("http://127.0.0.1:{port}"), server)
}

/// The JSON body of a recorded HTTP request.
fn request_body(request: &str) -> Value {
	let (_, body) = request
		.split_once("\r\n\r\n")
		.unwrap_or_else(|| panic!("request has a body: {request}"));

	serde_json::from_str(body).unwrap_or_else(|error| panic!("request body is JSON: {error}"))
}

fn read_request(stream: &mut std::net::TcpStream) -> String {
	let mut request = Vec::new();
	let mut buffer = [0_u8; 1024];

	loop {
		let read = match stream.read(&mut buffer) {
			Ok(0) | Err(_) => break,
			Ok(read) => read,
		};
		request.extend_from_slice(&buffer[..read]);
		let Some(end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
			continue;
		};
		let headers = String::from_utf8_lossy(&request[..end]).to_lowercase();
		if headers.contains("transfer-encoding: chunked") {
			if request.ends_with(b"0\r\n\r\n") {
				break;
			}
			continue;
		}
		let length = headers
			.lines()
			.find_map(|line| line.strip_prefix("content-length:"))
			.and_then(|value| value.trim().parse::<usize>().ok())
			.unwrap_or(0);
		if request.len() >= end + 4 + length {
			break;
		}
	}

	String::from_utf8_lossy(&request).into_owned()
}

const SIGNATURE: &str =
	"63T6XGyihfHKThEZFe36L5LkmAqJ8uVaqzQ6Po4ovSZPzCCDKk5h8EJGHdyQFP5sbbDwbW2csgfXWbWZWrXDjD7f";

fn endpoint(url: &str) -> RpcEndpoint {
	RpcEndpoint::custom(url).unwrap_or_else(|error| panic!("endpoint: {error}"))
}

#[test]
fn fetches_transactions_and_reports_rpc_failures() {
	let result = json!({ "slot": 3 });
	let (url, server) = serve(vec![(
		200,
		json!({ "jsonrpc": "2.0", "id": 1, "result": result }).to_string(),
	)]);
	let value = rpc::get_transaction(&endpoint(&url), SIGNATURE)
		.unwrap_or_else(|error| panic!("fetch: {error}"));
	let requests = server.join().unwrap_or_else(|_| panic!("server"));
	assert_eq!(value, result);
	let body = request_body(&requests[0]);
	assert_eq!(body["method"], "getTransaction");
	assert_eq!(
		body["params"],
		json!([SIGNATURE, { "encoding": "json", "maxSupportedTransactionVersion": 0, "commitment": "confirmed" }])
	);

	let cases = [
		(200, json!({ "jsonrpc": "2.0", "id": 1, "result": null }).to_string(), "was not found on the custom RPC endpoint"),
		(
			200,
			json!({ "jsonrpc": "2.0", "id": 1, "error": { "code": -32602, "message": "Invalid param" } }).to_string(),
			"getTransaction on the custom RPC endpoint failed: Invalid param",
		),
		(200, json!({ "jsonrpc": "2.0", "id": 1, "error": { "code": 7 } }).to_string(), "{\"code\":7}"),
		(200, json!({ "jsonrpc": "2.0", "id": 1 }).to_string(), "neither `result` nor `error`"),
		(200, "not json".to_owned(), "invalid JSON"),
		(500, "{}".to_owned(), "500"),
		(302, String::new(), "redirects are not followed"),
		(
			0,
			"HTTP/1.1 200 OK\r\ncontent-length: 100\r\nconnection: close\r\n\r\n{}".to_owned(),
			"getTransaction on the custom RPC endpoint failed",
		),
	];

	for (status, body, message) in cases {
		let (url, server) = serve(vec![(status, body)]);
		let error = rpc::get_transaction(&endpoint(&url), SIGNATURE).expect_err("RPC failure");
		let _ = server.join();
		assert!(error.to_string().contains(message), "{error}");
		assert!(!error.to_string().contains("127.0.0.1"), "{error}");
	}

	let closed =
		std::net::TcpListener::bind("127.0.0.1:0").unwrap_or_else(|error| panic!("bind: {error}"));
	let url = format!(
		"http://127.0.0.1:{}",
		closed
			.local_addr()
			.unwrap_or_else(|error| panic!("addr: {error}"))
			.port()
	);
	drop(closed);
	let error = rpc::get_transaction(&endpoint(&url), SIGNATURE).expect_err("connection refused");
	assert!(
		error
			.to_string()
			.contains("getTransaction on the custom RPC endpoint failed")
	);
}

#[test]
fn reads_account_states_and_rejects_short_responses() {
	let addresses = vec!["A".to_owned(), "B".to_owned(), "C".to_owned()];
	let body = json!({
		"jsonrpc": "2.0",
		"id": 1,
		"result": {
			"context": { "slot": 1 },
			"value": [
				{ "owner": PROGRAM_ID, "executable": false, "space": 19 },
				null,
				{ "owner": SYSTEM, "executable": true }
			]
		}
	});
	let (url, server) = serve(vec![(200, body.to_string())]);
	let states = rpc::get_account_states(&endpoint(&url), &addresses)
		.unwrap_or_else(|error| panic!("states: {error}"));
	let _ = server.join();

	assert_eq!(states["A"], state(PROGRAM_ID, false, Some(19)));
	assert_eq!(states["B"], state(SYSTEM, false, Some(0)));
	assert_eq!(states["C"], state(SYSTEM, true, None));

	let body = json!({ "jsonrpc": "2.0", "id": 1, "result": { "value": [null] } });
	let (url, server) = serve(vec![(200, body.to_string())]);
	let error = rpc::get_account_states(&endpoint(&url), &addresses).expect_err("short response");
	let _ = server.join();
	assert!(
		error
			.to_string()
			.contains("one account per requested address")
	);
}

#[test]
fn validates_custom_rpc_urls() {
	for (url, reason) in [
		("http://127.0.0.1:8899\n", "control characters"),
		("not a url", "not a valid URL"),
		("file:///tmp/socket", "an endpoint host is required"),
		("ftp://rpc.example.com", "only http and https"),
		(
			"https://user:secret@rpc.example.com",
			"embedded credentials",
		),
		(
			"https://rpc.example.com/?api-key=secret",
			"query parameters and fragments",
		),
		(
			"https://rpc.example.com/#token",
			"query parameters and fragments",
		),
		("http://rpc.example.com", "plaintext http"),
	] {
		let error = RpcEndpoint::custom(url).expect_err("unsafe URL");
		assert!(error.to_string().contains(reason), "{url}: {error}");
		assert!(!error.to_string().contains("secret"), "{error}");
	}

	for url in [
		"http://localhost:8899",
		"http://127.0.0.1:8899",
		"http://[::1]:8899",
		"https://rpc.example.com/token-path",
	] {
		let endpoint = RpcEndpoint::custom(url).unwrap_or_else(|error| panic!("{url}: {error}"));
		assert_eq!(endpoint.label(), "custom RPC endpoint");
	}

	for (network, label) in [
		(Network::Localnet, "localnet"),
		(Network::Devnet, "devnet"),
		(Network::Testnet, "testnet"),
		(Network::Mainnet, "mainnet"),
	] {
		assert_eq!(RpcEndpoint::network(network).label(), label);
	}
}

fn validation_program() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/validation_program")
}

fn fixture_file(name: &str) -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/explain/{name}.json"))
}

#[test]
fn explains_signatures_through_the_endpoint() {
	let result: Value = serde_json::from_str(
		&std::fs::read_to_string(fixture_file("missing_signer"))
			.unwrap_or_else(|error| panic!("fixture: {error}")),
	)
	.unwrap_or_else(|error| panic!("fixture JSON: {error}"));
	let states = json!({ "value": [null, null, null, null] });
	let (url, server) = serve(vec![
		(
			200,
			json!({ "jsonrpc": "2.0", "id": 1, "result": result }).to_string(),
		),
		(
			200,
			json!({ "jsonrpc": "2.0", "id": 1, "result": states }).to_string(),
		),
	]);
	let report = explain(&ExplainOptions {
		project: validation_program(),
		input: TransactionInput::Signature {
			signature: SIGNATURE.to_owned(),
			endpoint: endpoint(&url),
		},
	})
	.unwrap_or_else(|error| panic!("explain: {error}"));
	let requests = server.join().unwrap_or_else(|_| panic!("server"));

	assert_eq!(requests.len(), 2);
	assert_eq!(report.source, "custom RPC endpoint");
	assert_eq!(report.candidates[0].rule, "signer");
}

#[test]
fn rejects_invalid_inputs_before_fetching() {
	let options = |input| {
		ExplainOptions {
			project: validation_program(),
			input,
		}
	};
	let localnet = RpcEndpoint::network(Network::Localnet);

	let error = explain(&options(TransactionInput::Signature {
		signature: "not-base58!".to_owned(),
		endpoint: localnet.clone(),
	}))
	.expect_err("invalid signature");
	assert!(
		error
			.to_string()
			.contains("expected 64 base58-encoded bytes")
	);

	let error = explain(&options(TransactionInput::File {
		path: PathBuf::from("/definitely/missing.json"),
		endpoint: None,
	}))
	.expect_err("missing file");
	assert!(
		error
			.to_string()
			.contains("could not read /definitely/missing.json")
	);

	let directory = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
	let write = |name: &str, contents: &str| {
		let path = directory.path().join(name);
		std::fs::write(&path, contents).unwrap_or_else(|error| panic!("write: {error}"));
		path
	};

	for (name, contents, message) in [
		("broken.json", "{", "invalid transaction JSON"),
		(
			"envelope-error.json",
			r#"{"jsonrpc":"2.0","error":{"message":"gone"}}"#,
			"not a getTransaction result: gone",
		),
		(
			"envelope-null.json",
			r#"{"jsonrpc":"2.0","result":null}"#,
			"holds a null result",
		),
		(
			"shape.json",
			r#"{"jsonrpc":"2.0","result":{"slot":1}}"#,
			"not a JSON-encoded getTransaction result",
		),
	] {
		let error = explain(&options(TransactionInput::File {
			path: write(name, contents),
			endpoint: None,
		}))
		.expect_err(name);
		assert!(error.to_string().contains(message), "{name}: {error}");
	}

	let error = explain(&ExplainOptions {
		project: directory.path().to_path_buf(),
		input: TransactionInput::File {
			path: fixture_file("missing_signer"),
			endpoint: None,
		},
	})
	.expect_err("no project");
	assert!(!error.to_string().is_empty());
}

#[test]
fn reports_source_problems_as_errors() {
	let directory = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
	std::fs::create_dir_all(directory.path().join("src"))
		.unwrap_or_else(|error| panic!("mkdir: {error}"));
	std::fs::write(
		directory.path().join("pina.toml"),
		"[project]\nprogram = \".\"\n",
	)
	.unwrap_or_else(|error| panic!("write: {error}"));
	std::fs::write(
		directory.path().join("Cargo.toml"),
		"[package]\nname = \"broken\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[lib]\ncrate-type = [\"cdylib\"]\n",
	)
	.unwrap_or_else(|error| panic!("write: {error}"));
	std::fs::write(directory.path().join("src/lib.rs"), "pub fn broken(")
		.unwrap_or_else(|error| panic!("write: {error}"));

	let error = explain(&ExplainOptions {
		project: directory.path().to_path_buf(),
		input: TransactionInput::File {
			path: fixture_file("missing_signer"),
			endpoint: None,
		},
	})
	.expect_err("unparsable source");
	assert!(
		error
			.to_string()
			.contains("could not read the program source")
	);

	let error = source::index(
		vec![ResolvedFile {
			path: PathBuf::from("/project/src/lib.rs"),
			file: syn::parse_quote! {
				#[derive(Accounts)]
				pub struct Bad<'a> {
					#[pina(validate(singner))]
					pub authority: &'a AccountView,
				}
			},
		}],
		Path::new("/project"),
	)
	.err()
	.unwrap_or_else(|| panic!("an unknown rule fails indexing"));
	assert!(
		error
			.to_string()
			.contains("unknown account validation rule `singner`")
	);
	assert!(
		ExplainError::SourceIndex(error)
			.to_string()
			.contains("could not index the program source")
	);
}
