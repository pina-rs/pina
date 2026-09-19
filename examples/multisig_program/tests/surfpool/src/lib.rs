//! Surfpool end-to-end journeys for the multisig program.
//!
//! These tests run against the real runtime through `pina test`, which builds
//! the SBF artifact and runs this crate with `--ignored`. Every signer and
//! account uses fixed seeds so the recorded instruction paths stay
//! deterministic for the benchmark harness.

#![cfg(test)]

use pina_test::Account;
use pina_test::AccountMeta;
use pina_test::Instruction;
use pina_test::InstructionError;
use pina_test::Keypair;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use pina_test::Rent;
use pina_test::Signer;
use pina_test::TransactionConfig;
use pina_test::TransactionError;
use pina_test::TransactionFormat;
use program_under_test::ACTION_ADD_MEMBER;
use program_under_test::ACTION_SET_TIME_LOCK;
use program_under_test::Address;
use program_under_test::ConfigInitializeIx;
use program_under_test::KIND_CONFIG;
use program_under_test::KIND_VAULT;
use program_under_test::MAX_MESSAGE_BYTES;
use program_under_test::Multisig;
use program_under_test::MultisigAccountType;
use program_under_test::MultisigCreateIx;
use program_under_test::MultisigError;
use program_under_test::MultisigImportIx;
use program_under_test::MultisigInstruction;
use program_under_test::MultisigPatch;
use program_under_test::PERIOD_DAY;
use program_under_test::PERMISSIONS_ALL;
use program_under_test::ProgramConfig;
use program_under_test::Proposal;
use program_under_test::ProposalCreateIx;
use program_under_test::ProposalPatch;
use program_under_test::STATUS_ACTIVE;
use program_under_test::STATUS_APPROVED;
use program_under_test::STATUS_DRAFT;
use program_under_test::STATUS_EXECUTED;
use program_under_test::STATUS_REJECTED;
use program_under_test::SpendingLimit;
use program_under_test::SpendingLimitPatch;
use program_under_test::SpendingLimitUseIx;

const FUND: u64 = 2_000_000_000;
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
	.expect("encode message fixture");
	buffer[..length].to_vec()
}

fn system_transfer_data(lamports: u64) -> Vec<u8> {
	let mut data = Vec::new();
	data.extend_from_slice(&2_u32.to_le_bytes());
	data.extend_from_slice(&lamports.to_le_bytes());
	data
}

fn multisig_data(
	create_key: &Pubkey,
	members: &[Pubkey],
	threshold: u16,
	timelock: u32,
	bump: u8,
	transaction_index: u64,
) -> Vec<u8> {
	let mut keys = [Address::default(); 24];
	let mut permissions = [0_u8; 24];
	for (position, key) in members.iter().enumerate() {
		keys[position] = pina_address(key);
		permissions[position] = PERMISSIONS_ALL;
	}
	let space = Multisig::projected_bytes(members.len(), members.len()).unwrap();
	let mut data = vec![0_u8; space];
	Multisig::initialize(
		&mut data,
		&MultisigPatch::new()
			.bump(bump)
			.create_key(pina_address(create_key))
			.config_authority(Address::default())
			.rent_collector(None)
			.threshold(threshold)
			.timelock(timelock)
			.transaction_index(transaction_index)
			.stale_transaction_index(0)
			.replace_member_keys(&keys[..members.len()])
			.replace_member_permissions(&permissions[..members.len()]),
	)
	.expect("encode multisig fixture");
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
	let (_, vault_bump) = vault_pda(multisig, 0);
	let space = Proposal::projected_bytes(0, message.len(), actions.len()).unwrap();
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
			.approved_mask(0)
			.rejected_mask(0)
			.replace_message(message)
			.replace_actions(actions),
	)
	.expect("encode proposal fixture");
	data
}

fn create_multisig_ix(bump: u8, members: &[Pubkey], threshold: u16) -> Vec<u8> {
	let mut data = vec![0_u8; MultisigCreateIx::SIZE];
	MultisigCreateIx::initialize(&mut data, |ix| {
		ix.bump = bump;
		ix.threshold.set(threshold);
		ix.timelock.set(0);
		ix.member_count = members.len() as u8;
		for (position, member) in members.iter().enumerate() {
			ix.member_keys[position] = pina_address(member);
			ix.member_permissions[position] = PERMISSIONS_ALL;
		}
		ix.config_authority.set(None);
		ix.rent_collector.set(None);
		Ok(())
	})
	.expect("encode multisig create");
	data
}

