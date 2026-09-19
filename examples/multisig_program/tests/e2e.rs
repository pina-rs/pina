//! SBF end-to-end tests for the multisig program.
//!
//! Build the program before running these ignored tests:
//!
//! ```sh
//! cargo build-multisig-program
//! ```
//!
//! ```sh
//! SBF_OUT_DIR=target/deploy \
//!     cargo test -p multisig_program --test e2e -- --include-ignored
//! ```

use mollusk_svm::Mollusk;
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::result::Check;
use mollusk_svm::result::InstructionResult;
use multisig_program::ConfigInitializeIx;
use multisig_program::KIND_CONFIG;
use multisig_program::KIND_VAULT;
use multisig_program::MAX_MESSAGE_BYTES;
use multisig_program::Multisig;
use multisig_program::MultisigAccountType;
use multisig_program::MultisigError;
use multisig_program::MultisigInstruction;
use multisig_program::MultisigPatch;
use multisig_program::PERIOD_DAY;
use multisig_program::PERMISSIONS_ALL;
use multisig_program::ProgramConfig;
use multisig_program::Proposal;
use multisig_program::ProposalPatch;
use multisig_program::STATUS_ACTIVE;
use multisig_program::STATUS_APPROVED;
use multisig_program::STATUS_DRAFT;
use multisig_program::STATUS_EXECUTED;
use multisig_program::SpendingLimit;
use multisig_program::SpendingLimitPatch;
use multisig_program::SpendingLimitUseIx;
use pina::Address;
use solana_account::Account;
use solana_instruction::AccountMeta;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

const RENT_LAMPORTS: u64 = 100_000_000;
const VAULT_LAMPORTS: u64 = 500_000_000;

fn program_id() -> Pubkey {
	let bytes: &[u8] = multisig_program::ID.as_ref();
	let array: [u8; 32] = bytes
		.try_into()
		.unwrap_or_else(|_| panic!("program address must be 32 bytes"));
	Pubkey::new_from_array(array)
}

/// The program that owns the legacy account in the import test.
fn legacy_program_id() -> Pubkey {
	key(99)
}

/// An arbitrary Anchor-style account discriminator for the legacy fixture.
fn legacy_discriminator() -> [u8; 8] {
	[0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]
}

fn key(seed: u8) -> Pubkey {
	let mut bytes = [0_u8; 32];
	bytes[0] = seed;
	Pubkey::new_from_array(bytes)
}

fn create_mollusk() -> Mollusk {
	let so_name = "multisig_program.so";
	let search_dirs: Vec<std::path::PathBuf> = [
		std::env::var("SBF_OUT_DIR").ok(),
		std::env::var("BPF_OUT_DIR").ok(),
		Some("tests/fixtures".to_owned()),
	]
	.into_iter()
	.flatten()
	.map(std::path::PathBuf::from)
	.collect();

	assert!(
		search_dirs.iter().any(|dir| dir.join(so_name).is_file()),
		"multisig_program SBF binary not found; build it with `cargo build-multisig-program`"
	);

	Mollusk::new(&program_id(), "multisig_program")
}

fn pina_address(pubkey: &Pubkey) -> Address {
	Address::new_from_array(pubkey.to_bytes())
}

fn system_account(lamports: u64) -> Account {
	Account::new(lamports, 0, &solana_sdk_ids::system_program::id())
}

fn stored_account(data: Vec<u8>, lamports: u64) -> Account {
	Account {
		lamports,
		data,
		owner: program_id(),
		executable: false,
		rent_epoch: 0,
	}
}

/// Per-test world: the account list handed to each instruction, updated from
/// every instruction result so state carries across a lifecycle.
struct World {
	accounts: Vec<(Pubkey, Account)>,
}

impl World {
	fn new() -> Self {
		World {
			accounts: Vec::new(),
		}
	}

	fn add(&mut self, pubkey: Pubkey, account: Account) {
		self.accounts.retain(|(candidate, _)| *candidate != pubkey);
		self.accounts.push((pubkey, account));
	}

	fn apply(&mut self, result: &InstructionResult) {
		for (pubkey, account) in &result.resulting_accounts {
			self.add(*pubkey, account.clone());
		}
	}

	fn get(&self, pubkey: &Pubkey) -> &Account {
		&self
			.accounts
			.iter()
			.find(|(candidate, _)| candidate == pubkey)
			.unwrap_or_else(|| panic!("account {pubkey} missing from world"))
			.1
	}

	fn run(
		&mut self,
		mollusk: &Mollusk,
		instruction: &Instruction,
		checks: &[Check],
	) -> InstructionResult {
		let result =
			mollusk.process_and_validate_instruction(instruction, &self.accounts.clone(), checks);
		self.apply(&result);
		result
	}
}

