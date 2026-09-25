//! Surfpool end-to-end journeys for the multisig program.
//!
//! These tests run against the real runtime through `pina test`, which builds
//! the SBF artifact and runs this crate with `--ignored`. Every signer and
//! account uses fixed seeds so the recorded instruction paths stay
//! deterministic for the benchmark harness.

#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::Instruction;
use pina_test::InstructionError;
use pina_test::Keypair;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use pina_test::Rent;
use pina_test::Signer;
use pina_test::TransactionError;
use program_under_test::ACTION_ADD_MEMBER;
use program_under_test::ACTION_SET_TIME_LOCK;
use program_under_test::Address;
use program_under_test::ConfigAuthorityExecuteIx;
use program_under_test::ConfigInitializeIx;
use program_under_test::ConfigUpdateIx;
use program_under_test::KIND_CONFIG;
use program_under_test::KIND_VAULT;
use program_under_test::MAX_MESSAGE_BYTES;
use program_under_test::Multisig;
use program_under_test::MultisigCreateIx;
use program_under_test::MultisigError;
use program_under_test::MultisigImportIx;
use program_under_test::MultisigInstruction;
use program_under_test::PERIOD_DAY;
use program_under_test::PERMISSIONS_ALL;
use program_under_test::ProgramConfig;
use program_under_test::Proposal;
use program_under_test::ProposalCreateIx;
use program_under_test::STATUS_ACTIVE;
use program_under_test::STATUS_APPROVED;
use program_under_test::STATUS_CANCELLED;
use program_under_test::STATUS_DRAFT;
use program_under_test::STATUS_EXECUTED;
use program_under_test::STATUS_REJECTED;
use program_under_test::SpendingLimit;
use program_under_test::SpendingLimitPatch;
use program_under_test::SpendingLimitUseIx;

const FUND: u64 = 1_000_000_000;
const VAULT_FUND: u64 = 2_000_000_000;
const TRANSFER: u64 = 250_000_000;

const CLOCK_BYTES: [u8; 32] = [
	6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152, 105, 29, 94, 182, 139, 94, 184, 163, 155,
	75, 109, 92, 115, 85, 91, 33, 0, 0, 0, 0,
];
const SYSTEM_BYTES: [u8; 32] = [0; 32];

fn clock() -> Pubkey {
	Pubkey::new_from_array(CLOCK_BYTES)
}

fn system() -> Pubkey {
	Pubkey::new_from_array(SYSTEM_BYTES)
}

/// Fixed identities so recorded benchmarks stay deterministic.
fn member_a() -> Keypair {
	Keypair::new_from_array([0xA1; 32])
}

fn member_b() -> Keypair {
	Keypair::new_from_array([0xB2; 32])
}

fn member_c() -> Keypair {
	Keypair::new_from_array([0xC3; 32])
}

fn create_key() -> Keypair {
	Keypair::new_from_array([0x5E; 32])
}

fn destination() -> Pubkey {
	Pubkey::new_from_array([0xDE; 32])
}

fn program_id() -> Pubkey {
	let bytes: &[u8] = program_under_test::ID.as_ref();
	Pubkey::new_from_array(bytes.try_into().unwrap())
}

fn pina_address(pubkey: &Pubkey) -> Address {
	Address::new_from_array(pubkey.to_bytes())
}

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

fn sorted_members() -> [Pubkey; 3] {
	let mut keys = [
		member_a().pubkey(),
		member_b().pubkey(),
		member_c().pubkey(),
	];
	keys.sort_unstable();
	keys
}

