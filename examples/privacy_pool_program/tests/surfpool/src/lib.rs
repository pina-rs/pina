//! Surfpool end-to-end journeys for the privacy pool program.
//!
//! These tests run against the real runtime through `pina test`, which
//! builds the SBF artifact and runs this crate with `--ignored`. Every
//! signer and account uses fixed seeds so the recorded instruction paths
//! stay deterministic for the benchmark harness.
//!
//! The journeys cover the protocol's spine on a real runtime: pool setup
//! with both verifying keys, a deposit whose on-chain root matches the
//! host prediction, a real Groth16 withdrawal, and the tier-0 disclosure
//! lifecycle through consent, custodian approvals, and the atomic log.

#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::Instruction;
use pina_test::Keypair;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use pina_test::Signer;
use program_under_test::Address;
use program_under_test::ApproveDisclosureIx;
use program_under_test::CancelDisclosureIx;
use program_under_test::ChallengeDisclosureIx;
use program_under_test::DEPOSIT_LAMPORTS;
use program_under_test::DepositIx;
use program_under_test::GrantDisclosureIx;
use program_under_test::InitializeIx;
use program_under_test::RegisterRequesterIx;
use program_under_test::RequestDisclosureIx;
use program_under_test::ResolveChallengeIx;
use program_under_test::SetCustodiansIx;
use program_under_test::SetVerificationKeyIx;
use program_under_test::TREE_NODES;
use program_under_test::TransferIx;
use program_under_test::VK_SLOT_TRANSFER;
use program_under_test::VK_SLOT_WITHDRAW;
use program_under_test::WithdrawIx;
use program_under_test::prover;

const SYSTEM_BYTES: [u8; 32] = [0; 32];

/// The tree account stores 8,160 node bytes after the envelope, bump, and
/// next-leaf counter.
const TREE_NODES_AT: usize = 2 + 1 + 8;

fn system() -> Pubkey {
	Pubkey::new_from_array(SYSTEM_BYTES)
}

fn program_id() -> Pubkey {
	let bytes: &[u8] = program_under_test::ID.as_ref();
	Pubkey::new_from_array(bytes.try_into().unwrap())
}

fn pina_address(pubkey: &Pubkey) -> Address {
	Address::new_from_array(pubkey.to_bytes())
}

fn authority() -> Keypair {
	Keypair::new_from_array([0xCA; 32])
}

fn custodian_a() -> Keypair {
	Keypair::new_from_array([0xA1; 32])
}

fn custodian_b() -> Keypair {
	Keypair::new_from_array([0xB2; 32])
}

fn custodian_c() -> Keypair {
	Keypair::new_from_array([0xC3; 32])
}

fn depositor() -> Keypair {
	Keypair::new_from_array([0x5E; 32])
}

fn recipient() -> Pubkey {
	Pubkey::new_from_array([0xDE; 32])
}

fn requester() -> Keypair {
	Keypair::new_from_array([0x7A; 32])
}

fn view_key() -> Keypair {
	Keypair::new_from_array([0x5A; 32])
}

fn config_pda() -> (Pubkey, u8) {
	Pubkey::find_program_address(&[b"privacy-pool-config"], &program_id())
}

fn vault_pda() -> (Pubkey, u8) {
	Pubkey::find_program_address(&[b"privacy-pool-vault"], &program_id())
}

fn tree_pda() -> (Pubkey, u8) {
	Pubkey::find_program_address(&[b"privacy-pool-tree"], &program_id())
}

fn nullifiers_pda() -> (Pubkey, u8) {
	Pubkey::find_program_address(&[b"privacy-pool-nullifiers"], &program_id())
}

fn custodians_pda() -> (Pubkey, u8) {
	Pubkey::find_program_address(&[b"privacy-pool-custodians"], &program_id())
}

fn requesters_pda() -> (Pubkey, u8) {
	Pubkey::find_program_address(&[b"privacy-pool-requesters"], &program_id())
}

fn log_pda() -> (Pubkey, u8) {
	Pubkey::find_program_address(&[b"privacy-pool-log"], &program_id())
}

fn note_pda(commitment: &[u8; 32]) -> (Pubkey, u8) {
	Pubkey::find_program_address(&[b"privacy-pool-note", commitment], &program_id())
}