fn account<'a>(result: &'a InstructionResult, address: &Pubkey) -> &'a Account {
	&result
		.resulting_accounts
		.iter()
		.find(|(candidate, _)| candidate == address)
		.unwrap_or_else(|| panic!("account {address} missing from result"))
		.1
}

// ---------------------------------------------------------------------------
// PDA derivation, mirroring the on-chain seeds
// ---------------------------------------------------------------------------

fn multisig_pda(create_key: &Pubkey) -> (Pubkey, u8) {
	Pubkey::find_program_address(&[b"multisig", create_key.as_ref()], &program_id())
}

fn proposal_pda(multisig: &Pubkey, index: u64) -> (Pubkey, u8) {
	Pubkey::find_program_address(
		&[b"proposal", multisig.as_ref(), &index.to_le_bytes()],
		&program_id(),
	)
}

fn vault_pda(multisig: &Pubkey, vault_index: u8) -> (Pubkey, u8) {
	Pubkey::find_program_address(
		&[b"vault", multisig.as_ref(), &[vault_index]],
		&program_id(),
	)
}

fn program_config_pda() -> (Pubkey, u8) {
	Pubkey::find_program_address(&[b"multisig-program-config"], &program_id())
}

fn spending_limit_pda(multisig: &Pubkey, create_key: &Pubkey) -> (Pubkey, u8) {
	Pubkey::find_program_address(
		&[b"spending-limit", multisig.as_ref(), create_key.as_ref()],
		&program_id(),
	)
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Three members with every permission, sorted ascending so the on-chain
/// binary-search lookups behave.
fn sorted_member_keys() -> [Pubkey; 3] {
	let mut keys = [key(3), key(1), key(2)];
	keys.sort_unstable();
	keys
}

#[expect(
	clippy::too_many_arguments,
	reason = "fixtures mirror the on-chain fields one to one"
)]
fn multisig_data(
	create_key: &Pubkey,
	threshold: u16,
	timelock: u32,
	ttl: u32,
	member_keys: &[Pubkey],
	bump: u8,
	transaction_index: u64,
) -> Vec<u8> {
	let mut keys = [Address::default(); 16];
	let mut permissions = [0_u8; 16];
	for (position, key) in member_keys.iter().enumerate() {
		keys[position] = pina_address(key);
		permissions[position] = PERMISSIONS_ALL;
	}
	let space =
		Multisig::projected_bytes(member_keys.len() * 32, member_keys.len()).expect("member space");
	let mut data = vec![0_u8; space];
	Multisig::initialize(
		&mut data,
		&MultisigPatch::new()
			.bump(bump)
			.create_key(pina_address(create_key))
			.config_authority(Address::default())
			.rent_collector(Address::default())
			.threshold(threshold)
			.timelock(timelock)
			.ttl(ttl)
			.transaction_index(transaction_index)
			.stale_transaction_index(0)
			.replace_member_roster(
				&flatten_fixture_roster(&keys[..member_keys.len()])[..member_keys.len() * 32],
			)
			.replace_member_permissions(&permissions[..member_keys.len()]),
	)
	.unwrap_or_else(|error| panic!("encode multisig fixture: {error:?}"));
	data
}

fn proposal_data(
	multisig: &Pubkey,
	creator: &Pubkey,
	index: u64,
	kind: u8,
	status: u8,
	status_at: i64,
	message: Option<&[u8]>,
	actions: Option<&[u8]>,
	bump: u8,
) -> Vec<u8> {
	let message = message.unwrap_or(&[]);
	let actions = actions.unwrap_or(&[]);
	let vault_bump = vault_pda(multisig, 0).1;
	let space = Proposal::projected_bytes(0, message.len(), actions.len())
		.unwrap_or_else(|error| panic!("proposal space: {error:?}"));
	let mut data = vec![0_u8; space];
	Proposal::initialize(
		&mut data,
		&ProposalPatch::new()
			.bump(bump)
			.multisig(pina_address(multisig))
			.creator(pina_address(creator))
			.index(index)
			.kind(kind)
			.vault_index(0)
			.vault_bump(vault_bump)
			.status(status)
			.status_at(status_at)
			.expires_at(0)
			.approved_mask(0)
			.rejected_mask(0)
			.replace_message(message)
			.replace_actions(actions),
	)
	.unwrap_or_else(|error| panic!("encode proposal fixture: {error:?}"));
	data
}

/// Flatten addresses into the roster wire form.
fn flatten_fixture_roster(keys: &[Address]) -> [u8; 512] {
	let mut bytes = [0_u8; 512];
	for (position, key) in keys.iter().enumerate() {
		bytes[position * 32..position * 32 + 32].copy_from_slice(key.as_ref());
	}
	bytes
}