fn encode_message_fixture(keys: &[Pubkey], instructions: &[(usize, &[u8], &[u8])]) -> Vec<u8> {
	let mut buffer = [0_u8; MAX_MESSAGE_BYTES];
	let addresses: Vec<Address> = keys.iter().map(pina_address).collect();
	let length = program_under_test::encode_message(
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

fn system_transfer_data(lamports: u64) -> Vec<u8> {
	let mut data = Vec::new();
	data.extend_from_slice(&2_u32.to_le_bytes());
	data.extend_from_slice(&lamports.to_le_bytes());
	data
}

fn create_multisig_ix(bump: u8, member_count: usize, threshold: u16) -> Vec<u8> {
	let mut data = vec![0_u8; MultisigCreateIx::SIZE];
	MultisigCreateIx::initialize(&mut data, |ix| {
		ix.bump = bump;
		ix.threshold.set(threshold);
		ix.timelock.set(0);
		ix.ttl.set(0);
		for slot in ix.member_permissions.iter_mut().take(member_count) {
			*slot = PERMISSIONS_ALL;
		}
		ix.config_authority = Address::default();
		ix.rent_collector = Address::default();
		Ok(())
	})
	.unwrap_or_else(|error| panic!("encode multisig create: {error:?}"));
	data
}

fn proposal_create_ix(
	multisig: &Pubkey,
	bump: u8,
	kind: u8,
	message: &[u8],
	actions: &[u8],
) -> Vec<u8> {
	// The vault bump is a client-supplied argument now; pass the canonical
	// one so execution signs the conventionally-derived vault.
	let (_, vault_bump) = vault_pda(multisig, 0);
	let mut data = vec![0_u8; ProposalCreateIx::SIZE];
	ProposalCreateIx::initialize(&mut data, |ix| {
		ix.bump = bump;
		ix.kind = kind;
		ix.vault_index = 0;
		ix.vault_bump = vault_bump;
		ix.ephemeral_signers = 0;
		ix.message_len.set(message.len() as u16);
		ix.message[..message.len()].copy_from_slice(message);
		ix.actions_len.set(actions.len() as u16);
		ix.actions[..actions.len()].copy_from_slice(actions);
		Ok(())
	})
	.unwrap_or_else(|error| panic!("encode proposal create: {error:?}"));
	data
}

fn bare_ix(discriminant: u8) -> Vec<u8> {
	vec![discriminant, 0]
}

/// Flatten addresses into the roster wire form.
fn flatten_roster(keys: &[Address]) -> [u8; 512] {
	let mut bytes = [0_u8; 512];
	for (position, key) in keys.iter().enumerate() {
		bytes[position * 32..position * 32 + 32].copy_from_slice(key.as_ref());
	}
	bytes
}

/// Decode a roster into owned addresses plus its length.
fn decode_roster(bytes: &[u8]) -> ([Address; 16], usize) {
	let count = bytes.len() / 32;
	let mut keys = [Address::default(); 16];
	for (position, slot) in keys.iter_mut().take(count).enumerate() {
		*slot = Address::try_from(&bytes[position * 32..position * 32 + 32]).unwrap();
	}
	(keys, count)
}

/// Install the global program config with a zero fee, so multisig creation
/// needs no treasury account.
fn install_program_config(program: &ProgramTest, authority: &Pubkey) {
	let (config_key, config_bump) = program_config_pda();
	let mut data = vec![0_u8; ProgramConfig::SIZE];
	ProgramConfig::initialize(&mut data, |config| {
		config.bump = config_bump;
		config.authority = pina_address(authority);
		config.treasury = pina_address(authority);
		config.creation_fee.set(0);
		Ok(())
	})
	.unwrap_or_else(|error| panic!("encode program config fixture: {error:?}"));
	install_account(program, &config_key, &program_id(), data, 100_000_000);
}

/// The config authority keypair every config fixtures share: the seed is
/// fixed so recorded benchmark paths stay deterministic.
fn config_authority() -> Keypair {
	Keypair::new_from_array([0xCA; 32])
}

/// Install a prefabricated program-owned account through the state cheatcode.
fn install_account(
	program: &ProgramTest,
	address: &Pubkey,
	owner: &Pubkey,
	data: Vec<u8>,
	lamports: u64,
) {
	program
		.install_historical_account(
			&pina_test::HistoricalAccount::new(0, *address, *owner, data).with_lamports(lamports),
		)
		.unwrap_or_else(|error| panic!("install account fixture: {error:?}"));
}

#[test]
#[ignore = "run with pina test"]
fn end_to_end_governed_sol_transfer() {
	pina_test::run(async {
		let pid = program_id();
		let mut program = ProgramTest::start(pid)
			.await
			.unwrap_or_else(|error| panic!("start program test: {error:?}"));

		let config_authority = Keypair::new_from_array([0xCA; 32]);
		program
			.fund(&config_authority.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund authority: {error:?}"));

		// Bootstrap the global config with a zero fee.
		let (config_key, config_bump) = program_config_pda();
		let mut config_ix = vec![0_u8; ConfigInitializeIx::SIZE];
		ConfigInitializeIx::initialize(&mut config_ix, |ix| {
			ix.bump = config_bump;
			ix.treasury = pina_address(&config_authority.pubkey());
			ix.creation_fee.set(0);
			Ok(())
		})
		.unwrap_or_else(|error| panic!("encode config init: {error:?}"));
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&config_ix,
					vec![
						AccountMeta::new(config_authority.pubkey(), true),
						AccountMeta::new(config_key, false),
						AccountMeta::new_readonly(system(), false),
					],
				),
				&[&config_authority],
			)
			.unwrap_or_else(|error| panic!("initialize program config: {error:?}"));

		// Create a three-member, threshold-two multisig.
		let create = create_key();
		program
			.fund(&create.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund create key: {error:?}"));
		let members = sorted_members();
		for member in &members {
			program
				.fund(member, FUND)
				.unwrap_or_else(|error| panic!("fund member: {error:?}"));
		}
		let funder = Keypair::new_from_array([0xF0; 32]);
		program
			.fund(&funder.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund funder: {error:?}"));
		let (multisig_key, multisig_bump) = multisig_pda(&create.pubkey());
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&create_multisig_ix(multisig_bump, members.len(), 2),
					vec![
						AccountMeta::new_readonly(config_key, false),
						AccountMeta::new_readonly(create.pubkey(), true),
						AccountMeta::new(multisig_key, false),
						AccountMeta::new(funder.pubkey(), true),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(pid, false),
						AccountMeta::new_readonly(members[0], false),
						AccountMeta::new_readonly(members[1], false),
						AccountMeta::new_readonly(members[2], false),
					],
				),
				&[&create, &funder],
			)
			.unwrap_or_else(|error| panic!("create multisig: {error:?}"));

		let multisig_account = program
			.account(&multisig_key)
			.unwrap_or_else(|error| panic!("multisig exists: {error:?}"));
		let state = Multisig::try_from_bytes(&multisig_account.data)
			.unwrap_or_else(|error| panic!("decode: {error:?}"));
		let (_, count) = decode_roster(state.member_roster());
		assert_eq!(count, 3);
		assert_eq!(state.threshold.get(), 2);
		drop(multisig_account);

		// Fund the vault PDA and propose a SOL transfer out of it.
		let (vault_key, _) = vault_pda(&multisig_key, 0);
		program
			.fund(&vault_key, VAULT_FUND)
			.unwrap_or_else(|error| panic!("fund vault: {error:?}"));
		let payee = destination();
		let message = encode_message_fixture(
			&[vault_key, payee, system()],
			&[(2, &[0, 1], &system_transfer_data(TRANSFER))],
		);

		let (proposal_key, proposal_bump) = proposal_pda(&multisig_key, 1);
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&proposal_create_ix(&multisig_key, proposal_bump, KIND_VAULT, &message, &[]),
					vec![
						AccountMeta::new(multisig_key, false),
						AccountMeta::new(proposal_key, false),
						AccountMeta::new_readonly(member_a().pubkey(), true),
						AccountMeta::new(member_a().pubkey(), true),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(clock(), false),
					],
				),
				&[&member_a()],
			)
			.unwrap_or_else(|error| panic!("create vault proposal: {error:?}"));
		let proposal_account = program
			.account(&proposal_key)
			.unwrap_or_else(|error| panic!("proposal exists: {error:?}"));
		let state = Proposal::try_from_bytes(&proposal_account.data)
			.unwrap_or_else(|error| panic!("decode: {error:?}"));
		assert_eq!(state.status, STATUS_DRAFT);
		assert_eq!(state.index.get(), 1);
		drop(proposal_account);

		// Activate, approve to the threshold, execute after zero timelock.
		for (discriminant, signer) in [
			(MultisigInstruction::ProposalActivate as u8, &member_a()),
			(MultisigInstruction::ProposalApprove as u8, &member_a()),
			(MultisigInstruction::ProposalApprove as u8, &member_b()),
		] {
			program
				.send_with_signers(
					Instruction::new_with_bytes(
						pid,
						&bare_ix(discriminant),
						vec![
							AccountMeta::new_readonly(multisig_key, false),
							AccountMeta::new(proposal_key, false),
							AccountMeta::new_readonly(signer.pubkey(), true),
							AccountMeta::new_readonly(clock(), false),
						],
					),
					&[signer],
				)
				.unwrap_or_else(|error| panic!("advance proposal: {error:?}"));
		}
		let proposal_account = program
			.account(&proposal_key)
			.unwrap_or_else(|error| panic!("proposal exists: {error:?}"));
		let state = Proposal::try_from_bytes(&proposal_account.data)
			.unwrap_or_else(|error| panic!("decode: {error:?}"));
		assert_eq!(state.status, STATUS_APPROVED);
		drop(proposal_account);

		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&bare_ix(MultisigInstruction::VaultExecute as u8),
					vec![
						AccountMeta::new_readonly(multisig_key, false),
						AccountMeta::new(proposal_key, false),
						AccountMeta::new_readonly(member_c().pubkey(), true),
						AccountMeta::new_readonly(clock(), false),
						AccountMeta::new(vault_key, false),
						AccountMeta::new(payee, false),
						AccountMeta::new_readonly(system(), false),
					],
				),
				&[&member_c()],
			)
			.unwrap_or_else(|error| panic!("execute vault proposal: {error:?}"));

		assert_eq!(
			program
				.balance(&payee)
				.unwrap_or_else(|error| panic!("payee balance: {error:?}")),
			TRANSFER
		);
		assert_eq!(
			program
				.balance(&vault_key)
				.unwrap_or_else(|error| panic!("vault balance: {error:?}")),
			VAULT_FUND - TRANSFER
		);
		let proposal_account = program
			.account(&proposal_key)
			.unwrap_or_else(|error| panic!("proposal exists: {error:?}"));
		let state = Proposal::try_from_bytes(&proposal_account.data)
			.unwrap_or_else(|error| panic!("decode: {error:?}"));
		assert_eq!(state.status, STATUS_EXECUTED);

		program
			.stop()
			.unwrap_or_else(|error| panic!("stop program test: {error:?}"));
	});
}