fn request_pda(requester: &Pubkey, nonce: u64) -> (Pubkey, u8) {
	Pubkey::find_program_address(
		&[
			b"privacy-pool-request",
			requester.as_ref(),
			&nonce.to_le_bytes(),
		],
		&program_id(),
	)
}

fn vkey_pda(slot: u8) -> (Pubkey, u8) {
	let seed = u64::from(slot).to_le_bytes();
	Pubkey::find_program_address(&[b"privacy-pool-vkey", &seed], &program_id())
}

/// Host-side commitment: `poseidon(poseidon(secret, seed), amount)`.
fn commitment_bytes(secrets: &prover::NoteSecrets) -> [u8; 32] {
	let inner = program_under_test::poseidon2(
		&prover::fr_to_le(&secrets.secret),
		&prover::fr_to_le(&secrets.nullifier_seed),
	)
	.unwrap_or_else(|error| panic!("host commitment: {error:?}"));
	let mut amount = [0_u8; 32];
	amount[..8].copy_from_slice(&DEPOSIT_LAMPORTS.to_le_bytes());
	program_under_test::poseidon2(&inner, &amount)
		.unwrap_or_else(|error| panic!("host commitment: {error:?}"))
}

/// Host-side nullifier: `poseidon(seed, secret)`.
fn nullifier_bytes(secrets: &prover::NoteSecrets) -> [u8; 32] {
	program_under_test::poseidon2(
		&prover::fr_to_le(&secrets.nullifier_seed),
		&prover::fr_to_le(&secrets.secret),
	)
	.unwrap_or_else(|error| panic!("host nullifier: {error:?}"))
}

/// Journeys run serially: each embeds its own Surfnet and spends tens of
/// seconds in seeded Groth16 setup; parallel instances starve the runtime
/// runloops until RPC polls time out.
static JOURNEY_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn journey_guard() -> std::sync::MutexGuard<'static, ()> {
	JOURNEY_LOCK
		.lock()
		.unwrap_or_else(|poisoned| poisoned.into_inner())
}