/// Decode a roster into owned addresses plus its length.
fn decode_fixture_roster(bytes: &[u8]) -> ([Address; 16], usize) {
	let count = bytes.len() / 32;
	let mut keys = [Address::default(); 16];
	for (position, slot) in keys.iter_mut().take(count).enumerate() {
		*slot = Address::try_from(&bytes[position * 32..position * 32 + 32]).unwrap();
	}
	(keys, count)
}

fn program_config_data(authority: &Pubkey, treasury: &Pubkey, fee: u64, bump: u8) -> Vec<u8> {
	let mut data = vec![0_u8; ProgramConfig::SIZE];
	ProgramConfig::initialize(&mut data, |config| {
		config.bump = bump;
		config.authority = pina_address(authority);
		config.treasury = pina_address(treasury);
		config.creation_fee.set(fee);
		Ok(())
	})
	.expect("encode program config fixture");
	data
}

/// A system-program `Transfer` (discriminator 2) instruction payload.
fn system_transfer_data(lamports: u64) -> Vec<u8> {
	let mut data = Vec::new();
	data.extend_from_slice(&2_u32.to_le_bytes());
	data.extend_from_slice(&lamports.to_le_bytes());
	data
}

fn encode_message_fixture(keys: &[Pubkey], instructions: &[(usize, &[u8], &[u8])]) -> Vec<u8> {
	let mut buffer = [0_u8; MAX_MESSAGE_BYTES];
	let addresses: Vec<Address> = keys.iter().map(pina_address).collect();
	let length = multisig_program::encode_message(
		1,
		1,
		keys.len().saturating_sub(2),
		&addresses,
		instructions,
		&mut buffer,
	)
	.unwrap_or_else(|error| panic!("encode message fixture: {error:?}"));
	buffer[..length].to_vec()
}

fn empty_ix_data(discriminant: u8) -> Vec<u8> {
	// The migrations envelope gives every instruction a version byte.
	vec![discriminant, 0]
}