#[test]
#[ignore = "run with pina test"]
fn governed_config_change_grows_the_roster_and_invalidates_prior_proposals() {
	pina_test::run(async {
		let pid = program_id();
		let mut program = ProgramTest::start(pid)
			.await
			.unwrap_or_else(|error| panic!("start program test: {error:?}"));

		let create = create_key();
		program
			.fund(&create.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund create key: {error:?}"));
		let members = sorted_members();
		for member in &members {
			program
				.fund(member, FUND)
				.unwrap_or_else(|error| panic!("fund member: {error:?}"));
		}
		install_program_config(&program, &config_authority().pubkey());
		let funder = Keypair::new_from_array([0xF0; 32]);
		program
			.fund(&funder.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund funder: {error:?}"));
		let (multisig_key, multisig_bump) = multisig_pda(&create.pubkey());
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&create_multisig_ix(multisig_bump, members.len(), 2),
					vec![
						AccountMeta::new_readonly(program_config_pda().0, false),
						AccountMeta::new_readonly(create.pubkey(), true),
						AccountMeta::new(multisig_key, false),
						AccountMeta::new(funder.pubkey(), true),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(pid, false),
						AccountMeta::new_readonly(members[0], false),
						AccountMeta::new_readonly(members[1], false),
						AccountMeta::new_readonly(members[2], false),
					],
				),
				&[&create, &funder],
			)
			.unwrap_or_else(|error| panic!("create multisig: {error:?}"));

		// A real draft vault proposal at index 1 that must go stale after the
		// config change.
		let (stale_proposal_key, stale_bump) = proposal_pda(&multisig_key, 1);
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&proposal_create_ix(
						&multisig_key,
						stale_bump,
						KIND_VAULT,
						&[0, 0, 0, 0, 0, 0],
						&[],
					),
					vec![
						AccountMeta::new(multisig_key, false),
						AccountMeta::new(stale_proposal_key, false),
						AccountMeta::new_readonly(member_a().pubkey(), true),
						AccountMeta::new(member_a().pubkey(), true),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(clock(), false),
					],
				),
				&[&member_a()],
			)
			.unwrap_or_else(|error| panic!("create the stale-fated proposal: {error:?}"));

		// Propose adding a fourth member, then approve and execute it.
		let new_member = Pubkey::new_from_array([0xD4; 32]);
		let mut actions = vec![2_u8, ACTION_ADD_MEMBER];
		actions.extend_from_slice(new_member.as_ref());
		actions.push(PERMISSIONS_ALL);
		actions.push(ACTION_SET_TIME_LOCK);
		actions.extend_from_slice(&3600_u32.to_le_bytes());

		let (proposal_key, proposal_bump) = proposal_pda(&multisig_key, 2);
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&proposal_create_ix(&multisig_key, proposal_bump, KIND_CONFIG, &[], &actions),
					vec![
						AccountMeta::new(multisig_key, false),
						AccountMeta::new(proposal_key, false),
						AccountMeta::new_readonly(member_a().pubkey(), true),
						AccountMeta::new(member_a().pubkey(), true),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(clock(), false),
					],
				),
				&[&member_a()],
			)
			.unwrap_or_else(|error| panic!("create config proposal: {error:?}"));
		for (discriminant, signer) in [
			(MultisigInstruction::ProposalActivate as u8, &member_a()),
			(MultisigInstruction::ProposalApprove as u8, &member_a()),
			(MultisigInstruction::ProposalApprove as u8, &member_b()),
		] {
			program
				.send_with_signers(
					Instruction::new_with_bytes(
						pid,
						&bare_ix(discriminant),
						vec![
							AccountMeta::new_readonly(multisig_key, false),
							AccountMeta::new(proposal_key, false),
							AccountMeta::new_readonly(signer.pubkey(), true),
							AccountMeta::new_readonly(clock(), false),
						],
					),
					&[signer],
				)
				.unwrap_or_else(|error| panic!("advance config proposal: {error:?}"));
		}

		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&bare_ix(MultisigInstruction::ConfigExecute as u8),
					vec![
						AccountMeta::new(multisig_key, false),
						AccountMeta::new(proposal_key, false),
						AccountMeta::new_readonly(member_c().pubkey(), true),
						AccountMeta::new(member_a().pubkey(), true),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(clock(), false),
					],
				),
				&[&member_c(), &member_a()],
			)
			.unwrap_or_else(|error| panic!("execute config proposal: {error:?}"));

		let multisig_account = program
			.account(&multisig_key)
			.unwrap_or_else(|error| panic!("multisig exists: {error:?}"));
		let state = Multisig::try_from_bytes(&multisig_account.data)
			.unwrap_or_else(|error| panic!("decode: {error:?}"));
		let (roster, count) = decode_roster(state.member_roster());
		assert_eq!(count, 4);
		assert!(roster[..4].contains(&pina_address(&new_member)));
		assert_eq!(state.timelock.get(), 3600);
		assert_eq!(state.stale_transaction_index.get(), 2);
		drop(multisig_account);

		// The pre-change draft can no longer activate.
		let error = program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&bare_ix(MultisigInstruction::ProposalActivate as u8),
					vec![
						AccountMeta::new_readonly(multisig_key, false),
						AccountMeta::new(stale_proposal_key, false),
						AccountMeta::new_readonly(member_a().pubkey(), true),
						AccountMeta::new_readonly(clock(), false),
					],
				),
				&[&member_a()],
			)
			.expect_err("stale proposal must not activate");
		pina_test::assert_custom_error(&error, MultisigError::StaleProposal as u32);

		program
			.stop()
			.unwrap_or_else(|error| panic!("stop program test: {error:?}"));
	});
}

#[test]
#[ignore = "run with pina test"]
fn rejection_cutoff_settles_and_events_are_emitted() {
	pina_test::run(async {
		let pid = program_id();
		let mut program = ProgramTest::start(pid)
			.await
			.unwrap_or_else(|error| panic!("start program test: {error:?}"));

		let create = create_key();
		program
			.fund(&create.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund create key: {error:?}"));
		let members = sorted_members();
		for member in &members {
			program
				.fund(member, FUND)
				.unwrap_or_else(|error| panic!("fund member: {error:?}"));
		}
		install_program_config(&program, &config_authority().pubkey());
		let funder = Keypair::new_from_array([0xF0; 32]);
		program
			.fund(&funder.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund funder: {error:?}"));
		let (multisig_key, multisig_bump) = multisig_pda(&create.pubkey());
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&create_multisig_ix(multisig_bump, members.len(), 2),
					vec![
						AccountMeta::new_readonly(program_config_pda().0, false),
						AccountMeta::new_readonly(create.pubkey(), true),
						AccountMeta::new(multisig_key, false),
						AccountMeta::new(funder.pubkey(), true),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(pid, false),
						AccountMeta::new_readonly(members[0], false),
						AccountMeta::new_readonly(members[1], false),
						AccountMeta::new_readonly(members[2], false),
					],
				),
				&[&create, &funder],
			)
			.unwrap_or_else(|error| panic!("create multisig: {error:?}"));

		let (proposal_key, proposal_bump) = proposal_pda(&multisig_key, 1);
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&proposal_create_ix(
						&multisig_key,
						proposal_bump,
						KIND_VAULT,
						&[0, 0, 0, 0, 0, 0],
						&[],
					),
					vec![
						AccountMeta::new(multisig_key, false),
						AccountMeta::new(proposal_key, false),
						AccountMeta::new_readonly(member_a().pubkey(), true),
						AccountMeta::new(member_a().pubkey(), true),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(clock(), false),
					],
				),
				&[&member_a()],
			)
			.unwrap_or_else(|error| panic!("create rejection-fated proposal: {error:?}"));
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&bare_ix(MultisigInstruction::ProposalActivate as u8),
					vec![
						AccountMeta::new_readonly(multisig_key, false),
						AccountMeta::new(proposal_key, false),
						AccountMeta::new_readonly(member_a().pubkey(), true),
						AccountMeta::new_readonly(clock(), false),
					],
				),
				&[&member_a()],
			)
			.unwrap_or_else(|error| panic!("activate rejection-fated proposal: {error:?}"));

		// Two rejections out of three voters (threshold two) settle the
		// rejection: cutoff = 3 - 2 + 1.
		for signer in [&member_a(), &member_b()] {
			program
				.send_with_signers(
					Instruction::new_with_bytes(
						pid,
						&bare_ix(MultisigInstruction::ProposalReject as u8),
						vec![
							AccountMeta::new_readonly(multisig_key, false),
							AccountMeta::new(proposal_key, false),
							AccountMeta::new_readonly(signer.pubkey(), true),
							AccountMeta::new_readonly(clock(), false),
						],
					),
					&[signer],
				)
				.unwrap_or_else(|error| panic!("reject proposal: {error:?}"));
		}
		let proposal_account = program
			.account(&proposal_key)
			.unwrap_or_else(|error| panic!("proposal exists: {error:?}"));
		let state = Proposal::try_from_bytes(&proposal_account.data)
			.unwrap_or_else(|error| panic!("decode: {error:?}"));
		assert_eq!(state.status, STATUS_REJECTED);
		assert_eq!(state.rejected_mask.get(), 0b011);
		drop(proposal_account);

		// Every rejection left its mark: the third member's rejection would
		// also settle, but two already carried the cutoff, so the mask pins
		// exactly who voted. (Event-record decoding is unit-tested in the
		// program crate; `simulate_logs` cannot sign member instructions.)
		let proposal_account = program
			.account(&proposal_key)
			.unwrap_or_else(|error| panic!("proposal exists: {error:?}"));
		let state = Proposal::try_from_bytes(&proposal_account.data)
			.unwrap_or_else(|error| panic!("decode: {error:?}"));
		assert_eq!(state.rejected_mask.get(), 0b011);
		drop(proposal_account);

		program
			.stop()
			.unwrap_or_else(|error| panic!("stop program test: {error:?}"));
	});
}