fn proposal_create_ix(bump: u8, kind: u8, message: &[u8], actions: &[u8]) -> Vec<u8> {
	let mut data = vec![0_u8; ProposalCreateIx::SIZE];
	ProposalCreateIx::initialize(&mut data, |ix| {
		ix.bump = bump;
		ix.kind = kind;
		ix.vault_index = 0;
		ix.ephemeral_signers = 0;
		ix.message_len.set(message.len() as u16);
		ix.message[..message.len()].copy_from_slice(message);
		ix.actions_len.set(actions.len() as u16);
		ix.actions[..actions.len()].copy_from_slice(actions);
		Ok(())
	})
	.expect("encode proposal create");
	data
}

fn bare_ix(discriminant: u8) -> Vec<u8> {
	vec![discriminant, 0]
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
		.expect("install account fixture");
}

#[test]
#[ignore = "run with pina test"]
fn end_to_end_governed_sol_transfer() {
	pina_test::run(async {
		let pid = program_id();
		let mut program = ProgramTest::start(pid).await.expect("start program test");

		let config_authority = Keypair::new_from_array([0xCA; 32]);
		program
			.fund(&config_authority.pubkey(), FUND)
			.expect("fund authority");

		// Bootstrap the global config with a zero fee.
		let (config_key, config_bump) = program_config_pda();
		let mut config_ix = vec![0_u8; ConfigInitializeIx::SIZE];
		ConfigInitializeIx::initialize(&mut config_ix, |ix| {
			ix.bump = config_bump;
			ix.treasury = pina_address(&config_authority.pubkey());
			ix.creation_fee.set(0);
			Ok(())
		})
		.expect("encode config init");
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
			.expect("initialize program config");

		// Create a three-member, threshold-two multisig.
		let create = create_key();
		program
			.fund(&create.pubkey(), FUND)
			.expect("fund create key");
		let members = sorted_members();
		for member in &members {
			program.fund(member, FUND).expect("fund member");
		}
		let (multisig_key, multisig_bump) = multisig_pda(&create.pubkey());
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&create_multisig_ix(multisig_bump, &members, 2),
					vec![
						AccountMeta::new_readonly(config_key, false),
						AccountMeta::new(multisig_key, false),
						AccountMeta::new_readonly(create.pubkey(), true),
						AccountMeta::new(members[0], true),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(pid, false),
					],
				),
				&[&create, &member_a()],
			)
			.expect("create multisig");

		let multisig_account = program.account(&multisig_key).expect("multisig exists");
		let state = Multisig::try_from_bytes(&multisig_account.data).expect("decode");
		assert_eq!(state.member_keys().len(), 3);
		assert_eq!(state.threshold.get(), 2);
		drop(multisig_account);

		// Fund the vault PDA and propose a SOL transfer out of it.
		let (vault_key, _) = vault_pda(&multisig_key, 0);
		program.fund(&vault_key, VAULT_FUND).expect("fund vault");
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
					&proposal_create_ix(proposal_bump, KIND_VAULT, &message, &[]),
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
			.expect("create vault proposal");
		let proposal_account = program.account(&proposal_key).expect("proposal exists");
		let state = Proposal::try_from_bytes(&proposal_account.data).expect("decode");
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
				.expect("advance proposal");
		}
		let proposal_account = program.account(&proposal_key).expect("proposal exists");
		let state = Proposal::try_from_bytes(&proposal_account.data).expect("decode");
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
			.expect("execute vault proposal");

		assert_eq!(program.balance(&payee).expect("payee balance"), TRANSFER);
		assert_eq!(
			program.balance(&vault_key).expect("vault balance"),
			VAULT_FUND - TRANSFER
		);
		let proposal_account = program.account(&proposal_key).expect("proposal exists");
		let state = Proposal::try_from_bytes(&proposal_account.data).expect("decode");
		assert_eq!(state.status, STATUS_EXECUTED);

		program.stop().expect("stop program test");
	});
}