fn clock_meta() -> AccountMeta {
	AccountMeta::new_readonly(
		solana_pubkey::pubkey!("SysvarC1ock11111111111111111111111111111111"),
		false,
	)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
#[ignore = "build the SBF artifact first"]
fn config_initialize_writes_the_global_pda() {
	let mollusk = create_mollusk();
	let (config_key, bump) = program_config_pda();
	let authority = key(9);
	let treasury = key(10);

	let mut data = vec![0_u8; ConfigInitializeIx::SIZE];
	ConfigInitializeIx::initialize(&mut data, |ix| {
		ix.bump = bump;
		ix.treasury = pina_address(&treasury);
		ix.creation_fee.set(0);
		Ok(())
	})
	.unwrap();

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&data,
		vec![
			AccountMeta::new(authority, true),
			AccountMeta::new(config_key, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);
	let mut world = World::new();
	world.add(authority, system_account(RENT_LAMPORTS));
	world.add(config_key, Account::default());
	world.add(
		solana_sdk_ids::system_program::id(),
		keyed_account_for_system_program().1,
	);

	let result = world.run(&mollusk, &instruction, &[Check::success()]);
	assert_eq!(
		account(&result, &config_key).data[0],
		MultisigAccountType::ProgramConfig as u8
	);
}

#[test]
#[ignore = "build the SBF artifact first"]
fn multisig_create_initializes_the_compact_roster() {
	let mollusk = create_mollusk();
	let create_key = key(7);
	let rent_payer = key(8);
	let (multisig_key, bump) = multisig_pda(&create_key);
	let (config_key, config_bump) = program_config_pda();
	let members = sorted_member_keys();

	let mut data = vec![0_u8; multisig_program::MultisigCreateIx::SIZE];
	multisig_program::MultisigCreateIx::initialize(&mut data, |ix| {
		ix.bump = bump;
		ix.threshold.set(2);
		ix.timelock.set(0);
		ix.ttl.set(0);
		for (position, _member) in members.iter().enumerate() {
			ix.member_permissions[position] = PERMISSIONS_ALL;
		}
		ix.config_authority = Address::default();
		ix.rent_collector = Address::default();
		Ok(())
	})
	.unwrap();

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&data,
		vec![
			AccountMeta::new_readonly(config_key, false),
			AccountMeta::new_readonly(create_key, true),
			AccountMeta::new(multisig_key, false),
			AccountMeta::new(rent_payer, true),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
			AccountMeta::new_readonly(program_id(), false), // no treasury
			AccountMeta::new_readonly(members[0], false),
			AccountMeta::new_readonly(members[1], false),
			AccountMeta::new_readonly(members[2], false),
		],
	);
	let mut world = World::new();
	world.add(
		config_key,
		stored_account(program_config_data(&key(1), &key(2), 0, config_bump), 1),
	);
	world.add(multisig_key, Account::default());
	world.add(create_key, system_account(RENT_LAMPORTS));
	world.add(rent_payer, system_account(RENT_LAMPORTS));
	for member in &members {
		world.add(*member, system_account(RENT_LAMPORTS));
	}
	world.add(
		solana_sdk_ids::system_program::id(),
		keyed_account_for_system_program().1,
	);

	let result = world.run(&mollusk, &instruction, &[Check::success()]);

	let stored = account(&result, &multisig_key);
	let state = Multisig::try_from_bytes(stored.data.as_slice())
		.unwrap_or_else(|error| panic!("decode created multisig: {error:?}"));
	assert_eq!(state.threshold.get(), 2);
	let roster = decode_fixture_roster(state.member_roster());
	assert_eq!(roster.1, 3);
	assert_eq!(roster.0[0], pina_address(&members[0]));
	assert_eq!(state.transaction_index.get(), 0);
}

#[test]
#[ignore = "build the SBF artifact first"]
fn multisig_create_rejects_duplicate_members() {
	let mollusk = create_mollusk();
	let create_key = key(7);
	let rent_payer = key(8);
	let (multisig_key, bump) = multisig_pda(&create_key);
	let (config_key, config_bump) = program_config_pda();

	let duplicate = key(1);
	let mut data = vec![0_u8; multisig_program::MultisigCreateIx::SIZE];
	multisig_program::MultisigCreateIx::initialize(&mut data, |ix| {
		ix.bump = bump;
		ix.threshold.set(1);
		ix.timelock.set(0);
		ix.ttl.set(0);
		ix.member_permissions[0] = PERMISSIONS_ALL;
		ix.member_permissions[1] = PERMISSIONS_ALL;
		ix.config_authority = Address::default();
		ix.rent_collector = Address::default();
		Ok(())
	})
	.unwrap();

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&data,
		vec![
			AccountMeta::new_readonly(config_key, false),
			AccountMeta::new_readonly(create_key, true),
			AccountMeta::new(multisig_key, false),
			AccountMeta::new(rent_payer, true),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
			AccountMeta::new_readonly(program_id(), false), // no treasury
			AccountMeta::new_readonly(duplicate, false),
			AccountMeta::new_readonly(duplicate, false),
		],
	);
	let mut world = World::new();
	world.add(
		config_key,
		stored_account(program_config_data(&key(1), &key(2), 0, config_bump), 1),
	);
	world.add(multisig_key, Account::default());
	world.add(create_key, system_account(RENT_LAMPORTS));
	world.add(rent_payer, system_account(RENT_LAMPORTS));
	world.add(duplicate, system_account(RENT_LAMPORTS));
	world.add(
		solana_sdk_ids::system_program::id(),
		keyed_account_for_system_program().1,
	);

	world.run(
		&mollusk,
		&instruction,
		&[Check::err(MultisigError::DuplicateMember.into())],
	);
}

#[test]
#[ignore = "build the SBF artifact first"]
fn multisig_import_reads_a_legacy_anchor_account() {
	let mollusk = create_mollusk();
	let create_key = key(17);
	let rent_payer = key(18);
	let (multisig_key, bump) = multisig_pda(&create_key);
	let (config_key, config_bump) = program_config_pda();
	let legacy_key = key(19);
	let members = sorted_member_keys();

	// Hand-build a multisig account in the classic Anchor layout.
	let mut legacy = Vec::new();
	legacy.extend_from_slice(&legacy_discriminator());
	legacy.extend_from_slice(&[9_u8; 32]); // create_key
	legacy.extend_from_slice(&[0_u8; 32]); // autonomous config authority
	legacy.extend_from_slice(&2_u16.to_le_bytes());
	legacy.extend_from_slice(&0_u32.to_le_bytes());
	legacy.extend_from_slice(&5_u64.to_le_bytes());
	legacy.extend_from_slice(&5_u64.to_le_bytes());
	legacy.push(0); // rent_collector: None
	legacy.extend_from_slice(&[0_u8; 32]);
	legacy.push(255); // bump
	legacy.extend_from_slice(&3_u32.to_le_bytes());
	for member in &members {
		legacy.extend_from_slice(member.as_ref());
		legacy.push(PERMISSIONS_ALL);
	}

	let mut data = vec![0_u8; multisig_program::MultisigImportIx::SIZE];
	multisig_program::MultisigImportIx::initialize(&mut data, |ix| {
		ix.bump = bump;
		ix.legacy_program = pina_address(&legacy_program_id());
		ix.legacy_discriminator = legacy_discriminator();
		ix.set_config_authority = false.into();
		ix.config_authority = Address::default();
		ix.set_rent_collector = false.into();
		ix.rent_collector = Address::default();
		Ok(())
	})
	.unwrap();

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&data,
		vec![
			AccountMeta::new_readonly(legacy_key, false),
			AccountMeta::new_readonly(config_key, false),
			AccountMeta::new_readonly(create_key, true),
			AccountMeta::new(multisig_key, false),
			AccountMeta::new(rent_payer, true),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
			AccountMeta::new_readonly(program_id(), false), // no treasury
		],
	);
	let mut world = World::new();
	world.add(
		legacy_key,
		Account {
			lamports: 1,
			data: legacy,
			owner: legacy_program_id(),
			executable: false,
			rent_epoch: 0,
		},
	);
	world.add(
		config_key,
		stored_account(program_config_data(&key(1), &key(2), 0, config_bump), 1),
	);
	world.add(multisig_key, Account::default());
	world.add(create_key, system_account(RENT_LAMPORTS));
	world.add(rent_payer, system_account(RENT_LAMPORTS));
	for member in &members {
		world.add(*member, system_account(RENT_LAMPORTS));
	}
	world.add(
		solana_sdk_ids::system_program::id(),
		keyed_account_for_system_program().1,
	);

	let result = world.run(&mollusk, &instruction, &[Check::success()]);

	let stored = account(&result, &multisig_key);
	let state = Multisig::try_from_bytes(stored.data.as_slice())
		.unwrap_or_else(|error| panic!("decode imported multisig: {error:?}"));
	assert_eq!(state.threshold.get(), 2);
	let roster = decode_fixture_roster(state.member_roster());
	assert_eq!(roster.1, 3);
	assert_eq!(roster.0[2], pina_address(&members[2]));
	assert_eq!(state.create_key, pina_address(&create_key));
}