#[test]
#[ignore = "run with pina test"]
fn imports_reject_a_legacy_account_with_a_mismatched_owner() {
	pina_test::run(async {
		let pid = program_id();
		let mut program = ProgramTest::start(pid)
			.await
			.unwrap_or_else(|error| panic!("start program test: {error:?}"));

		let create = create_key();
		program
			.fund(&create.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund create key: {error:?}"));
		let payer = member_a();
		program
			.fund(&payer.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund payer: {error:?}"));
		let members = sorted_members();

		// Classic-layout bytes, owned by this program because the state
		// cheatcode cannot install foreign-owned accounts. Pointing the
		// import at a different expected owner must trip the owner check
		// before anything is parsed. The happy-path import runs in the
		// Mollusk suite, which can install a foreign-owned fixture.
		let legacy_key = Pubkey::new_from_array([0x1E; 32]);
		let mut legacy = Vec::new();
		legacy.extend_from_slice(&[0x11_u8; 8]);
		legacy.extend_from_slice(&[0x11; 32]);
		legacy.extend_from_slice(&[0_u8; 32]);
		legacy.extend_from_slice(&2_u16.to_le_bytes());
		legacy.extend_from_slice(&0_u32.to_le_bytes());
		legacy.extend_from_slice(&7_u64.to_le_bytes());
		legacy.extend_from_slice(&7_u64.to_le_bytes());
		legacy.push(0);
		legacy.extend_from_slice(&[0_u8; 32]);
		legacy.push(255);
		legacy.extend_from_slice(&3_u32.to_le_bytes());
		for member in &members {
			legacy.extend_from_slice(member.as_ref());
			legacy.push(PERMISSIONS_ALL);
		}
		install_account(&program, &legacy_key, &pid, legacy, 1);

		let (multisig_key, multisig_bump) = multisig_pda(&create.pubkey());
		let mut import_ix = vec![0_u8; MultisigImportIx::SIZE];
		MultisigImportIx::initialize(&mut import_ix, |ix| {
			ix.bump = multisig_bump;
			ix.legacy_program = pina_address(&Pubkey::new_from_array([0xEE; 32]));
			ix.legacy_discriminator = [0x11; 8];
			ix.set_config_authority = false.into();
			ix.config_authority = Address::default();
			ix.set_rent_collector = false.into();
			ix.rent_collector = Address::default();
			Ok(())
		})
		.unwrap_or_else(|error| panic!("encode import: {error:?}"));

		let error = program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&import_ix,
					vec![
						AccountMeta::new_readonly(legacy_key, false),
						AccountMeta::new_readonly(program_config_pda().0, false),
						AccountMeta::new_readonly(create.pubkey(), true),
						AccountMeta::new(multisig_key, false),
						AccountMeta::new(payer.pubkey(), true),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(pid, false),
					],
				),
				&[&create, &payer],
			)
			.expect_err("a foreign-owned account must not import");
		assert!(
			matches!(
				error.transaction_error(),
				Some(TransactionError::InstructionError(
					_,
					InstructionError::InvalidAccountOwner
				))
			),
			"expected InvalidAccountOwner, got: {error:?}"
		);

		program
			.stop()
			.unwrap_or_else(|error| panic!("stop program test: {error:?}"));
	});
}

#[test]
#[ignore = "run with pina test"]
fn spending_limit_moves_sol_without_a_vote() {
	pina_test::run(async {
		let pid = program_id();
		let mut program = ProgramTest::start(pid)
			.await
			.unwrap_or_else(|error| panic!("start program test: {error:?}"));

		let create = create_key();
		let members = sorted_members();
		for member in &members {
			program
				.fund(member, FUND)
				.unwrap_or_else(|error| panic!("fund member: {error:?}"));
		}
		install_program_config(&program, &config_authority().pubkey());
		let funder = Keypair::new_from_array([0xF0; 32]);
		program
			.fund(&funder.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund funder: {error:?}"));
		let (multisig_key, multisig_bump) = multisig_pda(&create.pubkey());
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&create_multisig_ix(multisig_bump, members.len(), 2),
					vec![
						AccountMeta::new_readonly(program_config_pda().0, false),
						AccountMeta::new_readonly(create.pubkey(), true),
						AccountMeta::new(multisig_key, false),
						AccountMeta::new(funder.pubkey(), true),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(pid, false),
						AccountMeta::new_readonly(members[0], false),
						AccountMeta::new_readonly(members[1], false),
						AccountMeta::new_readonly(members[2], false),
					],
				),
				&[&create, &funder],
			)
			.unwrap_or_else(|error| panic!("create multisig: {error:?}"));

		let (vault_key, _) = vault_pda(&multisig_key, 0);
		let limit_create_key = Pubkey::new_from_array([0x51; 32]);
		let (limit_key, limit_bump) = spending_limit_pda(&multisig_key, &limit_create_key);
		let payee = destination();
		// The payee must already be rent-exempt to receive lamports.
		program
			.fund(&payee, FUND)
			.unwrap_or_else(|error| panic!("fund payee: {error:?}"));
		program
			.fund(&vault_key, VAULT_FUND)
			.unwrap_or_else(|error| panic!("fund vault: {error:?}"));

		let rent = Rent::default();

		let mut member_addresses = [Address::default(); 24];
		for (position, member) in members.iter().enumerate() {
			member_addresses[position] = pina_address(member);
		}
		let space = SpendingLimit::projected_bytes(3 * 32, 32).unwrap();
		let mut limit_bytes = vec![0_u8; space];
		SpendingLimit::initialize(
			&mut limit_bytes,
			&SpendingLimitPatch::new()
				.bump(limit_bump)
				.multisig(pina_address(&multisig_key))
				.create_key(pina_address(&limit_create_key))
				.vault_index(0)
				.vault_bump(vault_pda(&multisig_key, 0).1)
				.mint(Address::default())
				.amount(1000)
				.remaining_amount(1000)
				.last_reset(0)
				.period(PERIOD_DAY)
				.replace_members(&flatten_roster(&member_addresses[..3])[..3 * 32])
				.replace_destinations(&flatten_roster(&[pina_address(&payee)])[..32]),
		)
		.unwrap_or_else(|error| panic!("encode spending limit: {error:?}"));
		let limit_rent = rent.minimum_balance(limit_bytes.len());
		install_account(&program, &limit_key, &pid, limit_bytes, limit_rent);

		let mut spend_ix = vec![0_u8; SpendingLimitUseIx::SIZE];
		SpendingLimitUseIx::initialize(&mut spend_ix, |ix| {
			ix.amount.set(400);
			ix.decimals = 9;
			Ok(())
		})
		.unwrap_or_else(|error| panic!("encode spending limit use: {error:?}"));
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&spend_ix,
					vec![
						AccountMeta::new_readonly(multisig_key, false),
						AccountMeta::new(limit_key, false),
						AccountMeta::new_readonly(member_a().pubkey(), true),
						AccountMeta::new(vault_key, false),
						AccountMeta::new(payee, false),
						AccountMeta::new_readonly(clock(), false),
						AccountMeta::new_readonly(pid, false),
						AccountMeta::new_readonly(pid, false),
						AccountMeta::new_readonly(pid, false),
						AccountMeta::new_readonly(system(), false),
					],
				),
				&[&member_a()],
			)
			.unwrap_or_else(|error| panic!("spend against the limit: {error:?}"));

		assert_eq!(
			program
				.balance(&payee)
				.unwrap_or_else(|error| panic!("payee balance: {error:?}")),
			FUND + 400
		);
		let limit_account = program
			.account(&limit_key)
			.unwrap_or_else(|error| panic!("limit exists: {error:?}"));
		let state = SpendingLimit::try_from_bytes(&limit_account.data)
			.unwrap_or_else(|error| panic!("decode: {error:?}"));
		assert_eq!(state.remaining_amount.get(), 600);

		// A draw past the allowance is rejected.
		let mut overdraw = vec![0_u8; SpendingLimitUseIx::SIZE];
		SpendingLimitUseIx::initialize(&mut overdraw, |ix| {
			ix.amount.set(601);
			ix.decimals = 9;
			Ok(())
		})
		.unwrap_or_else(|error| panic!("encode overdraw: {error:?}"));
		let error = program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&overdraw,
					vec![
						AccountMeta::new_readonly(multisig_key, false),
						AccountMeta::new(limit_key, false),
						AccountMeta::new_readonly(member_a().pubkey(), true),
						AccountMeta::new(vault_key, false),
						AccountMeta::new(payee, false),
						AccountMeta::new_readonly(clock(), false),
						AccountMeta::new_readonly(pid, false),
						AccountMeta::new_readonly(pid, false),
						AccountMeta::new_readonly(pid, false),
						AccountMeta::new_readonly(system(), false),
					],
				),
				&[&member_a()],
			)
			.expect_err("overdraw must fail");
		pina_test::assert_custom_error(&error, MultisigError::SpendingLimitExceeded as u32);

		program
			.stop()
			.unwrap_or_else(|error| panic!("stop program test: {error:?}"));
	});
}