#[test]
#[ignore = "run with pina test"]
fn governed_config_change_grows_the_roster_and_invalidates_prior_proposals() {
	pina_test::run(async {
		let pid = program_id();
		let mut program = ProgramTest::start(pid).await.expect("start program test");

		let create = create_key();
		program
			.fund(&create.pubkey(), FUND)
			.expect("fund create key");
		let members = sorted_members();
		for member in &members {
			program.fund(member, FUND).expect("fund member");
		}
		let (multisig_key, multisig_bump) = multisig_pda(&create.pubkey());
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&create_multisig_ix(multisig_bump, &members, 2),
					vec![
						AccountMeta::new_readonly(program_config_pda().0, false),
						AccountMeta::new(multisig_key, false),
						AccountMeta::new_readonly(create.pubkey(), true),
						AccountMeta::new(member_a().pubkey(), true),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(pid, false),
					],
				),
				&[&create, &member_a()],
			)
			.expect("create multisig");

		// A real draft vault proposal at index 1 that must go stale after the
		// config change.
		let (stale_proposal_key, stale_bump) = proposal_pda(&multisig_key, 1);
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&proposal_create_ix(stale_bump, KIND_VAULT, &[0, 0, 0, 0, 0, 1], &[]),
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
			.expect("create the stale-fated proposal");

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
					&proposal_create_ix(proposal_bump, KIND_CONFIG, &[], &actions),
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
			.expect("create config proposal");
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
				.expect("advance config proposal");
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
			.expect("execute config proposal");

		let multisig_account = program.account(&multisig_key).expect("multisig exists");
		let state = Multisig::try_from_bytes(&multisig_account.data).expect("decode");
		assert_eq!(state.member_keys().len(), 4);
		assert!(state.member_keys().contains(&pina_address(&new_member)));
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

		program.stop().expect("stop program test");
	});
}

#[test]
#[ignore = "run with pina test"]
fn rejection_cutoff_settles_and_events_are_emitted() {
	pina_test::run(async {
		let pid = program_id();
		let mut program = ProgramTest::start(pid).await.expect("start program test");

		let create = create_key();
		program
			.fund(&create.pubkey(), FUND)
			.expect("fund create key");
		let members = sorted_members();
		for member in &members {
			program.fund(member, FUND).expect("fund member");
		}
		let (multisig_key, multisig_bump) = multisig_pda(&create.pubkey());
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&create_multisig_ix(multisig_bump, &members, 2),
					vec![
						AccountMeta::new_readonly(program_config_pda().0, false),
						AccountMeta::new(multisig_key, false),
						AccountMeta::new_readonly(create.pubkey(), true),
						AccountMeta::new(member_a().pubkey(), true),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(pid, false),
					],
				),
				&[&create, &member_a()],
			)
			.expect("create multisig");

		let (proposal_key, proposal_bump) = proposal_pda(&multisig_key, 1);
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&proposal_create_ix(proposal_bump, KIND_VAULT, &[0, 0, 0, 0, 0, 1], &[]),
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
			.expect("create rejection-fated proposal");
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
			.expect("activate rejection-fated proposal");

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
				.expect("reject proposal");
		}
		let proposal_account = program.account(&proposal_key).expect("proposal exists");
		let state = Proposal::try_from_bytes(&proposal_account.data).expect("decode");
		assert_eq!(state.status, STATUS_REJECTED);
		assert_eq!(state.rejected_mask.get(), 0b011);
		drop(proposal_account);

		// The rejection emitted a decodable status event.
		let logs = program.simulate_logs(
			&bare_ix(MultisigInstruction::ProposalReject as u8),
			vec![
				AccountMeta::new_readonly(multisig_key, false),
				AccountMeta::new(proposal_key, false),
				AccountMeta::new_readonly(members[2], true),
				AccountMeta::new_readonly(clock(), false),
			],
		);
		assert!(
			logs.iter()
				.any(|log| log.iter().any(|line| line.contains("Program data: "))),
			"expected a ProposalStatus event record, got: {logs:?}"
		);

		program.stop().expect("stop program test");
	});
}