/// Shared fixture for the proposal lifecycle tests: a three-member,
/// threshold-two multisig with one draft proposal at index 1.
fn lifecycle_world(
	now: i64,
	proposal: Option<(&[u8], u8, i64)>,
) -> (Mollusk, World, Pubkey, Pubkey, [Pubkey; 3], u8) {
	let mut mollusk = create_mollusk();
	mollusk.sysvars.clock.unix_timestamp = now;

	let create_key = key(21);
	let members = sorted_member_keys();
	let (multisig_key, multisig_bump) = multisig_pda(&create_key);
	let (proposal_key, proposal_bump) = proposal_pda(&multisig_key, 1);
	let (_, _vault_bump) = vault_pda(&multisig_key, 0);

	let mut world = World::new();
	world.add(
		multisig_key,
		stored_account(
			multisig_data(&create_key, 2, 0, 0, &members, multisig_bump, 1),
			RENT_LAMPORTS,
		),
	);
	let (message, status, status_at) = proposal.unwrap_or((
		&[0, 0, 0, 0, 0, 1], // a minimal valid vault message
		STATUS_DRAFT,
		now,
	));
	world.add(
		proposal_key,
		stored_account(
			proposal_data(
				&multisig_key,
				&members[0],
				1,
				KIND_VAULT,
				status,
				status_at,
				Some(message),
				None,
				proposal_bump,
			),
			RENT_LAMPORTS,
		),
	);
	for member in &members {
		world.add(*member, system_account(RENT_LAMPORTS));
	}
	let (clock_key, clock_account) = mollusk.sysvars.keyed_account_for_clock_sysvar();
	world.add(clock_key, clock_account);
	let (system_key, system_account) = keyed_account_for_system_program();
	world.add(system_key, system_account);

	(
		mollusk,
		world,
		multisig_key,
		proposal_key,
		members,
		proposal_bump,
	)
}

fn proposal_instruction(
	discriminant: u8,
	multisig_key: &Pubkey,
	proposal_key: &Pubkey,
	member: &Pubkey,
) -> Instruction {
	Instruction::new_with_bytes(
		program_id(),
		&empty_ix_data(discriminant),
		vec![
			AccountMeta::new_readonly(*multisig_key, false),
			AccountMeta::new(*proposal_key, false),
			AccountMeta::new_readonly(*member, true),
			clock_meta(),
		],
	)
}