async fn start_pool() -> (ProgramTest, prover::NoteSecrets, [u8; 32], u64) {
	let pid = program_id();
	let mut program = ProgramTest::start(pid)
		.await
		.unwrap_or_else(|error| panic!("start program test: {error:?}"));

	let auth = authority();
	program
		.fund(&auth.pubkey(), 3_000_000_000)
		.unwrap_or_else(|error| panic!("fund authority: {error:?}"));

	// Initialize the pool with the fixed custodian committee.
	let (config_key, config_bump) = config_pda();
	let (vault_key, vault_bump) = vault_pda();
	let (tree_key, tree_bump) = tree_pda();
	let (nullifiers_key, nullifiers_bump) = nullifiers_pda();
	let (custodians_key, custodians_bump) = custodians_pda();
	let (requesters_key, requesters_bump) = requesters_pda();
	let (log_key, log_bump) = log_pda();

	let mut init = vec![0_u8; InitializeIx::SIZE];
	InitializeIx::initialize(&mut init, |ix| {
		ix.config_bump = config_bump;
		ix.vault_bump = vault_bump;
		ix.tree_bump = tree_bump;
		ix.nullifiers_bump = nullifiers_bump;
		ix.custodians_bump = custodians_bump;
		ix.requesters_bump = requesters_bump;
		ix.log_bump = log_bump;
		let mut committee = [0_u8; 96];
		for (slot, key) in [custodian_a(), custodian_b(), custodian_c()]
			.iter()
			.enumerate()
		{
			committee[slot * 32..(slot + 1) * 32].copy_from_slice(key.pubkey().as_ref());
		}
		ix.custodians = committee;
		Ok(())
	})
	.unwrap_or_else(|error| panic!("encode initialize: {error:?}"));

	program
		.send_with_signers(
			Instruction::new_with_bytes(
				pid,
				&init,
				vec![
					AccountMeta::new(auth.pubkey(), true),
					AccountMeta::new(config_key, false),
					AccountMeta::new(vault_key, false),
					AccountMeta::new(tree_key, false),
					AccountMeta::new(nullifiers_key, false),
					AccountMeta::new(custodians_key, false),
					AccountMeta::new(requesters_key, false),
					AccountMeta::new(log_key, false),
					AccountMeta::new_readonly(system(), false),
				],
			),
			&[&auth],
		)
		.unwrap_or_else(|error| panic!("initialize pool: {error:?}"));

	// Install both verifying keys from the seeded setups.
	let (_withdraw_pk, withdraw_vk) = prover::seeded_setup(false, 0x1111);
	let (_transfer_pk, transfer_vk) = prover::seeded_setup(true, 0x2222);
	for (slot, wire) in [
		(VK_SLOT_WITHDRAW, prover::serialize_vk(&withdraw_vk)),
		(VK_SLOT_TRANSFER, prover::serialize_vk(&transfer_vk)),
	] {
		let (vkey_key, vkey_bump) = vkey_pda(slot);
		let mut data = vec![0_u8; SetVerificationKeyIx::SIZE];
		SetVerificationKeyIx::initialize(&mut data, |ix| {
			ix.bump = vkey_bump;
			ix.slot = slot;
			ix.ic_len = wire.ic_len;
			ix.alpha_g1 = wire.alpha_g1;
			ix.beta_g2 = wire.beta_g2;
			ix.gamma_g2 = wire.gamma_g2;
			ix.delta_g2 = wire.delta_g2;
			ix.ic0 = wire.ic[0];
			ix.ic1 = wire.ic[1];
			ix.ic2 = wire.ic[2];
			ix.ic3 = wire.ic[3];
			Ok(())
		})
		.unwrap_or_else(|error| panic!("encode vkey: {error:?}"));
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&data,
					vec![
						AccountMeta::new(auth.pubkey(), true),
						AccountMeta::new_readonly(config_key, false),
						AccountMeta::new(vkey_key, false),
						AccountMeta::new_readonly(system(), false),
					],
				),
				&[&auth],
			)
			.unwrap_or_else(|error| panic!("install vkey {slot}: {error:?}"));
	}

	// Deposit one note with the fixed view key.
	let dep = depositor();
	program
		.fund(&dep.pubkey(), 2_000_000_000)
		.unwrap_or_else(|error| panic!("fund depositor: {error:?}"));
	let secrets = prover::note_secrets(0x88);
	let commitment = commitment_bytes(&secrets);
	let (note_key, note_bump) = note_pda(&commitment);
	let mut deposit = vec![0_u8; DepositIx::SIZE];
	DepositIx::initialize(&mut deposit, |ix| {
		ix.bump = note_bump;
		ix.commitment = commitment;
		ix.view_pubkey = view_key().pubkey().to_bytes();
		ix.envelope_len = 128;
		ix.envelope = core::array::from_fn(|index| index as u8);
		ix.shares = core::array::from_fn(|index| 0xA0 ^ index as u8);
		Ok(())
	})
	.unwrap_or_else(|error| panic!("encode deposit: {error:?}"));
	program
		.send_with_signers(
			Instruction::new_with_bytes(
				pid,
				&deposit,
				vec![
					AccountMeta::new(dep.pubkey(), true),
					AccountMeta::new(config_key, false),
					AccountMeta::new(vault_key, false),
					AccountMeta::new(tree_key, false),
					AccountMeta::new(note_key, false),
					AccountMeta::new_readonly(system(), false),
				],
			),
			&[&dep],
		)
		.unwrap_or_else(|error| panic!("deposit: {error:?}"));

	(program, secrets, commitment, 0)
}

#[test]
#[ignore = "run with pina test"]
fn deposit_root_matches_the_host_prediction() {
	let _guard = journey_guard();
	pina_test::run(async {
		let (program, secrets, commitment, _) = start_pool().await;

		// The stored leaf equals the host commitment, and the on-chain
		// root (computed through the Poseidon syscall) equals the host
		// fold of that leaf up the zero-hash chain.
		let tree = tree_pda().0;
		let account = program
			.account(&tree)
			.unwrap_or_else(|error| panic!("fetch tree: {error:?}"));
		let nodes = &account.data[TREE_NODES_AT..TREE_NODES_AT + TREE_NODES * 32];
		assert_eq!(&nodes[..32], &commitment[..]);

		let zeros = program_under_test::zero_hashes()
			.unwrap_or_else(|error| panic!("zero hashes: {error:?}"));
		let mut expected = program_under_test::poseidon2(&commitment, &zeros[0])
			.unwrap_or_else(|error| panic!("host fold: {error:?}"));
		for level in 2..=program_under_test::TREE_DEPTH {
			expected = program_under_test::poseidon2(&expected, &zeros[level - 1])
				.unwrap_or_else(|error| panic!("host fold: {error:?}"));
		}
		assert_eq!(&nodes[(TREE_NODES - 1) * 32..], &expected[..]);

		// The nullifier derivation stays private but reproducible.
		let nullifier = nullifier_bytes(&secrets);
		assert_ne!(nullifier, commitment);
	});
}