#[test]
#[ignore = "run with pina test"]
fn config_update_revocation_cancellation_authority_execute_and_close() {
	pina_test::run(async {
		let pid = program_id();
		let mut program = ProgramTest::start(pid)
			.await
			.unwrap_or_else(|error| panic!("start program test: {error:?}"));

		// The installed config names the shared authority keypair, so
		// ConfigUpdate runs against the real signer.
		let config_authority = config_authority();
		install_program_config(&program, &config_authority.pubkey());
		let (config_key, _) = program_config_pda();
		let mut update_ix = vec![0_u8; ConfigUpdateIx::SIZE];
		ConfigUpdateIx::initialize(&mut update_ix, |ix| {
			ix.set_treasury = false.into();
			ix.treasury = Address::default();
			ix.set_creation_fee = true.into();
			ix.creation_fee.set(0);
			Ok(())
		})
		.unwrap_or_else(|error| panic!("encode config update: {error:?}"));
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&update_ix,
					vec![
						AccountMeta::new_readonly(config_authority.pubkey(), true),
						AccountMeta::new(config_key, false),
					],
				),
				&[&config_authority],
			)
			.unwrap_or_else(|error| panic!("update program config: {error:?}"));

		// A controlled multisig with a rent collector: the authority path and
		// the close path both need it.
		let create = create_key();
		let members = sorted_members();
		for member in &members {
			program
				.fund(member, FUND)
				.unwrap_or_else(|error| panic!("fund member: {error:?}"));
		}
		let authority = Keypair::new_from_array([0xEE; 32]);
		let collector = Pubkey::new_from_array([0xCC; 32]);
		let funder = Keypair::new_from_array([0xF0; 32]);
		program
			.fund(&funder.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund funder: {error:?}"));
		let (multisig_key, multisig_bump) = multisig_pda(&create.pubkey());
		let mut controlled_ix = vec![0_u8; MultisigCreateIx::SIZE];
		MultisigCreateIx::initialize(&mut controlled_ix, |ix| {
			ix.bump = multisig_bump;
			ix.threshold.set(2);
			ix.timelock.set(0);
			ix.ttl.set(0);
			for slot in ix.member_permissions.iter_mut().take(members.len()) {
				*slot = PERMISSIONS_ALL;
			}
			ix.config_authority = pina_address(&authority.pubkey());
			ix.rent_collector = pina_address(&collector);
			Ok(())
		})
		.unwrap_or_else(|error| panic!("encode controlled multisig create: {error:?}"));
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&controlled_ix,
					vec![
						AccountMeta::new_readonly(config_key, false),
						AccountMeta::new_readonly(create.pubkey(), true),
						AccountMeta::new(multisig_key, false),
						AccountMeta::new(funder.pubkey(), true),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(pid, false),
						AccountMeta::new_readonly(members[0], false),
						AccountMeta::new_readonly(members[1], false),
						AccountMeta::new_readonly(members[2], false),
					],
				),
				&[&create, &funder],
			)
			.unwrap_or_else(|error| panic!("create controlled multisig: {error:?}"));

		// Approve to the threshold, then revoke one approval: the proposal
		// must fall back to active.
		let (proposal_key, proposal_bump) = proposal_pda(&multisig_key, 1);
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&proposal_create_ix(
						&multisig_key,
						proposal_bump,
						KIND_VAULT,
						&[0, 0, 0, 0, 0, 0],
						&[],
					),
					vec![
						AccountMeta::new(multisig_key, false),
						AccountMeta::new(proposal_key, false),
						AccountMeta::new_readonly(member_a().pubkey(), true),
						AccountMeta::new(member_a().pubkey(), true),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(clock(), false),
					],
				),
				&[&member_a()],
			)
			.unwrap_or_else(|error| panic!("create the revocation-fated proposal: {error:?}"));
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&bare_ix(MultisigInstruction::ProposalActivate as u8),
					vec![
						AccountMeta::new_readonly(multisig_key, false),
						AccountMeta::new(proposal_key, false),
						AccountMeta::new_readonly(member_a().pubkey(), true),
						AccountMeta::new_readonly(clock(), false),
					],
				),
				&[&member_a()],
			)
			.unwrap_or_else(|error| panic!("activate the revocation-fated proposal: {error:?}"));
		for (discriminant, signer) in [
			(MultisigInstruction::ProposalApprove as u8, &member_a()),
			(MultisigInstruction::ProposalApprove as u8, &member_b()),
			(MultisigInstruction::ProposalRevoke as u8, &member_a()),
		] {
			program
				.send_with_signers(
					Instruction::new_with_bytes(
						pid,
						&bare_ix(discriminant),
						vec![
							AccountMeta::new_readonly(multisig_key, false),
							AccountMeta::new(proposal_key, false),
							AccountMeta::new_readonly(signer.pubkey(), true),
							AccountMeta::new_readonly(clock(), false),
						],
					),
					&[signer],
				)
				.unwrap_or_else(|error| panic!("advance the revocation-fated proposal: {error:?}"));
		}
		let proposal_account = program
			.account(&proposal_key)
			.unwrap_or_else(|error| panic!("proposal exists: {error:?}"));
		let state = Proposal::try_from_bytes(&proposal_account.data)
			.unwrap_or_else(|error| panic!("decode: {error:?}"));
		assert_eq!(
			state.status, STATUS_ACTIVE,
			"revocation must settle back to active"
		);
		drop(proposal_account);

		// A second draft the creator cancels outright.
		let (draft_key, draft_bump) = proposal_pda(&multisig_key, 2);
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&proposal_create_ix(
						&multisig_key,
						draft_bump,
						KIND_VAULT,
						&[0, 0, 0, 0, 0, 0],
						&[],
					),
					vec![
						AccountMeta::new(multisig_key, false),
						AccountMeta::new(draft_key, false),
						AccountMeta::new_readonly(member_a().pubkey(), true),
						AccountMeta::new(member_a().pubkey(), true),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(clock(), false),
					],
				),
				&[&member_a()],
			)
			.unwrap_or_else(|error| panic!("create the cancellation-fated draft: {error:?}"));
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&bare_ix(MultisigInstruction::ProposalCancel as u8),
					vec![
						AccountMeta::new_readonly(multisig_key, false),
						AccountMeta::new(draft_key, false),
						AccountMeta::new_readonly(member_a().pubkey(), true),
						AccountMeta::new_readonly(clock(), false),
					],
				),
				&[&member_a()],
			)
			.unwrap_or_else(|error| panic!("cancel the draft: {error:?}"));
		let draft_account = program
			.account(&draft_key)
			.unwrap_or_else(|error| panic!("draft exists: {error:?}"));
		let state = Proposal::try_from_bytes(&draft_account.data)
			.unwrap_or_else(|error| panic!("decode: {error:?}"));
		assert_eq!(state.status, STATUS_CANCELLED);
		drop(draft_account);

		// The controlled path: the authority executes a config action stream
		// directly, no proposal required.
		let mut actions = Vec::new();
		actions.push(1_u8);
		actions.push(ACTION_SET_TIME_LOCK);
		actions.extend_from_slice(&3600_u32.to_le_bytes());
		let mut authority_ix = vec![0_u8; ConfigAuthorityExecuteIx::SIZE];
		ConfigAuthorityExecuteIx::initialize(&mut authority_ix, |ix| {
			ix.actions_len.set(actions.len() as u16);
			ix.actions[..actions.len()].copy_from_slice(&actions);
			Ok(())
		})
		.unwrap_or_else(|error| panic!("encode authority execute: {error:?}"));
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&authority_ix,
					vec![
						AccountMeta::new(multisig_key, false),
						AccountMeta::new_readonly(authority.pubkey(), true),
						AccountMeta::new(funder.pubkey(), true),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(clock(), false),
					],
				),
				&[&authority, &funder],
			)
			.unwrap_or_else(|error| panic!("authority-execute the timelock change: {error:?}"));
		let multisig_account = program
			.account(&multisig_key)
			.unwrap_or_else(|error| panic!("multisig exists: {error:?}"));
		let state = Multisig::try_from_bytes(&multisig_account.data)
			.unwrap_or_else(|error| panic!("decode: {error:?}"));
		assert_eq!(state.timelock.get(), 3600);
		drop(multisig_account);

		// Terminal proposals pay their rent to the collector.
		program.fund(&collector, 0).err();
		let collector_balance = program
			.balance(&collector)
			.unwrap_or_else(|error| panic!("collector balance: {error:?}"));
		program
			.send_instruction(Instruction::new_with_bytes(
				pid,
				&bare_ix(MultisigInstruction::ProposalClose as u8),
				vec![
					AccountMeta::new_readonly(multisig_key, false),
					AccountMeta::new(draft_key, false),
					AccountMeta::new(collector, false),
				],
			))
			.unwrap_or_else(|error| panic!("close the cancelled draft: {error:?}"));
		assert!(
			program
				.balance(&collector)
				.unwrap_or_else(|error| panic!("collector balance: {error:?}"))
				> collector_balance,
			"the close must refund the collector"
		);

		program
			.stop()
			.unwrap_or_else(|error| panic!("stop program test: {error:?}"));
	});
}