#[test]
#[ignore = "run with pina test"]
fn imports_reject_a_legacy_account_not_owned_by_squads() {
	pina_test::run(async {
		let pid = program_id();
		let mut program = ProgramTest::start(pid).await.expect("start program test");

		let create = create_key();
		program
			.fund(&create.pubkey(), FUND)
			.expect("fund create key");
		let payer = member_a();
		program.fund(&payer.pubkey(), FUND).expect("fund payer");
		let members = sorted_members();

		// Squads-shaped bytes, but owned by this program instead of Squads:
		// the state cheatcode cannot install foreign-owned accounts, which is
		// exactly the precondition the on-chain owner check must reject. The
		// happy-path import runs in the Mollusk suite, which can install a
		// Squads-owned fixture.
		let legacy_key = Pubkey::new_from_array([0x1E; 32]);
		let mut legacy = Vec::new();
		legacy.extend_from_slice(&program_under_test::SQUADS_MULTISIG_DISCRIMINATOR);
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
			ix.config_authority.set(None);
			ix.rent_collector.set(None);
			Ok(())
		})
		.expect("encode import");

		let error = program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&import_ix,
					vec![
						AccountMeta::new_readonly(legacy_key, false),
						AccountMeta::new_readonly(program_config_pda().0, false),
						AccountMeta::new(multisig_key, false),
						AccountMeta::new_readonly(create.pubkey(), true),
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

		program.stop().expect("stop program test");
	});
}

#[test]
#[ignore = "run with pina test"]
fn spending_limit_moves_sol_without_a_vote() {
	pina_test::run(async {
		let pid = program_id();
		let mut program = ProgramTest::start(pid).await.expect("start program test");

		let create = create_key();
		let members = sorted_members();
		for member in &members {
			program.fund(member, FUND).expect("fund member");
		}
		let (multisig_key, multisig_bump) = multisig_pda(&create.pubkey());
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&create_multisig_ix(multisig_bump, &members, 2),
					vec![
						AccountMeta::new_readonly(program_config_pda().0, false),
						AccountMeta::new(multisig_key, false),
						AccountMeta::new_readonly(create.pubkey(), true),
						AccountMeta::new(member_a().pubkey(), true),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(pid, false),
					],
				),
				&[&create, &member_a()],
			)
			.expect("create multisig");

		let (vault_key, _) = vault_pda(&multisig_key, 0);
		let limit_create_key = Pubkey::new_from_array([0x51; 32]);
		let (limit_key, limit_bump) = spending_limit_pda(&multisig_key, &limit_create_key);
		let payee = destination();
		// The payee must already be rent-exempt to receive lamports.
		program.fund(&payee, FUND).expect("fund payee");
		program.fund(&vault_key, VAULT_FUND).expect("fund vault");

		let rent = Rent::default();

		let mut member_addresses = [Address::default(); 24];
		for (position, member) in members.iter().enumerate() {
			member_addresses[position] = pina_address(member);
		}
		let space = SpendingLimit::projected_bytes(3, 1).unwrap();
		let mut limit_bytes = vec![0_u8; space];
		SpendingLimit::initialize(
			&mut limit_bytes,
			&SpendingLimitPatch::new()
				.bump(limit_bump)
				.multisig(pina_address(&multisig_key))
				.create_key(pina_address(&limit_create_key))
				.vault_index(0)
				.mint(Address::default())
				.amount(1000)
				.remaining_amount(1000)
				.last_reset(0)
				.period(PERIOD_DAY)
				.replace_members(&member_addresses[..3])
				.replace_destinations(&[pina_address(&payee)][..1]),
		)
		.expect("encode spending limit");
		let limit_rent = rent.minimum_balance(limit_bytes.len());
		install_account(&program, &limit_key, &pid, limit_bytes, limit_rent);

		let mut spend_ix = vec![0_u8; SpendingLimitUseIx::SIZE];
		SpendingLimitUseIx::initialize(&mut spend_ix, |ix| {
			ix.amount.set(400);
			ix.decimals = 9;
			Ok(())
		})
		.expect("encode spending limit use");
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
			.expect("spend against the limit");

		assert_eq!(program.balance(&payee).expect("payee balance"), 400);
		let limit_account = program.account(&limit_key).expect("limit exists");
		let state = SpendingLimit::try_from_bytes(&limit_account.data).expect("decode");
		assert_eq!(state.remaining_amount.get(), 600);

		// A draw past the allowance is rejected.
		let mut overdraw = vec![0_u8; SpendingLimitUseIx::SIZE];
		SpendingLimitUseIx::initialize(&mut overdraw, |ix| {
			ix.amount.set(601);
			ix.decimals = 9;
			Ok(())
		})
		.expect("encode overdraw");
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

		program.stop().expect("stop program test");
	});
}