#[test]
#[ignore = "run with pina test"]
fn withdraw_pays_out_through_a_real_proof() {
	let _guard = journey_guard();
	pina_test::run(async {
		let (program, secrets, ..) = start_pool().await;
		let pid = program_id();

		let tree = tree_pda().0;
		let account = program
			.account(&tree)
			.unwrap_or_else(|error| panic!("fetch tree: {error:?}"));
		let nodes = account.data[TREE_NODES_AT..TREE_NODES_AT + TREE_NODES * 32].to_vec();
		let mut root = [0_u8; 32];
		root.copy_from_slice(&nodes[(TREE_NODES - 1) * 32..]);

		let nullifier = nullifier_bytes(&secrets);
		let (path_elements, path_indices) = prover::build_witness(&nodes, 0);
		let (pk, vk) = prover::seeded_setup(false, 0x1111);
		let circuit = prover::SpendCircuit {
			root: prover::fr_from_le(&root).unwrap(),
			nullifier: prover::fr_from_le(&nullifier).unwrap(),
			output_commitment: None,
			amount: prover::amount_field(),
			witness: Some(prover::SpendWitness {
				spent: secrets,
				path_elements,
				path_indices,
				successor: None,
			}),
		};
		let proof = prover::prove_spend(&pk, circuit, 0x99);
		assert!(prover::verify_host(
			&vk,
			&[
				prover::fr_from_le(&root).unwrap(),
				prover::fr_from_le(&nullifier).unwrap()
			],
			&proof,
		));

		// The recorded instruction path is deterministic; the withdrawal
		// pays the denomination to the fixed recipient.
		let before = program.balance(&recipient()).unwrap_or(0);

		let wire = prover::serialize_proof(&proof);
		let mut data = vec![0_u8; WithdrawIx::SIZE];
		WithdrawIx::initialize(&mut data, |ix| {
			ix.nullifier = nullifier;
			ix.root = root;
			ix.proof_a = wire.a;
			ix.proof_b = wire.b;
			ix.proof_c = wire.c;
			Ok(())
		})
		.unwrap_or_else(|error| panic!("encode withdraw: {error:?}"));

		program
			.send_instruction(Instruction::new_with_bytes(
				pid,
				&data,
				vec![
					AccountMeta::new_readonly(config_pda().0, false),
					AccountMeta::new(vault_pda().0, false),
					AccountMeta::new_readonly(tree, false),
					AccountMeta::new(nullifiers_pda().0, false),
					AccountMeta::new_readonly(vkey_pda(VK_SLOT_WITHDRAW).0, false),
					AccountMeta::new(recipient(), false),
					AccountMeta::new_readonly(system(), false),
				],
			))
			.unwrap_or_else(|error| panic!("withdraw: {error:?}"));

		let after = program
			.balance(&recipient())
			.unwrap_or_else(|error| panic!("fetch recipient: {error:?}"));
		assert_eq!(after, before + DEPOSIT_LAMPORTS);
	});
}