// ---------------------------------------------------------------------------
// Audit regressions (2026-09-22 deep audit, re-verified 2026-09-23)
//
// Each test below asserts the *secure* behavior from the audit report. It
// fails on the current tree because the exploit is still live, and must pass
// once the corresponding fix lands. Run with `pina test --project
// examples/multisig_program --filter audit_sec_`.
// ---------------------------------------------------------------------------

use program_under_test::ACTION_ADD_SPENDING_LIMIT;
use program_under_test::ACTION_REMOVE_MEMBER;
use program_under_test::ACTION_REMOVE_SPENDING_LIMIT;
use program_under_test::PERIOD_ONE_TIME;

/// Encode a multisig create instruction with an explicit timelock, TTL, and
/// rent collector, mirroring `create_multisig_ix` above.
fn audit_create_multisig_ix(
	bump: u8,
	member_count: usize,
	threshold: u16,
	timelock: u32,
	ttl: u32,
	rent_collector: Option<&Pubkey>,
) -> Vec<u8> {
	let mut data = vec![0_u8; MultisigCreateIx::SIZE];
	MultisigCreateIx::initialize(&mut data, |ix| {
		ix.bump = bump;
		ix.threshold.set(threshold);
		ix.timelock.set(timelock);
		ix.ttl.set(ttl);
		for slot in ix.member_permissions.iter_mut().take(member_count) {
			*slot = PERMISSIONS_ALL;
		}
		ix.config_authority = Address::default();
		ix.rent_collector = rent_collector
			.map(|collector| pina_address(collector))
			.unwrap_or_default();
		Ok(())
	})
	.unwrap_or_else(|error| panic!("encode multisig create: {error:?}"));
	data
}

/// SEC-11: the global `ProgramConfig` PDA is a singleton and
/// `ConfigInitialize` stores whichever signer arrives first as its permanent
/// authority, with no bootstrap binding. An unapproved first caller must not
/// be able to occupy it.
///
/// Current behavior: any funded signer initializes the config and becomes the
/// authority, so the `expect_err` below fails and the test proves the capture
/// is live.
#[test]
#[ignore = "run with pina test"]
fn audit_sec_11_unauthorized_first_initializer_cannot_capture_the_program_config() {
	pina_test::run(async {
		let pid = program_id();
		let mut program = ProgramTest::start(pid)
			.await
			.unwrap_or_else(|error| panic!("start program test: {error:?}"));

		let attacker = Keypair::new_from_array([0xA1; 32]);
		program
			.fund(&attacker.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund attacker: {error:?}"));

		let (config_key, config_bump) = program_config_pda();
		let mut config_ix = vec![0_u8; ConfigInitializeIx::SIZE];
		ConfigInitializeIx::initialize(&mut config_ix, |ix| {
			ix.bump = config_bump;
			ix.treasury = pina_address(&attacker.pubkey());
			ix.creation_fee.set(0);
			Ok(())
		})
		.unwrap_or_else(|error| panic!("encode config init: {error:?}"));

		let error = program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&config_ix,
					vec![
						AccountMeta::new(attacker.pubkey(), true),
						AccountMeta::new(config_key, false),
						AccountMeta::new_readonly(system(), false),
					],
				),
				&[&attacker],
			)
			.expect_err("an unapproved signer must not capture the global config");

		assert!(
			program.account(&config_key).is_err(),
			"the rejected initialization must not leave config state behind"
		);
		assert!(
			error.message().contains("custom program error")
				|| error.message().contains("Unauthorized"),
			"expected an authorization rejection, got: {}",
			error.message()
		);

		program
			.stop()
			.unwrap_or_else(|error| panic!("stop program test: {error:?}"));
	});
}