#[test]
#[ignore = "build the SBF artifact first"]
fn proposal_lifecycle_approves_at_threshold() {
	let now = 1_700_000_000;
	let (mollusk, mut world, multisig_key, proposal_key, members, _) = lifecycle_world(now, None);

	world.run(
		&mollusk,
		&proposal_instruction(
			MultisigInstruction::ProposalActivate as u8,
			&multisig_key,
			&proposal_key,
			&members[0],
		),
		&[Check::success()],
	);

	// A repeat approval while still active is a duplicate vote.
	world.run(
		&mollusk,
		&proposal_instruction(
			MultisigInstruction::ProposalApprove as u8,
			&multisig_key,
			&proposal_key,
			&members[0],
		),
		&[Check::success()],
	);
	world.run(
		&mollusk,
		&proposal_instruction(
			MultisigInstruction::ProposalApprove as u8,
			&multisig_key,
			&proposal_key,
			&members[0],
		),
		&[Check::err(MultisigError::AlreadyVoted.into())],
	);

	// The second approval settles the threshold of two.
	world.run(
		&mollusk,
		&proposal_instruction(
			MultisigInstruction::ProposalApprove as u8,
			&multisig_key,
			&proposal_key,
			&members[1],
		),
		&[Check::success()],
	);
	let stored = world.get(&proposal_key);
	let state = Proposal::try_from_bytes(stored.data.as_slice())
		.unwrap_or_else(|error| panic!("decode approved proposal: {error:?}"));
	assert_eq!(state.status, STATUS_APPROVED);
	assert_eq!(state.approved_mask.get(), 0b011);

	// Voting on a settled proposal is a status error, not a duplicate.
	world.run(
		&mollusk,
		&proposal_instruction(
			MultisigInstruction::ProposalApprove as u8,
			&multisig_key,
			&proposal_key,
			&members[2],
		),
		&[Check::err(MultisigError::InvalidProposalStatus.into())],
	);
}

#[test]
#[ignore = "build the SBF artifact first"]
fn proposal_revoke_returns_a_settled_proposal_to_active() {
	let now = 1_700_000_000;
	let (mollusk, mut world, multisig_key, proposal_key, members, _) = lifecycle_world(now, None);

	world.run(
		&mollusk,
		&proposal_instruction(
			MultisigInstruction::ProposalActivate as u8,
			&multisig_key,
			&proposal_key,
			&members[0],
		),
		&[Check::success()],
	);
	for member in &members[..2] {
		world.run(
			&mollusk,
			&proposal_instruction(
				MultisigInstruction::ProposalApprove as u8,
				&multisig_key,
				&proposal_key,
				member,
			),
			&[Check::success()],
		);
	}

	// Revoking one approval drops below the threshold: active again.
	world.run(
		&mollusk,
		&proposal_instruction(
			MultisigInstruction::ProposalRevoke as u8,
			&multisig_key,
			&proposal_key,
			&members[0],
		),
		&[Check::success()],
	);
	let stored = world.get(&proposal_key);
	let state = Proposal::try_from_bytes(stored.data.as_slice())
		.unwrap_or_else(|error| panic!("decode revoked proposal: {error:?}"));
	assert_eq!(state.status, STATUS_ACTIVE);
	assert_eq!(state.approved_mask.get(), 0b010);
}