#[test]
#[ignore = "run with pina test"]
fn tier_zero_disclosure_requires_consent_and_logs_execution() {
	let _guard = journey_guard();
	pina_test::run(async {
		let (program, _secrets, commitment, _) = start_pool().await;
		let pid = program_id();

		let req = requester();
		program
			.fund(&req.pubkey(), 1_000_000_000)
			.unwrap_or_else(|error| panic!("fund requester: {error:?}"));

		// Anyone may file at tier 0.
		let (request_key, request_bump) = request_pda(&req.pubkey(), 1);
		let mut request_data = vec![0_u8; RequestDisclosureIx::SIZE];
		RequestDisclosureIx::initialize(&mut request_data, |ix| {
			ix.bump = request_bump;
			ix.nonce.set(1);
			ix.tier = program_under_test::TIER_CONSENT;
			ix.commitment = commitment;
			ix.notice_len = 96;
			ix.notice = core::array::from_fn(|index| 0xC0 ^ index as u8);
			ix.legal_basis_hash = [0; 32];
			Ok(())
		})
		.unwrap_or_else(|error| panic!("encode request: {error:?}"));
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&request_data,
					vec![
						AccountMeta::new(req.pubkey(), true),
						AccountMeta::new_readonly(config_pda().0, false),
						AccountMeta::new_readonly(requesters_pda().0, false),
						AccountMeta::new_readonly(note_pda(&commitment).0, false),
						AccountMeta::new(request_key, false),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(clock(), false),
					],
				),
				&[&req],
			)
			.unwrap_or_else(|error| panic!("request disclosure: {error:?}"));

		// The note's view key grants consent.
		let viewer = view_key();
		let mut grant = vec![0_u8; GrantDisclosureIx::SIZE];
		GrantDisclosureIx::initialize(&mut grant, |_| Ok(()))
			.unwrap_or_else(|error| panic!("encode grant: {error:?}"));
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&grant,
					vec![
						AccountMeta::new(request_key, false),
						AccountMeta::new_readonly(note_pda(&commitment).0, false),
						AccountMeta::new_readonly(viewer.pubkey(), true),
					],
				),
				&[&viewer],
			)
			.unwrap_or_else(|error| panic!("grant consent: {error:?}"));

		// Two custodian approvals execute the request and append the log
		// entry in the same instruction.
		for custodian in [custodian_a(), custodian_b()] {
			let mut approval = vec![0_u8; program_under_test::ApproveDisclosureIx::SIZE];
			program_under_test::ApproveDisclosureIx::initialize(&mut approval, |_| Ok(()))
				.unwrap_or_else(|error| panic!("encode approval: {error:?}"));
			program
				.send_with_signers(
					Instruction::new_with_bytes(
						pid,
						&approval,
						vec![
							AccountMeta::new_readonly(custodian.pubkey(), true),
							AccountMeta::new_readonly(config_pda().0, false),
							AccountMeta::new_readonly(custodians_pda().0, false),
							AccountMeta::new(request_key, false),
							AccountMeta::new(log_pda().0, false),
							AccountMeta::new_readonly(clock(), false),
						],
					),
					&[&custodian],
				)
				.unwrap_or_else(|error| panic!("approve disclosure: {error:?}"));
		}

		// The log holds exactly one entry naming the requester, the
		// commitment, and the tier.
		let log = log_pda().0;
		let account = program
			.account(&log)
			.unwrap_or_else(|error| panic!("fetch log: {error:?}"));
		let count = u64::from_le_bytes(
			account.data[3..11]
				.try_into()
				.unwrap_or_else(|_| panic!("count")),
		);
		assert_eq!(count, 1);
		assert_eq!(&account.data[11..43], req.pubkey().as_ref());
		assert_eq!(&account.data[43..75], &commitment[..]);
		assert_eq!(account.data[75], program_under_test::TIER_CONSENT);
	});
}