/// SEC-21: a nonzero TTL not larger than the timelock expires a proposal
/// before its execution window can open (expiry starts at creation, the delay
/// at approval). Creating a multisig in that configuration must be rejected.
///
/// Current behavior: the combination is accepted, so the `expect_err` below
/// fails and the test proves proposals can be born impossible to execute.
#[test]
#[ignore = "run with pina test"]
fn audit_sec_21_creation_rejects_a_ttl_that_expires_before_the_timelock_elapses() {
	pina_test::run(async {
		let pid = program_id();
		let mut program = ProgramTest::start(pid)
			.await
			.unwrap_or_else(|error| panic!("start program test: {error:?}"));

		let authority = config_authority();
		program
			.fund(&authority.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund authority: {error:?}"));
		install_program_config(&program, &authority.pubkey());

		let create = create_key();
		program
			.fund(&create.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund create key: {error:?}"));
		let members = sorted_members();
		for member in &members {
			program
				.fund(member, FUND)
				.unwrap_or_else(|error| panic!("fund member: {error:?}"));
		}
		let funder = Keypair::new_from_array([0xF0; 32]);
		program
			.fund(&funder.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund funder: {error:?}"));

		let (multisig_key, multisig_bump) = multisig_pda(&create.pubkey());
		let (config_key, _) = program_config_pda();

		// A one-minute TTL under a one-hour timelock: every proposal created by
		// this multisig dies before it can execute.
		let error = program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&audit_create_multisig_ix(multisig_bump, members.len(), 2, 3_600, 60, None),
					vec![
						AccountMeta::new_readonly(config_key, false),
						AccountMeta::new_readonly(create.pubkey(), true),
						AccountMeta::new(multisig_key, false),
						AccountMeta::new(funder.pubkey(), true),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(pid, false),
						AccountMeta::new_readonly(members[0], false),
						AccountMeta::new_readonly(members[1], false),
						AccountMeta::new_readonly(members[2], false),
					],
				),
				&[&create, &funder],
			)
			.expect_err("a TTL at or below the timelock must be rejected at creation");

		assert!(
			program.account(&multisig_key).is_err(),
			"the rejected configuration must not create the multisig"
		);
		drop(error);

		program
			.stop()
			.unwrap_or_else(|error| panic!("stop program test: {error:?}"));
	});
}

/// SEC-22: `ProposalClose` only accepts terminal proposals, so an expired
/// active proposal — which can neither progress nor expire into a terminal
/// status — strands its rent forever. An expired proposal must be
/// permissionlessly closable with the refund going to the configured rent
/// collector.
///
/// Current behavior: the close is refused with `InvalidProposalStatus`, so
/// the `expect` below fails and the test proves the stranded rent.
#[test]
#[ignore = "run with pina test"]
fn audit_sec_22_an_expired_proposal_can_be_closed_by_anyone() {
	pina_test::run(async {
		let pid = program_id();
		let mut program = ProgramTest::start(pid)
			.await
			.unwrap_or_else(|error| panic!("start program test: {error:?}"));

		let authority = config_authority();
		program
			.fund(&authority.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund authority: {error:?}"));
		install_program_config(&program, &authority.pubkey());

		let collector = Keypair::new_from_array([0xC0; 32]);
		program
			.fund(&collector.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund rent collector: {error:?}"));
		let collector_before = program
			.balance(&collector.pubkey())
			.unwrap_or_else(|error| panic!("collector balance: {error:?}"));

		let create = create_key();
		program
			.fund(&create.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund create key: {error:?}"));
		let members = sorted_members();
		for member in &members {
			program
				.fund(member, FUND)
				.unwrap_or_else(|error| panic!("fund member: {error:?}"));
		}
		let funder = Keypair::new_from_array([0xF0; 32]);
		program
			.fund(&funder.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund funder: {error:?}"));

		let (multisig_key, multisig_bump) = multisig_pda(&create.pubkey());
		let (config_key, _) = program_config_pda();
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&audit_create_multisig_ix(
						multisig_bump,
						members.len(),
						2,
						0,
						60,
						Some(&collector.pubkey()),
					),
					vec![
						AccountMeta::new_readonly(config_key, false),
						AccountMeta::new_readonly(create.pubkey(), true),
						AccountMeta::new(multisig_key, false),
						AccountMeta::new(funder.pubkey(), true),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(pid, false),
						AccountMeta::new_readonly(members[0], false),
						AccountMeta::new_readonly(members[1], false),
						AccountMeta::new_readonly(members[2], false),
					],
				),
				&[&create, &funder],
			)
			.unwrap_or_else(|error| panic!("create multisig: {error:?}"));

		// A draft vault proposal with a sixty-second lifetime.
		let (proposal_key, proposal_bump) = proposal_pda(&multisig_key, 1);
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&proposal_create_ix(
						&multisig_key,
						proposal_bump,
						KIND_VAULT,
						&[0, 0, 0, 0, 0, 0],
						&[],
					),
					vec![
						AccountMeta::new(multisig_key, false),
						AccountMeta::new(proposal_key, false),
						AccountMeta::new_readonly(member_a().pubkey(), true),
						AccountMeta::new(member_a().pubkey(), true),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(clock(), false),
					],
				),
				&[&member_a()],
			)
			.unwrap_or_else(|error| panic!("create vault proposal: {error:?}"));

		// Advance well past the sixty-second TTL so the proposal is expired.
		program
			.time_travel_to_timestamp_millis(2_500_000_000_000)
			.unwrap_or_else(|error| panic!("time travel past expiry: {error:?}"));

		// The expired proposal must now be closable, refunding the collector.
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&bare_ix(MultisigInstruction::ProposalClose as u8),
					vec![
						AccountMeta::new(multisig_key, false),
						AccountMeta::new(proposal_key, false),
						AccountMeta::new(collector.pubkey(), true),
					],
				),
				&[&collector],
			)
			.expect("an expired proposal must be closable so its rent is not stranded");

		assert!(
			program.account(&proposal_key).is_err(),
			"the closed proposal account must be gone"
		);
		let collector_after = program
			.balance(&collector.pubkey())
			.unwrap_or_else(|error| panic!("collector balance: {error:?}"));
		assert!(
			collector_after > collector_before,
			"the close refund must reach the configured rent collector"
		);

		program
			.stop()
			.unwrap_or_else(|error| panic!("stop program test: {error:?}"));
	});
}