#[test]
#[ignore = "build the SBF artifact first"]
fn vault_execute_transfers_sol_through_the_vault_pda() {
	let now = 1_700_000_000;
	let (mollusk, mut world, multisig_key, proposal_key, members, proposal_bump) =
		lifecycle_world(now, None);

	// Rewrite the proposal as an approved vault transfer from the vault PDA.
	let (vault_key, _vault_bump) = vault_pda(&multisig_key, 0);
	let destination = key(30);
	let message = encode_message_fixture(
		&[vault_key, destination, solana_sdk_ids::system_program::id()],
		&[(2, &[0, 1], &system_transfer_data(2_000_000))],
	);
	world.add(
		proposal_key,
		stored_account(
			proposal_data(
				&multisig_key,
				&members[0],
				1,
				KIND_VAULT,
				STATUS_APPROVED,
				now - 1,
				Some(&message),
				None,
				proposal_bump,
			),
			RENT_LAMPORTS,
		),
	);
	world.add(vault_key, system_account(VAULT_LAMPORTS));
	world.add(destination, system_account(0));

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&empty_ix_data(MultisigInstruction::VaultExecute as u8),
		vec![
			AccountMeta::new_readonly(multisig_key, false),
			AccountMeta::new(proposal_key, false),
			AccountMeta::new_readonly(members[2], true),
			clock_meta(),
			// Message accounts: vault, destination, system program.
			AccountMeta::new(vault_key, false),
			AccountMeta::new(destination, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);
	let result = world.run(&mollusk, &instruction, &[Check::success()]);

	assert_eq!(account(&result, &destination).lamports, 2_000_000);
	assert_eq!(
		account(&result, &vault_key).lamports,
		VAULT_LAMPORTS - 2_000_000
	);
	let stored = account(&result, &proposal_key);
	let state = Proposal::try_from_bytes(stored.data.as_slice())
		.unwrap_or_else(|error| panic!("decode executed proposal: {error:?}"));
	assert_eq!(state.status, STATUS_EXECUTED);
}

#[test]
#[ignore = "build the SBF artifact first"]
fn vault_execute_rejects_a_message_that_reuses_the_proposal_account() {
	let now = 1_700_000_000;
	let (mollusk, mut world, multisig_key, proposal_key, members, proposal_bump) =
		lifecycle_world(now, None);

	// The message declares the proposal account itself as writable. Pina's
	// account parser refuses the duplicated writable account before any state
	// is read, so the on-chain protected-account check behind it stays
	// defense-in-depth for callers this parser cannot express.
	let message = encode_message_fixture(
		&[proposal_key, solana_sdk_ids::system_program::id()],
		&[(1, &[0], &[])],
	);
	world.add(
		proposal_key,
		stored_account(
			proposal_data(
				&multisig_key,
				&members[0],
				1,
				KIND_VAULT,
				STATUS_APPROVED,
				now - 1,
				Some(&message),
				None,
				proposal_bump,
			),
			RENT_LAMPORTS,
		),
	);

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&empty_ix_data(MultisigInstruction::VaultExecute as u8),
		vec![
			AccountMeta::new_readonly(multisig_key, false),
			AccountMeta::new(proposal_key, false),
			AccountMeta::new_readonly(members[2], true),
			clock_meta(),
			AccountMeta::new(proposal_key, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);
	world.run(
		&mollusk,
		&instruction,
		&[Check::err(
			pina::PinaProgramError::DuplicateMutableAccount.into(),
		)],
	);
}

#[test]
#[ignore = "build the SBF artifact first"]
fn config_execute_adds_a_member_and_invalidates_prior_proposals() {
	let now = 1_700_000_000;
	let mut mollusk = create_mollusk();
	mollusk.sysvars.clock.unix_timestamp = now;

	let create_key = key(21);
	let members = sorted_member_keys();
	let (multisig_key, multisig_bump) = multisig_pda(&create_key);
	let (proposal_key, proposal_bump) = proposal_pda(&multisig_key, 1);
	let new_member = key(40);

	// One add-member action.
	let mut actions = vec![1_u8, multisig_program::ACTION_ADD_MEMBER];
	actions.extend_from_slice(new_member.as_ref());
	actions.push(PERMISSIONS_ALL);

	let mut world = World::new();
	world.add(
		multisig_key,
		stored_account(
			multisig_data(&create_key, 2, 0, 0, &members, multisig_bump, 1),
			RENT_LAMPORTS,
		),
	);
	world.add(
		proposal_key,
		stored_account(
			proposal_data(
				&multisig_key,
				&members[0],
				1,
				KIND_CONFIG,
				STATUS_APPROVED,
				now - 1,
				None,
				Some(&actions),
				proposal_bump,
			),
			RENT_LAMPORTS,
		),
	);
	for member in &members {
		world.add(*member, system_account(RENT_LAMPORTS));
	}
	let (clock_key, clock_account) = mollusk.sysvars.keyed_account_for_clock_sysvar();
	world.add(clock_key, clock_account);
	let (system_key, system_account) = keyed_account_for_system_program();
	world.add(system_key, system_account);

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&empty_ix_data(MultisigInstruction::ConfigExecute as u8),
		vec![
			AccountMeta::new(multisig_key, false),
			AccountMeta::new(proposal_key, false),
			AccountMeta::new_readonly(members[2], true),
			AccountMeta::new(members[0], true),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
			clock_meta(),
		],
	);
	let result = world.run(&mollusk, &instruction, &[Check::success()]);

	let stored = account(&result, &multisig_key);
	let state = Multisig::try_from_bytes(stored.data.as_slice())
		.unwrap_or_else(|error| panic!("decode resized multisig: {error:?}"));
	let roster = decode_fixture_roster(state.member_roster());
	assert_eq!(roster.1, 4);
	assert!(roster.0[..4].contains(&pina_address(&new_member)));
	// The consensus moved, so every prior proposal is stale.
	assert_eq!(state.stale_transaction_index.get(), 1);

	let proposal = account(&result, &proposal_key);
	let proposal_state = Proposal::try_from_bytes(proposal.data.as_slice())
		.unwrap_or_else(|error| panic!("decode executed config proposal: {error:?}"));
	assert_eq!(proposal_state.status, STATUS_EXECUTED);
}

#[test]
#[ignore = "build the SBF artifact first"]
fn spending_limit_use_resets_the_period_and_moves_sol() {
	let now = 1_700_000_000;
	let mut mollusk = create_mollusk();
	mollusk.sysvars.clock.unix_timestamp = now;

	let create_key = key(21);
	let members = sorted_member_keys();
	let (multisig_key, multisig_bump) = multisig_pda(&create_key);
	let (vault_key, _) = vault_pda(&multisig_key, 0);
	let limit_create_key = key(50);
	let (limit_key, limit_bump) = spending_limit_pda(&multisig_key, &limit_create_key);
	let destination = key(60);
	let day = 24 * 60 * 60;

	let mut member_addresses = [Address::default(); 16];
	for (position, member) in members.iter().enumerate() {
		member_addresses[position] = pina_address(member);
	}
	let space = SpendingLimit::projected_bytes(3 * 32, 0).unwrap();
	let mut limit_account = vec![0_u8; space];
	SpendingLimit::initialize(
		&mut limit_account,
		&SpendingLimitPatch::new()
			.bump(limit_bump)
			.multisig(pina_address(&multisig_key))
			.create_key(pina_address(&limit_create_key))
			.vault_index(0)
			.vault_bump(vault_pda(&multisig_key, 0).1)
			.mint(Address::default())
			.amount(100)
			.remaining_amount(0)
			.last_reset(now - 2 * day)
			.period(PERIOD_DAY)
			.replace_members(&flatten_fixture_roster(&member_addresses[..3])[..3 * 32]),
	)
	.unwrap();

	let mut world = World::new();
	world.add(
		multisig_key,
		stored_account(
			multisig_data(&create_key, 2, 0, 0, &members, multisig_bump, 0),
			RENT_LAMPORTS,
		),
	);
	world.add(limit_key, stored_account(limit_account, RENT_LAMPORTS));
	world.add(vault_key, system_account(VAULT_LAMPORTS));
	world.add(destination, system_account(0));
	for member in &members {
		world.add(*member, system_account(RENT_LAMPORTS));
	}
	let (clock_key, clock_account) = mollusk.sysvars.keyed_account_for_clock_sysvar();
	world.add(clock_key, clock_account);
	let (system_key, system_account) = keyed_account_for_system_program();
	world.add(system_key, system_account);

	let mut data = vec![0_u8; SpendingLimitUseIx::SIZE];
	SpendingLimitUseIx::initialize(&mut data, |ix| {
		ix.amount.set(40);
		ix.decimals = 9;
		Ok(())
	})
	.unwrap();

	let filler = program_id();
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&data,
		vec![
			AccountMeta::new_readonly(multisig_key, false),
			AccountMeta::new(limit_key, false),
			AccountMeta::new_readonly(members[0], true),
			AccountMeta::new(vault_key, false),
			AccountMeta::new(destination, false),
			clock_meta(),
			AccountMeta::new_readonly(filler, false), // no vault token account
			AccountMeta::new_readonly(filler, false), // no mint
			AccountMeta::new_readonly(filler, false), // no token program
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);
	let result = world.run(&mollusk, &instruction, &[Check::success()]);

	assert_eq!(account(&result, &destination).lamports, 40);
	let stored = account(&result, &limit_key);
	let state = SpendingLimit::try_from_bytes(stored.data.as_slice())
		.unwrap_or_else(|error| panic!("decode drawn limit: {error:?}"));
	// The two-day-old anchor rolled two whole days and reset before the draw.
	assert_eq!(state.remaining_amount.get(), 60);
	assert_eq!(state.last_reset.get(), now - 2 * day + 2 * day);
}

#[test]
#[ignore = "build the SBF artifact first"]
fn proposal_close_refunds_the_rent_collector() {
	let now = 1_700_000_000;
	let mut mollusk = create_mollusk();
	mollusk.sysvars.clock.unix_timestamp = now;

	let create_key = key(21);
	let members = sorted_member_keys();
	let (multisig_key, multisig_bump) = multisig_pda(&create_key);
	let (proposal_key, proposal_bump) = proposal_pda(&multisig_key, 1);
	let collector = key(70);

	let mut multisig = multisig_data(&create_key, 2, 0, 0, &members, multisig_bump, 1);
	Multisig::update(
		&mut multisig,
		&MultisigPatch::new().rent_collector(pina_address(&collector)),
	)
	.expect("set rent collector");

	let mut world = World::new();
	world.add(multisig_key, stored_account(multisig, RENT_LAMPORTS));
	world.add(
		proposal_key,
		stored_account(
			proposal_data(
				&multisig_key,
				&members[0],
				1,
				KIND_VAULT,
				STATUS_EXECUTED,
				now,
				None,
				None,
				proposal_bump,
			),
			RENT_LAMPORTS,
		),
	);
	world.add(collector, system_account(0));

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&empty_ix_data(MultisigInstruction::ProposalClose as u8),
		vec![
			AccountMeta::new_readonly(multisig_key, false),
			AccountMeta::new(proposal_key, false),
			AccountMeta::new(collector, false),
		],
	);
	let result = world.run(&mollusk, &instruction, &[Check::success()]);

	assert_eq!(account(&result, &collector).lamports, RENT_LAMPORTS);
	assert_eq!(account(&result, &proposal_key).lamports, 0);
}