/// Exercise every remaining instruction so the benchmark harness sees a
/// sample for each IDL discriminator: `transfer`, `setCustodians`,
/// `registerRequester`, `challengeDisclosure`, `resolveChallenge`, and
/// `cancelDisclosure`.
#[test]
#[ignore = "run with pina test"]
fn every_instruction_discriminator_is_exercised() {
	let _guard = journey_guard();
	pina_test::run(async {
		let (program, secrets, commitment, _) = start_pool().await;
		let pid = program_id();

		// registerRequester (3): the authority registers a tier-1 entity.
		let requester = requester();
		program
			.fund(&requester.pubkey(), 1_000_000_000)
			.unwrap_or_else(|error| panic!("fund requester: {error:?}"));
		let mut register = vec![0_u8; RegisterRequesterIx::SIZE];
		RegisterRequesterIx::initialize(&mut register, |ix| {
			ix.requester = pina_address(&requester.pubkey());
			ix.max_tier = program_under_test::TIER_COMPELLED;
			Ok(())
		})
		.unwrap_or_else(|error| panic!("encode register: {error:?}"));
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&register,
					vec![
						AccountMeta::new_readonly(authority().pubkey(), true),
						AccountMeta::new_readonly(config_pda().0, false),
						AccountMeta::new(requesters_pda().0, false),
					],
				),
				&[&authority()],
			)
			.unwrap_or_else(|error| panic!("register requester: {error:?}"));

		// setCustodians (2): rotate the committee to three fresh keys.
		let replacement = [
			Keypair::new_from_array([0xD1; 32]),
			Keypair::new_from_array([0xD2; 32]),
			Keypair::new_from_array([0xD3; 32]),
		];
		let mut rotate = vec![0_u8; SetCustodiansIx::SIZE];
		SetCustodiansIx::initialize(&mut rotate, |ix| {
			let mut committee = [0_u8; 96];
			for (slot, key) in replacement.iter().enumerate() {
				committee[slot * 32..(slot + 1) * 32].copy_from_slice(key.pubkey().as_ref());
			}
			ix.custodians = committee;
			Ok(())
		})
		.unwrap_or_else(|error| panic!("encode set custodians: {error:?}"));
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&rotate,
					vec![
						AccountMeta::new_readonly(authority().pubkey(), true),
						AccountMeta::new_readonly(config_pda().0, false),
						AccountMeta::new(custodians_pda().0, false),
					],
				),
				&[&authority()],
			)
			.unwrap_or_else(|error| panic!("set custodians: {error:?}"));

		// transfer (6): spend the deposited note into a fresh commitment.
		let tree = tree_pda().0;
		let account = program
			.account(&tree)
			.unwrap_or_else(|error| panic!("fetch tree: {error:?}"));
		let nodes = account.data[TREE_NODES_AT..TREE_NODES_AT + TREE_NODES * 32].to_vec();
		let mut root = [0_u8; 32];
		root.copy_from_slice(&nodes[(TREE_NODES - 1) * 32..]);
		let nullifier = nullifier_bytes(&secrets);
		let successor = prover::note_secrets(0xAB);
		let (path_elements, path_indices) = prover::build_witness(&nodes, 0);
		let (transfer_pk, _) = prover::seeded_setup(true, 0x2222);
		let circuit = prover::SpendCircuit {
			root: prover::fr_from_le(&root).unwrap(),
			nullifier: prover::fr_from_le(&nullifier).unwrap(),
			output_commitment: Some(prover::fr_from_le(&commitment_bytes(&successor)).unwrap()),
			amount: prover::amount_field(),
			witness: Some(prover::SpendWitness {
				spent: secrets,
				path_elements,
				path_indices,
				successor: Some(successor),
			}),
		};
		let wire = prover::serialize_proof(&prover::prove_spend(&transfer_pk, circuit, 0xCD));
		let (note_key, note_bump) = note_pda(&commitment_bytes(&successor));
		let mut transfer = vec![0_u8; TransferIx::SIZE];
		TransferIx::initialize(&mut transfer, |ix| {
			ix.bump = note_bump;
			ix.nullifier = nullifier;
			ix.root = root;
			ix.new_commitment = commitment_bytes(&successor);
			ix.new_view_pubkey = [0x0B; 32];
			ix.envelope_len = 128;
			ix.envelope = core::array::from_fn(|index| 0x40 ^ index as u8);
			ix.shares = core::array::from_fn(|index| 0x80 ^ index as u8);
			ix.proof_a = wire.a;
			ix.proof_b = wire.b;
			ix.proof_c = wire.c;
			Ok(())
		})
		.unwrap_or_else(|error| panic!("encode transfer: {error:?}"));
		let payer = Keypair::new_from_array([0xF1; 32]);
		program
			.fund(&payer.pubkey(), 1_000_000_000)
			.unwrap_or_else(|error| panic!("fund payer: {error:?}"));
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&transfer,
					vec![
						AccountMeta::new_readonly(config_pda().0, false),
						AccountMeta::new(payer.pubkey(), true),
						AccountMeta::new(tree, false),
						AccountMeta::new(nullifiers_pda().0, false),
						AccountMeta::new_readonly(vkey_pda(VK_SLOT_TRANSFER).0, false),
						AccountMeta::new(note_key, false),
						AccountMeta::new_readonly(system(), false),
					],
				),
				&[&payer],
			)
			.unwrap_or_else(|error| panic!("transfer: {error:?}"));

		// cancelDisclosure (12): the requester withdraws a pending request.
		let (cancel_request_key, cancel_bump) = request_pda(&requester.pubkey(), 5);
		file_request(
			&program,
			&requester,
			cancel_bump,
			5,
			program_under_test::TIER_COMPELLED,
		)
		.await;
		let mut cancel = vec![0_u8; CancelDisclosureIx::SIZE];
		CancelDisclosureIx::initialize(&mut cancel, |_| Ok(()))
			.unwrap_or_else(|error| panic!("encode cancel: {error:?}"));
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&cancel,
					vec![
						AccountMeta::new_readonly(requester.pubkey(), true),
						AccountMeta::new(cancel_request_key, false),
					],
				),
				&[&requester],
			)
			.unwrap_or_else(|error| panic!("cancel disclosure: {error:?}"));

		// challengeDisclosure (9) then resolveChallenge (10): a tier-1
		// request is challenged inside its window and resolved.
		let (challenge_request_key, challenge_bump) = request_pda(&requester.pubkey(), 6);
		file_request(
			&program,
			&requester,
			challenge_bump,
			6,
			program_under_test::TIER_VERIFIED,
		)
		.await;
		let viewer = view_key();
		let mut challenge = vec![0_u8; ChallengeDisclosureIx::SIZE];
		ChallengeDisclosureIx::initialize(&mut challenge, |_| Ok(()))
			.unwrap_or_else(|error| panic!("encode challenge: {error:?}"));
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&challenge,
					vec![
						AccountMeta::new(challenge_request_key, false),
						AccountMeta::new_readonly(note_pda(&commitment).0, false),
						AccountMeta::new_readonly(viewer.pubkey(), true),
						AccountMeta::new_readonly(clock(), false),
					],
				),
				&[&viewer],
			)
			.unwrap_or_else(|error| panic!("challenge disclosure: {error:?}"));

		let mut resolve = vec![0_u8; ResolveChallengeIx::SIZE];
		ResolveChallengeIx::initialize(&mut resolve, |ix| {
			ix.approve = 1;
			Ok(())
		})
		.unwrap_or_else(|error| panic!("encode resolve: {error:?}"));
		program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&resolve,
					vec![
						AccountMeta::new_readonly(authority().pubkey(), true),
						AccountMeta::new_readonly(config_pda().0, false),
						AccountMeta::new(challenge_request_key, false),
					],
				),
				&[&authority()],
			)
			.unwrap_or_else(|error| panic!("resolve challenge: {error:?}"));
	});
}