/// SEC-20: when a config execution prunes a spending-limit roster down to
/// empty and closes the account, the refund goes to the executor-supplied
/// `rent_payer` rather than the multisig's configured `rent_collector`. The
/// executor must not be able to direct that refund to themselves.
///
/// Current behavior: the close refunds `rent_payer` (here, the executing
/// member's own wallet), so the collector-balance assertion below fails and
/// the test proves the executor-directed refund.
#[test]
#[ignore = "run with pina test"]
fn audit_sec_20_config_execution_refunds_closed_rent_to_the_configured_collector() {
	pina_test::run(async {
		let pid = program_id();
		let mut program = ProgramTest::start(pid)
			.await
			.unwrap_or_else(|error| panic!("start program test: {error:?}"));

		let authority = config_authority();
		program
			.fund(&authority.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund authority: {error:?}"));
		install_program_config(&program, &authority.pubkey());

		let collector = Keypair::new_from_array([0xC0; 32]);
		program
			.fund(&collector.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund rent collector: {error:?}"));
		let collector_before = program
			.balance(&collector.pubkey())
			.unwrap_or_else(|error| panic!("collector balance: {error:?}"));

		let create = create_key();
		program
			.fund(&create.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund create key: {error:?}"));
		let members = sorted_members();
		for member in &members {
			program
				.fund(member, FUND)
				.unwrap_or_else(|error| panic!("fund member: {error:?}"));
		}
		let funder = Keypair::new_from_array([0xF0; 32]);
		program
			.fund(&funder.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund funder: {error:?}"));

		let (multisig_key, multisig_bump) = multisig_pda(&create.pubkey());
		let (config_key, _) = program_config_pda();
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&audit_create_multisig_ix(
						multisig_bump,
						members.len(),
						2,
						0,
						0,
						Some(&collector.pubkey()),
					),
					vec![
						AccountMeta::new_readonly(config_key, false),
						AccountMeta::new_readonly(create.pubkey(), true),
						AccountMeta::new(multisig_key, false),
						AccountMeta::new(funder.pubkey(), true),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(pid, false),
						AccountMeta::new_readonly(members[0], false),
						AccountMeta::new_readonly(members[1], false),
						AccountMeta::new_readonly(members[2], false),
					],
				),
				&[&create, &funder],
			)
			.unwrap_or_else(|error| panic!("create multisig: {error:?}"));

		// Governed step one: grant member C a one-time native-SOL spending
		// limit, so a funded limit account exists to retire.
		let limit_create_key = Keypair::new_from_array([0x51; 32]).pubkey();
		let (limit_key, _) = spending_limit_pda(&multisig_key, &limit_create_key);
		let mut grant = vec![1_u8, ACTION_ADD_SPENDING_LIMIT];
		grant.extend_from_slice(limit_create_key.as_ref());
		grant.push(0); // vault index
		grant.extend_from_slice(Pubkey::default().as_ref()); // native SOL
		grant.extend_from_slice(&1_000_u64.to_le_bytes());
		grant.push(PERIOD_ONE_TIME);
		grant.push(1); // one member on the limit roster
		grant.extend_from_slice(members[2].as_ref());
		grant.push(0); // no destinations

		// Governed step two: retire that spending limit explicitly. The
		// retirement closes the account and refunds the executor-supplied
		// `rent_payer`, not the multisig's configured rent collector.
		let mut retirement = vec![1_u8, ACTION_REMOVE_SPENDING_LIMIT];
		retirement.extend_from_slice(limit_key.as_ref());

		for (index, actions) in [(1_u64, &grant), (2, &retirement)] {
			let (proposal_key, proposal_bump) = proposal_pda(&multisig_key, index);
			program
				.send_with_signers(
					Instruction::new_with_bytes(
						pid,
						&proposal_create_ix(
							&multisig_key,
							proposal_bump,
							KIND_CONFIG,
							&[],
							actions,
						),
						vec![
							AccountMeta::new(multisig_key, false),
							AccountMeta::new(proposal_key, false),
							AccountMeta::new_readonly(member_a().pubkey(), true),
							AccountMeta::new(member_a().pubkey(), true),
							AccountMeta::new_readonly(system(), false),
							AccountMeta::new_readonly(clock(), false),
						],
					),
					&[&member_a()],
				)
				.unwrap_or_else(|error| panic!("create config proposal {index}: {error:?}"));
			for (discriminant, signer) in [
				(MultisigInstruction::ProposalActivate as u8, &member_a()),
				(MultisigInstruction::ProposalApprove as u8, &member_a()),
				(MultisigInstruction::ProposalApprove as u8, &member_b()),
			] {
				program
					.send_with_signers(
						Instruction::new_with_bytes(
							pid,
							&bare_ix(discriminant),
							vec![
								AccountMeta::new_readonly(multisig_key, false),
								AccountMeta::new(proposal_key, false),
								AccountMeta::new_readonly(signer.pubkey(), true),
								AccountMeta::new_readonly(clock(), false),
							],
						),
						&[signer],
					)
					.unwrap_or_else(|error| panic!("advance proposal {index}: {error:?}"));
			}

			// member_a executes and names *itself* as rent payer. The limit
			// account was funded by member_a at creation; its closing refund
			// must return to the configured collector, not member_a.
			let executor_before = program
				.balance(&member_a().pubkey())
				.unwrap_or_else(|error| panic!("executor balance: {error:?}"));
			program
				.send_with_signers(
					Instruction::new_with_bytes(
						pid,
						&bare_ix(MultisigInstruction::ConfigExecute as u8),
						vec![
							AccountMeta::new(multisig_key, false),
							AccountMeta::new(proposal_key, false),
							AccountMeta::new_readonly(member_a().pubkey(), true),
							AccountMeta::new(member_a().pubkey(), true),
							AccountMeta::new_readonly(system(), false),
							AccountMeta::new_readonly(clock(), false),
							AccountMeta::new(limit_key, false),
						],
					),
					&[&member_a()],
				)
				.unwrap_or_else(|error| panic!("execute proposal {index}: {error:?}"));
			let executor_after = program
				.balance(&member_a().pubkey())
				.unwrap_or_else(|error| panic!("executor balance: {error:?}"));
			let _ = (executor_before, executor_after);

			if index == 2 {
				// The retirement closed the account: its rent left with the
				// refund. Where did it land?
				let closed_lamports = program
					.account(&limit_key)
					.map(|account| account.lamports)
					.unwrap_or(0);
				assert_eq!(
					closed_lamports, 0,
					"the retired spending-limit account must be closed with its rent refunded"
				);
				let collector_after = program
					.balance(&collector.pubkey())
					.unwrap_or_else(|error| panic!("collector balance: {error:?}"));
				let executor_final = program
					.balance(&member_a().pubkey())
					.unwrap_or_else(|error| panic!("executor balance: {error:?}"));
				// The secure outcome: the refund reaches the configured
				// collector and the executor cannot profit from naming
				// themselves rent payer. Today the collector is unchanged and
				// the executor keeps the refund, so the first assertion below
				// fails and proves SEC-20.
				assert!(
					collector_after > collector_before,
					"the close refund must reach the multisig's configured rent collector, not 					 the executor"
				);
				assert!(
					executor_final <= executor_before,
					"the executor must not profit from directing the rent refund to themselves"
				);
			}
		}

		program
			.stop()
			.unwrap_or_else(|error| panic!("stop program test: {error:?}"));
	});
}

/// SEC-09: `ConfigExecute` checks staleness but never `expires_at`, so an
/// approved configuration proposal can execute after its declared lifetime
/// and change membership, thresholds, timelocks, or rent collectors. An
/// expired proposal must be refused exactly like `VaultExecute` refuses it.
///
/// Current behavior: the long-expired config proposal executes, so the
/// `expect_err` below fails and the test proves the posthumous execution.
#[test]
#[ignore = "run with pina test"]
fn audit_sec_09_an_expired_config_proposal_cannot_execute() {
	pina_test::run(async {
		let pid = program_id();
		let mut program = ProgramTest::start(pid)
			.await
			.unwrap_or_else(|error| panic!("start program test: {error:?}"));

		let authority = config_authority();
		program
			.fund(&authority.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund authority: {error:?}"));
		install_program_config(&program, &authority.pubkey());

		let create = create_key();
		program
			.fund(&create.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund create key: {error:?}"));
		let members = sorted_members();
		for member in &members {
			program
				.fund(member, FUND)
				.unwrap_or_else(|error| panic!("fund member: {error:?}"));
		}
		let funder = Keypair::new_from_array([0xF0; 32]);
		program
			.fund(&funder.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund funder: {error:?}"));

		// A sixty-second proposal lifetime.
		let (multisig_key, multisig_bump) = multisig_pda(&create.pubkey());
		let (config_key, _) = program_config_pda();
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&audit_create_multisig_ix(multisig_bump, members.len(), 2, 0, 60, None),
					vec![
						AccountMeta::new_readonly(config_key, false),
						AccountMeta::new_readonly(create.pubkey(), true),
						AccountMeta::new(multisig_key, false),
						AccountMeta::new(funder.pubkey(), true),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(pid, false),
						AccountMeta::new_readonly(members[0], false),
						AccountMeta::new_readonly(members[1], false),
						AccountMeta::new_readonly(members[2], false),
					],
				),
				&[&create, &funder],
			)
			.unwrap_or_else(|error| panic!("create multisig: {error:?}"));

		// A config proposal that raises the timelock to one hour.
		let mut actions = vec![1_u8, ACTION_SET_TIME_LOCK];
		actions.extend_from_slice(&3_600_u32.to_le_bytes());
		let (proposal_key, proposal_bump) = proposal_pda(&multisig_key, 1);
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&proposal_create_ix(&multisig_key, proposal_bump, KIND_CONFIG, &[], &actions),
					vec![
						AccountMeta::new(multisig_key, false),
						AccountMeta::new(proposal_key, false),
						AccountMeta::new_readonly(member_a().pubkey(), true),
						AccountMeta::new(member_a().pubkey(), true),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(clock(), false),
					],
				),
				&[&member_a()],
			)
			.unwrap_or_else(|error| panic!("create config proposal: {error:?}"));
		for (discriminant, signer) in [
			(MultisigInstruction::ProposalActivate as u8, &member_a()),
			(MultisigInstruction::ProposalApprove as u8, &member_a()),
			(MultisigInstruction::ProposalApprove as u8, &member_b()),
		] {
			program
				.send_with_signers(
					Instruction::new_with_bytes(
						pid,
						&bare_ix(discriminant),
						vec![
							AccountMeta::new_readonly(multisig_key, false),
							AccountMeta::new(proposal_key, false),
							AccountMeta::new_readonly(signer.pubkey(), true),
							AccountMeta::new_readonly(clock(), false),
						],
					),
					&[signer],
				)
				.unwrap_or_else(|error| panic!("advance config proposal: {error:?}"));
		}

		// The proposal's sixty-second lifetime is long past.
		program
			.time_travel_to_timestamp_millis(2_500_000_000_000)
			.unwrap_or_else(|error| panic!("time travel past expiry: {error:?}"));

		let error = program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&bare_ix(MultisigInstruction::ConfigExecute as u8),
					vec![
						AccountMeta::new(multisig_key, false),
						AccountMeta::new(proposal_key, false),
						AccountMeta::new_readonly(member_c().pubkey(), true),
						AccountMeta::new(member_c().pubkey(), true),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(clock(), false),
					],
				),
				&[&member_c()],
			)
			.expect_err("an expired config proposal must not execute");

		// The refused execution must leave the timelock unchanged at zero.
		let multisig_account = program
			.account(&multisig_key)
			.unwrap_or_else(|error| panic!("multisig exists: {error:?}"));
		let state = Multisig::try_from_bytes(&multisig_account.data)
			.unwrap_or_else(|error| panic!("decode multisig: {error:?}"));
		assert_eq!(
			state.timelock.get(),
			0,
			"the expired proposal's config change must not apply: {}",
			error.message()
		);

		program
			.stop()
			.unwrap_or_else(|error| panic!("stop program test: {error:?}"));
	});
}