/// File one disclosure request with an opaque notice.
async fn file_request(program: &ProgramTest, requester: &Keypair, bump: u8, nonce: u64, tier: u8) {
	let mut request = vec![0_u8; RequestDisclosureIx::SIZE];
	RequestDisclosureIx::initialize(&mut request, |ix| {
		ix.bump = bump;
		ix.nonce.set(nonce);
		ix.tier = tier;
		ix.commitment = commitment_of_deposit();
		ix.notice_len = 96;
		ix.notice = core::array::from_fn(|index| 0xC0 ^ index as u8);
		ix.legal_basis_hash = [0x77; 32];
		Ok(())
	})
	.unwrap_or_else(|error| panic!("encode request: {error:?}"));
	let (request_key, _) = request_pda(&requester.pubkey(), nonce);
	program
		.send_with_signers(
			Instruction::new_with_bytes(
				program_id(),
				&request,
				vec![
					AccountMeta::new(requester.pubkey(), true),
					AccountMeta::new_readonly(config_pda().0, false),
					AccountMeta::new_readonly(requesters_pda().0, false),
					AccountMeta::new_readonly(note_pda(&commitment_of_deposit()).0, false),
					AccountMeta::new(request_key, false),
					AccountMeta::new_readonly(system(), false),
					AccountMeta::new_readonly(clock(), false),
				],
			),
			&[requester],
		)
		.unwrap_or_else(|error| panic!("file disclosure request: {error:?}"));
}

/// The commitment the fixed deposit seeds, recomputed from the same seed.
fn commitment_of_deposit() -> [u8; 32] {
	commitment_bytes(&prover::note_secrets(0x88))
}

fn clock() -> Pubkey {
	// The clock sysvar's fixed address.
	Pubkey::new_from_array([
		6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152, 105, 29, 94, 182, 139, 94, 184, 163,
		155, 75, 109, 92, 115, 85, 91, 33, 0, 0, 0, 0,
	])
}
