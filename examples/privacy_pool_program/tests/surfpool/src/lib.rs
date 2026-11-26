//! Surfpool end-to-end journeys for the upgraded privacy pool program.
//!
//! Runs against a real runtime through `pina test` with deterministic seeds so
//! every instruction discriminator (`Initialize` through `VerifyPolicy`) is
//! recorded cleanly by the benchmark harness.

#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::Instruction;
use pina_test::Keypair;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use pina_test::Signer;

#[path = "../../../src/lib.rs"]
mod program;

pub use program::*;

const SYSTEM_BYTES: [u8; 32] = [0; 32];

fn system() -> Pubkey {
	Pubkey::new_from_array(SYSTEM_BYTES)
}

fn program_id() -> Pubkey {
	let bytes: &[u8] = program::ID.as_ref();
	Pubkey::new_from_array(bytes.try_into().unwrap())
}

fn pina_address(pubkey: &Pubkey) -> Address {
	Address::new_from_array(pubkey.to_bytes())
}

fn token_program_id() -> Pubkey {
	let bytes: &[u8] = pina::token::ID.as_ref();
	Pubkey::new_from_array(bytes.try_into().unwrap())
}

fn bootstrap_authority() -> Keypair {
	Keypair::new_from_array([0xB0; 32])
}

fn program_data_address() -> Pubkey {
	Pubkey::find_program_address(
		&[program_id().as_ref()],
		&Pubkey::from_str_const("BPFLoaderUpgradeab1e11111111111111111111111"),
	)
	.0
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

fn relayer() -> Keypair {
	Keypair::new_from_array([0xF1; 32])
}

fn cosigner() -> Keypair {
	Keypair::new_from_array([0xE4; 32])
}

fn requester() -> Keypair {
	Keypair::new_from_array([0x7A; 32])
}

fn view_key() -> Keypair {
	Keypair::new_from_array([0x5A; 32])
}

fn clock() -> Pubkey {
	Pubkey::new_from_array([
		6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152, 105, 29, 94, 182, 139, 94, 184, 163,
		155, 75, 109, 92, 115, 85, 91, 33, 0, 0, 0, 0,
	])
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

fn nullifier_pda(nullifier: &[u8; 32]) -> (Pubkey, u8) {
	Pubkey::find_program_address(&[b"privacy-pool-nullifier", nullifier], &program_id())
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

fn asset_pda(mint: &Pubkey) -> (Pubkey, u8) {
	Pubkey::find_program_address(&[b"privacy-pool-asset", mint.as_ref()], &program_id())
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

static JOURNEY_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn journey_guard() -> std::sync::MutexGuard<'static, ()> {
	JOURNEY_LOCK
		.lock()
		.unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct StartedPool {
	program: ProgramTest,
	tree: prover::HostMerkleTree,
	note: prover::NoteSecrets,
	join_split_pk: ark_groth16::ProvingKey<ark_bn254::Bn254>,
	policy_pk: ark_groth16::ProvingKey<ark_bn254::Bn254>,
}

async fn start_pool() -> StartedPool {
	let pid = program_id();
	let program = ProgramTest::start(pid)
		.await
		.unwrap_or_else(|error| panic!("start program test: {error:?}"));

	let auth = bootstrap_authority();
	program
		.set_upgrade_authority(Some(auth.pubkey()))
		.unwrap_or_else(|error| panic!("configure test deployer: {error:?}"));
	program
		.fund(&auth.pubkey(), 3_000_000_000)
		.unwrap_or_else(|error| panic!("fund authority: {error:?}"));

	let (config_key, config_bump) = config_pda();
	let (vault_key, vault_bump) = vault_pda();
	let (tree_key, tree_bump) = tree_pda();
	let (custodians_key, custodians_bump) = custodians_pda();
	let (requesters_key, requesters_bump) = requesters_pda();
	let (log_key, log_bump) = log_pda();

	let mut init = vec![0_u8; InitializeIx::SIZE];
	InitializeIx::initialize(&mut init, |ix| {
		ix.config_bump = config_bump;
		ix.vault_bump = vault_bump;
		ix.tree_bump = tree_bump;
		ix.custodians_bump = custodians_bump;
		ix.requesters_bump = requesters_bump;
		ix.log_bump = log_bump;
		let mut committee = [0_u8; 96];
		for (slot, k) in [custodian_a(), custodian_b(), custodian_c()]
			.iter()
			.enumerate()
		{
			committee[slot * 32..(slot + 1) * 32].copy_from_slice(k.pubkey().as_ref());
		}
		ix.custodians = committee;
		Ok(())
	})
	.unwrap();

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
					AccountMeta::new(custodians_key, false),
					AccountMeta::new(requesters_key, false),
					AccountMeta::new(log_key, false),
					AccountMeta::new_readonly(system(), false),
					AccountMeta::new_readonly(program_data_address(), false),
				],
			),
			&[&auth],
		)
		.unwrap_or_else(|error| panic!("initialize pool: {error:?}"));

	let (join_split_pk, join_split_vk) = prover::seeded_setup_join_split(0x1111);
	let (policy_pk, policy_vk) = prover::seeded_setup_policy(0x2222);

	for (slot, wire) in [
		(VK_SLOT_JOIN_SPLIT, prover::serialize_vk(&join_split_vk)),
		(VK_SLOT_POLICY, prover::serialize_vk(&policy_vk)),
	] {
		let (vkey_key, vkey_bump) = vkey_pda(slot);
		let mut data = vec![0_u8; SetVerificationKeyIx::SIZE];
		SetVerificationKeyIx::initialize(&mut data, |ix| {
			ix.bump = vkey_bump;
			ix.slot = slot;
			ix.alpha_g1 = wire.alpha_g1;
			ix.beta_g2 = wire.beta_g2;
			ix.gamma_g2 = wire.gamma_g2;
			ix.delta_g2 = wire.delta_g2;
			ix.ic0 = wire.ic0;
			ix.ic1 = wire.ic1;
			Ok(())
		})
		.unwrap();
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

	// Variable-amount SOL deposit (800_000_000 lamports) with X25519 + AEAD envelope.
	let dep = depositor();
	program.fund(&dep.pubkey(), 2_000_000_000).unwrap();
	let note = prover::note_secrets(
		0x88,
		800_000_000,
		ark_bn254::Fr::from(0_u64),
		view_key().pubkey().to_bytes(),
	);
	let recipient_kp = shielded::ShieldedKeypair::from_seed([0x5E; 32]);
	let mut pt = [0_u8; 40];
	pt[..8].copy_from_slice(&note.amount.to_le_bytes());
	pt[8..].copy_from_slice(&prover::fr_to_le(&note.secret));
	let enc =
		shielded::encrypt_note_envelope(&recipient_kp.viewing_pubkey, &[0x44; 32], &pt).unwrap();
	let shares = shielded::encrypt_custodian_shares(
		&[
			shielded::ShieldedKeypair::from_seed([0xA1; 32]).viewing_pubkey,
			shielded::ShieldedKeypair::from_seed([0xB2; 32]).viewing_pubkey,
			shielded::ShieldedKeypair::from_seed([0xC3; 32]).viewing_pubkey,
		],
		&[0x44; 32],
		&enc.tx_viewing_key,
		&[0x55; 32],
	)
	.unwrap();

	let mut deposit = vec![0_u8; DepositIx::SIZE];
	DepositIx::initialize(&mut deposit, |ix| {
		ix.amount.set(note.amount);
		ix.commitment = note.note_id_bytes();
		ix.view_pubkey = note.view_pubkey;
		ix.envelope_len = enc.len;
		ix.envelope = enc.bytes;
		ix.shares = shares;
		Ok(())
	})
	.unwrap();
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
					AccountMeta::new_readonly(system(), false),
				],
			),
			&[&dep],
		)
		.unwrap_or_else(|error| panic!("deposit: {error:?}"));

	let mut tree = prover::HostMerkleTree::new();
	tree.push(note.leaf_bytes());

	StartedPool {
		program,
		tree,
		note,
		join_split_pk,
		policy_pk,
	}
}

#[test]
#[ignore = "run with pina test"]
fn bootstrap_requires_the_current_deployment_authority() {
	let _guard = journey_guard();
	pina_test::run(async {
		let mut program = ProgramTest::start(program_id()).await.unwrap();
		let deployer = bootstrap_authority();
		program
			.set_upgrade_authority(Some(deployer.pubkey()))
			.unwrap();
		let published = Keypair::new_from_array(*b"pina-pool-bootstrap-2026-09-26!!");
		let mut data = vec![0; InitializeIx::SIZE];
		InitializeIx::initialize(&mut data, |_| Ok(())).unwrap();

		for signer in [&published, &deployer] {
			if signer.pubkey() == deployer.pubkey() {
				program
					.set_upgrade_authority(Some(custodian_a().pubkey()))
					.unwrap();
			}
			program.fund(&signer.pubkey(), 1_000_000_000).unwrap();
			let error = program
				.send_with_signers(
					Instruction::new_with_bytes(
						program_id(),
						&data,
						vec![
							AccountMeta::new(signer.pubkey(), true),
							AccountMeta::new(config_pda().0, false),
							AccountMeta::new(vault_pda().0, false),
							AccountMeta::new(tree_pda().0, false),
							AccountMeta::new(custodians_pda().0, false),
							AccountMeta::new(requesters_pda().0, false),
							AccountMeta::new(log_pda().0, false),
							AccountMeta::new_readonly(system(), false),
							AccountMeta::new_readonly(program_data_address(), false),
						],
					),
					&[signer],
				)
				.expect_err("only the deployment authority may initialize");
			pina_test::assert_custom_error(&error, PrivacyPoolError::InvalidAuthority as u32);
		}
		program.stop().unwrap();
	});
}

#[test]
#[ignore = "run with pina test"]
fn withdraw_pays_recipient_and_relayer_with_compressed_proof() {
	let _guard = journey_guard();
	pina_test::run(async {
		let mut s = start_pool().await;
		let pid = program_id();
		let rel = relayer();
		s.program.fund(&rel.pubkey(), 1_000_000_000).unwrap();

		let root = s.tree.root();
		let (path_elements, path_indices) = s.tree.witness(0);
		let change = prover::note_secrets(
			0x89,
			290_000_000,
			ark_bn254::Fr::from(0_u64),
			view_key().pubkey().to_bytes(),
		);
		let n0 = s.note.nullifier_bytes();
		let (n0_key, n0_bump) = nullifier_pda(&n0);
		let zero_env = [0_u8; MAX_ENVELOPE_BYTES];
		let zero_shares = [0_u8; SHARE_BLOB_BYTES];

		let ext_hash = ext_data_digest(
			&pina_address(&recipient()),
			&pina_address(&rel.pubkey()),
			&Address::default(),
			&change.view_pubkey,
			0,
			&zero_env,
			&zero_shares,
			&[0_u8; 32],
			0,
			&zero_env,
			&zero_shares,
		)
		.unwrap();
		let amounts = pack_amounts(0, 500_000_000, 10_000_000);
		let statement = join_split_statement_hash(
			&root,
			&n0,
			&[0_u8; 32],
			&change.commitment_bytes(),
			&[0_u8; 32],
			&ext_hash,
			&[0_u8; 32],
			&amounts,
		)
		.unwrap();

		let circuit = prover::JoinSplitCircuit {
			statement_hash: prover::fr_from_le(&statement).unwrap(),
			witness: Some(prover::JoinSplitWitness {
				root: prover::fr_from_le(&root).unwrap(),
				asset_id: ark_bn254::Fr::from(0_u64),
				ext_data_hash: prover::fr_from_le(&ext_hash).unwrap(),
				public_in: 0,
				public_out: 500_000_000,
				relayer_fee: 10_000_000,
				inputs: [
					prover::InputWitness {
						note: s.note,
						path_elements,
						path_indices,
					},
					prover::InputWitness::dummy(ark_bn254::Fr::from(0_u64)),
				],
				outputs: [
					change,
					prover::NoteSecrets::dummy(ark_bn254::Fr::from(0_u64)),
				],
			}),
		};
		let proof = prover::prove_join_split(&s.join_split_pk, circuit, 0x99);
		let wire = prover::serialize_proof_compressed(&proof);

		let before = s.program.balance(&recipient()).unwrap_or(0);
		let mut data = vec![0_u8; WithdrawIx::SIZE];
		WithdrawIx::initialize(&mut data, |ix| {
			ix.nullifier_0_bump = n0_bump;
			ix.nullifier_1_bump = 0;
			ix.nullifier_0 = n0;
			ix.nullifier_1 = [0_u8; 32];
			ix.root = root;
			ix.public_out.set(500_000_000);
			ix.relayer_fee.set(10_000_000);
			ix.change_commitment = change.commitment_bytes();
			ix.change_view_pubkey = change.view_pubkey;
			ix.envelope_len = 0;
			ix.envelope = zero_env;
			ix.shares = zero_shares;
			ix.proof_a = wire.a;
			ix.proof_b = wire.b;
			ix.proof_c = wire.c;
			Ok(())
		})
		.unwrap();

		s.program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&data,
					vec![
						AccountMeta::new(rel.pubkey(), true),
						AccountMeta::new_readonly(config_pda().0, false),
						AccountMeta::new(vault_pda().0, false),
						AccountMeta::new(tree_pda().0, false),
						AccountMeta::new(n0_key, false),
						AccountMeta::new(Pubkey::new_from_array([0xDD; 32]), false),
						AccountMeta::new_readonly(vkey_pda(VK_SLOT_JOIN_SPLIT).0, false),
						AccountMeta::new(recipient(), false),
						AccountMeta::new_readonly(cosigner().pubkey(), false),
						AccountMeta::new_readonly(system(), false),
					],
				),
				&[&rel],
			)
			.unwrap_or_else(|error| panic!("withdraw: {error:?}"));

		assert_eq!(
			s.program.balance(&recipient()).unwrap(),
			before + 500_000_000
		);
		s.program.stop().unwrap();
	});
}

#[test]
#[ignore = "run with pina test"]
fn tier_zero_disclosure_and_every_remaining_instruction() {
	let _guard = journey_guard();
	pina_test::run(async {
		let mut s = start_pool().await;
		let pid = program_id();
		let auth = bootstrap_authority();
		let req = requester();
		let rel = relayer();
		s.program.fund(&req.pubkey(), 1_000_000_000).unwrap();
		s.program.fund(&rel.pubkey(), 1_000_000_000).unwrap();

		// 1. Tier-0 RequestDisclosure (with Merkle proof) -> GrantDisclosure -> ApproveDisclosure (salted scope hash in log)
		let path_bytes = s.tree.witness_bytes(0);
		let (req_key, req_bump) = request_pda(&req.pubkey(), 1);
		let mut req_data = vec![0_u8; RequestDisclosureIx::SIZE];
		RequestDisclosureIx::initialize(&mut req_data, |ix| {
			ix.bump = req_bump;
			ix.nonce.set(1);
			ix.tier = TIER_CONSENT;
			ix.commitment = s.note.commitment_bytes();
			ix.view_pubkey = s.note.view_pubkey;
			ix.leaf_index.set(0);
			ix.path_elements = path_bytes;
			ix.notice_len = 16;
			ix.notice = [0xC0; 96];
			ix.legal_basis_hash = [0; 32];
			Ok(())
		})
		.unwrap();
		s.program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&req_data,
					vec![
						AccountMeta::new(req.pubkey(), true),
						AccountMeta::new_readonly(config_pda().0, false),
						AccountMeta::new_readonly(requesters_pda().0, false),
						AccountMeta::new_readonly(tree_pda().0, false),
						AccountMeta::new(req_key, false),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(clock(), false),
					],
				),
				&[&req],
			)
			.unwrap();

		let viewer = view_key();
		let mut grant = vec![0_u8; GrantDisclosureIx::SIZE];
		GrantDisclosureIx::initialize(&mut grant, |_| Ok(())).unwrap();
		s.program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&grant,
					vec![
						AccountMeta::new(req_key, false),
						AccountMeta::new_readonly(viewer.pubkey(), true),
					],
				),
				&[&viewer],
			)
			.unwrap();

		for cust in [custodian_a(), custodian_b()] {
			let mut app = vec![0_u8; ApproveDisclosureIx::SIZE];
			ApproveDisclosureIx::initialize(&mut app, |_| Ok(())).unwrap();
			s.program
				.send_with_signers(
					Instruction::new_with_bytes(
						pid,
						&app,
						vec![
							AccountMeta::new_readonly(cust.pubkey(), true),
							AccountMeta::new_readonly(config_pda().0, false),
							AccountMeta::new_readonly(custodians_pda().0, false),
							AccountMeta::new(req_key, false),
							AccountMeta::new(log_pda().0, false),
							AccountMeta::new_readonly(clock(), false),
						],
					),
					&[&cust],
				)
				.unwrap();
		}

		// Verify privacy-preserving salted scope hash in DisclosureLog.
		let log_acc = s.program.account(&log_pda().0).unwrap();
		let expected_scope =
			disclosure_scope_hash(&s.note.commitment_bytes(), &pina_address(&req.pubkey()), 1);
		assert_eq!(&log_acc.data[43..75], &expected_scope[..]);

		// 2. RegisterRequester, SetCustodians, CancelDisclosure, ChallengeDisclosure, ResolveChallenge
		let mut reg = vec![0_u8; RegisterRequesterIx::SIZE];
		RegisterRequesterIx::initialize(&mut reg, |ix| {
			ix.requester = pina_address(&req.pubkey());
			ix.max_tier = TIER_COMPELLED;
			Ok(())
		})
		.unwrap();
		s.program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&reg,
					vec![
						AccountMeta::new_readonly(auth.pubkey(), true),
						AccountMeta::new_readonly(config_pda().0, false),
						AccountMeta::new(requesters_pda().0, false),
					],
				),
				&[&auth],
			)
			.unwrap();

		let mut set_cust = vec![0_u8; SetCustodiansIx::SIZE];
		SetCustodiansIx::initialize(&mut set_cust, |ix| {
			let mut committee = [0_u8; 96];
			for (slot, k) in [custodian_a(), custodian_b(), custodian_c()]
				.iter()
				.enumerate()
			{
				committee[slot * 32..(slot + 1) * 32].copy_from_slice(k.pubkey().as_ref());
			}
			ix.custodians = committee;
			Ok(())
		})
		.unwrap();
		s.program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&set_cust,
					vec![
						AccountMeta::new_readonly(auth.pubkey(), true),
						AccountMeta::new_readonly(config_pda().0, false),
						AccountMeta::new(custodians_pda().0, false),
					],
				),
				&[&auth],
			)
			.unwrap();

		// File & Cancel tier-2 request
		let (cancel_key, cancel_bump) = request_pda(&req.pubkey(), 2);
		let mut req2 = vec![0_u8; RequestDisclosureIx::SIZE];
		RequestDisclosureIx::initialize(&mut req2, |ix| {
			ix.bump = cancel_bump;
			ix.nonce.set(2);
			ix.tier = TIER_COMPELLED;
			ix.commitment = s.note.commitment_bytes();
			ix.view_pubkey = s.note.view_pubkey;
			ix.leaf_index.set(0);
			ix.path_elements = path_bytes;
			ix.notice_len = 8;
			ix.notice = [0xC1; 96];
			ix.legal_basis_hash = [0x77; 32];
			Ok(())
		})
		.unwrap();
		s.program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&req2,
					vec![
						AccountMeta::new(req.pubkey(), true),
						AccountMeta::new_readonly(config_pda().0, false),
						AccountMeta::new_readonly(requesters_pda().0, false),
						AccountMeta::new_readonly(tree_pda().0, false),
						AccountMeta::new(cancel_key, false),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(clock(), false),
					],
				),
				&[&req],
			)
			.unwrap();
		let mut cancel = vec![0_u8; CancelDisclosureIx::SIZE];
		CancelDisclosureIx::initialize(&mut cancel, |_| Ok(())).unwrap();
		s.program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&cancel,
					vec![
						AccountMeta::new_readonly(req.pubkey(), true),
						AccountMeta::new(cancel_key, false),
					],
				),
				&[&req],
			)
			.unwrap();

		// File, Challenge, and Resolve tier-1 request
		let (ch_key, ch_bump) = request_pda(&req.pubkey(), 3);
		let mut req3 = vec![0_u8; RequestDisclosureIx::SIZE];
		RequestDisclosureIx::initialize(&mut req3, |ix| {
			ix.bump = ch_bump;
			ix.nonce.set(3);
			ix.tier = TIER_VERIFIED;
			ix.commitment = s.note.commitment_bytes();
			ix.view_pubkey = s.note.view_pubkey;
			ix.leaf_index.set(0);
			ix.path_elements = path_bytes;
			ix.notice_len = 8;
			ix.notice = [0xC2; 96];
			ix.legal_basis_hash = [0x78; 32];
			Ok(())
		})
		.unwrap();
		s.program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&req3,
					vec![
						AccountMeta::new(req.pubkey(), true),
						AccountMeta::new_readonly(config_pda().0, false),
						AccountMeta::new_readonly(requesters_pda().0, false),
						AccountMeta::new_readonly(tree_pda().0, false),
						AccountMeta::new(ch_key, false),
						AccountMeta::new_readonly(system(), false),
						AccountMeta::new_readonly(clock(), false),
					],
				),
				&[&req],
			)
			.unwrap();
		let mut ch = vec![0_u8; ChallengeDisclosureIx::SIZE];
		ChallengeDisclosureIx::initialize(&mut ch, |_| Ok(())).unwrap();
		s.program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&ch,
					vec![
						AccountMeta::new(ch_key, false),
						AccountMeta::new_readonly(viewer.pubkey(), true),
						AccountMeta::new_readonly(clock(), false),
					],
				),
				&[&viewer],
			)
			.unwrap();
		let mut res = vec![0_u8; ResolveChallengeIx::SIZE];
		ResolveChallengeIx::initialize(&mut res, |ix| {
			ix.approve = 1;
			Ok(())
		})
		.unwrap();
		s.program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&res,
					vec![
						AccountMeta::new_readonly(auth.pubkey(), true),
						AccountMeta::new_readonly(config_pda().0, false),
						AccountMeta::new(ch_key, false),
					],
				),
				&[&auth],
			)
			.unwrap();

		// 3. Transfer (2-in-2-out join-split splitting 800M into 500M + 295M + 5M relayer fee)
		let root = s.tree.root();
		let (path_elements, path_indices) = s.tree.witness(0);
		let out0 = prover::note_secrets(0xAB, 500_000_000, ark_bn254::Fr::from(0_u64), [0x0B; 32]);
		let out1 = prover::note_secrets(0xAC, 295_000_000, ark_bn254::Fr::from(0_u64), [0x0C; 32]);
		let n0 = s.note.nullifier_bytes();
		let (n0_key, n0_bump) = nullifier_pda(&n0);
		let zero_env = [0_u8; MAX_ENVELOPE_BYTES];
		let zero_shares = [0_u8; SHARE_BLOB_BYTES];
		let ext_hash = ext_data_digest(
			&Address::default(),
			&pina_address(&rel.pubkey()),
			&Address::default(),
			&out0.view_pubkey,
			0,
			&zero_env,
			&zero_shares,
			&out1.view_pubkey,
			0,
			&zero_env,
			&zero_shares,
		)
		.unwrap();
		let amounts = pack_amounts(0, 0, 5_000_000);
		let statement = join_split_statement_hash(
			&root,
			&n0,
			&[0_u8; 32],
			&out0.commitment_bytes(),
			&out1.commitment_bytes(),
			&ext_hash,
			&[0_u8; 32],
			&amounts,
		)
		.unwrap();
		let tcircuit = prover::JoinSplitCircuit {
			statement_hash: prover::fr_from_le(&statement).unwrap(),
			witness: Some(prover::JoinSplitWitness {
				root: prover::fr_from_le(&root).unwrap(),
				asset_id: ark_bn254::Fr::from(0_u64),
				ext_data_hash: prover::fr_from_le(&ext_hash).unwrap(),
				public_in: 0,
				public_out: 0,
				relayer_fee: 5_000_000,
				inputs: [
					prover::InputWitness {
						note: s.note,
						path_elements,
						path_indices,
					},
					prover::InputWitness::dummy(ark_bn254::Fr::from(0_u64)),
				],
				outputs: [out0, out1],
			}),
		};
		let wire = prover::serialize_proof_compressed(&prover::prove_join_split(
			&s.join_split_pk,
			tcircuit,
			0xCD,
		));
		let mut transfer = vec![0_u8; TransferIx::SIZE];
		TransferIx::initialize(&mut transfer, |ix| {
			ix.nullifier_0_bump = n0_bump;
			ix.nullifier_1_bump = 0;
			ix.nullifier_0 = n0;
			ix.nullifier_1 = [0_u8; 32];
			ix.root = root;
			ix.relayer_fee.set(5_000_000);
			ix.out0_commitment = out0.commitment_bytes();
			ix.out0_view_pubkey = out0.view_pubkey;
			ix.out0_envelope_len = 0;
			ix.out0_envelope = zero_env;
			ix.out1_commitment = out1.commitment_bytes();
			ix.out1_view_pubkey = out1.view_pubkey;
			ix.out1_envelope_len = 0;
			ix.out1_envelope = zero_env;
			ix.proof_a = wire.a;
			ix.proof_b = wire.b;
			ix.proof_c = wire.c;
			Ok(())
		})
		.unwrap();
		s.program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&transfer,
					vec![
						AccountMeta::new(rel.pubkey(), true),
						AccountMeta::new_readonly(config_pda().0, false),
						AccountMeta::new(vault_pda().0, false),
						AccountMeta::new(tree_pda().0, false),
						AccountMeta::new(n0_key, false),
						AccountMeta::new(Pubkey::new_from_array([0xDE; 32]), false),
						AccountMeta::new_readonly(vkey_pda(VK_SLOT_JOIN_SPLIT).0, false),
						AccountMeta::new_readonly(cosigner().pubkey(), false),
						AccountMeta::new_readonly(system(), false),
					],
				),
				&[&rel],
			)
			.unwrap();
		s.tree.push(out0.leaf_bytes());
		s.tree.push(out1.leaf_bytes());

		// 4. SetPolicy & VerifyPolicy (Custom Ring policy proof)
		let mut sp = vec![0_u8; SetPolicyIx::SIZE];
		SetPolicyIx::initialize(&mut sp, |ix| {
			ix.policy_flags = 0;
			ix.policy_cosigner = pina_address(&cosigner().pubkey());
			Ok(())
		})
		.unwrap();
		s.program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&sp,
					vec![
						AccountMeta::new_readonly(auth.pubkey(), true),
						AccountMeta::new(config_pda().0, false),
					],
				),
				&[&auth],
			)
			.unwrap();

		let id_secret = ark_bn254::Fr::from(42_u64);
		let salt = ark_bn254::Fr::from(99_u64);
		let policy_hash =
			poseidon2(&prover::fr_to_le(&id_secret), &prover::fr_to_le(&salt)).unwrap();
		let pproof = prover::prove_policy(
			&s.policy_pk,
			prover::PolicyCircuit {
				policy_hash: prover::fr_from_le(&policy_hash).unwrap(),
				identity_secret: Some(id_secret),
				policy_salt: Some(salt),
			},
			0x77,
		);
		let pwire = prover::serialize_proof_compressed(&pproof);
		let mut vp = vec![0_u8; VerifyPolicyIx::SIZE];
		VerifyPolicyIx::initialize(&mut vp, |ix| {
			ix.policy_hash = policy_hash;
			ix.proof_a = pwire.a;
			ix.proof_b = pwire.b;
			ix.proof_c = pwire.c;
			Ok(())
		})
		.unwrap();
		s.program
			.send_instruction(Instruction::new_with_bytes(
				pid,
				&vp,
				vec![AccountMeta::new_readonly(vkey_pda(VK_SLOT_POLICY).0, false)],
			))
			.unwrap();

		// 5. RegisterAsset, DepositToken, WithdrawToken (SPL Token)
		let mint = Keypair::new_from_array([0x1A; 32]);
		let dep = depositor();
		let (av_key, av_bump) = asset_pda(&mint.pubkey());
		let vault_ta = Keypair::new_from_array([0x1B; 32]);
		let dep_ta = Keypair::new_from_array([0x1C; 32]);
		let rec_ta = Keypair::new_from_array([0x1D; 32]);
		let rel_ta = Keypair::new_from_array([0x1E; 32]);
		let payer_pk = s.program.payer();
		let tok_prog = token_program_id();

		let create_acc = |new_acc: &Keypair, space: u64| {
			let lamports = pina_test::Rent::default().minimum_balance(space as usize);
			let mut d = vec![0_u8, 0, 0, 0];
			d.extend_from_slice(&lamports.to_le_bytes());
			d.extend_from_slice(&space.to_le_bytes());
			d.extend_from_slice(tok_prog.as_ref());
			s.program
				.send_with_signers(
					Instruction::new_with_bytes(
						system(),
						&d,
						vec![
							AccountMeta::new(payer_pk, true),
							AccountMeta::new(new_acc.pubkey(), true),
							AccountMeta::new_readonly(tok_prog, false),
						],
					),
					&[new_acc],
				)
				.unwrap();
		};
		// InitializeMint2 (tag 20)
		create_acc(&mint, 82);
		let mut init_mint = vec![20_u8, 6_u8];
		init_mint.extend_from_slice(auth.pubkey().as_ref());
		init_mint.extend_from_slice(&0_u32.to_le_bytes());
		s.program
			.send_instruction(Instruction::new_with_bytes(
				tok_prog,
				&init_mint,
				vec![AccountMeta::new(mint.pubkey(), false)],
			))
			.unwrap();

		// InitializeAccount3 (tag 18) for vault, depositor, recipient, relayer
		for (ta, owner) in [
			(&vault_ta, av_key),
			(&dep_ta, dep.pubkey()),
			(&rec_ta, recipient()),
			(&rel_ta, rel.pubkey()),
		] {
			create_acc(ta, 165);
			let mut init_ta = vec![18_u8];
			init_ta.extend_from_slice(owner.as_ref());
			s.program
				.send_instruction(Instruction::new_with_bytes(
					tok_prog,
					&init_ta,
					vec![
						AccountMeta::new(ta.pubkey(), false),
						AccountMeta::new_readonly(mint.pubkey(), false),
					],
				))
				.unwrap();
		}

		// MintTo (tag 7)
		let mut mint_to = vec![7_u8];
		mint_to.extend_from_slice(&1_000_000_u64.to_le_bytes());
		s.program
			.send_with_signers(
				Instruction::new_with_bytes(
					tok_prog,
					&mint_to,
					vec![
						AccountMeta::new(mint.pubkey(), false),
						AccountMeta::new(dep_ta.pubkey(), false),
						AccountMeta::new_readonly(auth.pubkey(), true),
					],
				),
				&[&auth],
			)
			.unwrap();

		let mut reg_asset = vec![0_u8; RegisterAssetIx::SIZE];
		RegisterAssetIx::initialize(&mut reg_asset, |ix| {
			ix.bump = av_bump;
			Ok(())
		})
		.unwrap();
		s.program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&reg_asset,
					vec![
						AccountMeta::new(auth.pubkey(), true),
						AccountMeta::new_readonly(mint.pubkey(), false),
						AccountMeta::new(av_key, false),
						AccountMeta::new_readonly(vault_ta.pubkey(), false),
						AccountMeta::new_readonly(token_program_id(), false),
						AccountMeta::new_readonly(system(), false),
					],
				),
				&[&auth],
			)
			.unwrap();

		let mint_asset_bytes = asset_id(&pina_address(&mint.pubkey())).unwrap();
		let mint_asset_fr = prover::fr_from_le(&mint_asset_bytes).unwrap();
		let tok_note = prover::note_secrets(
			0x701,
			500_000,
			mint_asset_fr,
			view_key().pubkey().to_bytes(),
		);
		let mut dep_tok = vec![0_u8; DepositTokenIx::SIZE];
		DepositTokenIx::initialize(&mut dep_tok, |ix| {
			ix.amount.set(tok_note.amount);
			ix.commitment = tok_note.note_id_bytes();
			ix.view_pubkey = tok_note.view_pubkey;
			ix.envelope_len = 0;
			ix.envelope = zero_env;
			ix.shares = zero_shares;
			Ok(())
		})
		.unwrap();
		s.program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&dep_tok,
					vec![
						AccountMeta::new(dep.pubkey(), true),
						AccountMeta::new(config_pda().0, false),
						AccountMeta::new(tree_pda().0, false),
						AccountMeta::new_readonly(mint.pubkey(), false),
						AccountMeta::new_readonly(av_key, false),
						AccountMeta::new(dep_ta.pubkey(), false),
						AccountMeta::new(vault_ta.pubkey(), false),
						AccountMeta::new_readonly(token_program_id(), false),
					],
				),
				&[&dep],
			)
			.unwrap();
		let tok_idx = s.tree.push(tok_note.leaf_bytes());

		let tok_root = s.tree.root();
		let (tok_pe, tok_pi) = s.tree.witness(tok_idx);
		let tok_n0 = tok_note.nullifier_bytes();
		let (tok_n0_key, tok_n0_bump) = nullifier_pda(&tok_n0);
		let tok_ext = ext_data_digest(
			&pina_address(&rec_ta.pubkey()),
			&pina_address(&rel_ta.pubkey()),
			&pina_address(&mint.pubkey()),
			&[0_u8; 32],
			0,
			&zero_env,
			&zero_shares,
			&[0_u8; 32],
			0,
			&zero_env,
			&zero_shares,
		)
		.unwrap();
		let tok_amounts = pack_amounts(0, 490_000, 10_000);
		let tok_stmt = join_split_statement_hash(
			&tok_root,
			&tok_n0,
			&[0_u8; 32],
			&[0_u8; 32],
			&[0_u8; 32],
			&tok_ext,
			&mint_asset_bytes,
			&tok_amounts,
		)
		.unwrap();
		let wt_circuit = prover::JoinSplitCircuit {
			statement_hash: prover::fr_from_le(&tok_stmt).unwrap(),
			witness: Some(prover::JoinSplitWitness {
				root: prover::fr_from_le(&tok_root).unwrap(),
				asset_id: mint_asset_fr,
				ext_data_hash: prover::fr_from_le(&tok_ext).unwrap(),
				public_in: 0,
				public_out: 490_000,
				relayer_fee: 10_000,
				inputs: [
					prover::InputWitness {
						note: tok_note,
						path_elements: tok_pe,
						path_indices: tok_pi,
					},
					prover::InputWitness::dummy(mint_asset_fr),
				],
				outputs: [
					prover::NoteSecrets::dummy(mint_asset_fr),
					prover::NoteSecrets::dummy(mint_asset_fr),
				],
			}),
		};
		let wt_wire = prover::serialize_proof_compressed(&prover::prove_join_split(
			&s.join_split_pk,
			wt_circuit,
			0x702,
		));
		let mut wt = vec![0_u8; WithdrawTokenIx::SIZE];
		WithdrawTokenIx::initialize(&mut wt, |ix| {
			ix.nullifier_0_bump = tok_n0_bump;
			ix.nullifier_1_bump = 0;
			ix.nullifier_0 = tok_n0;
			ix.nullifier_1 = [0_u8; 32];
			ix.root = tok_root;
			ix.public_out.set(490_000);
			ix.relayer_fee.set(10_000);
			ix.change_commitment = [0_u8; 32];
			ix.change_view_pubkey = [0_u8; 32];
			ix.envelope_len = 0;
			ix.envelope = zero_env;
			ix.proof_a = wt_wire.a;
			ix.proof_b = wt_wire.b;
			ix.proof_c = wt_wire.c;
			Ok(())
		})
		.unwrap();
		s.program
			.send_with_signers(
				Instruction::new_with_bytes(
					pid,
					&wt,
					vec![
						AccountMeta::new(rel.pubkey(), true),
						AccountMeta::new_readonly(config_pda().0, false),
						AccountMeta::new(tree_pda().0, false),
						AccountMeta::new(tok_n0_key, false),
						AccountMeta::new(Pubkey::new_from_array([0xDF; 32]), false),
						AccountMeta::new_readonly(vkey_pda(VK_SLOT_JOIN_SPLIT).0, false),
						AccountMeta::new_readonly(mint.pubkey(), false),
						AccountMeta::new_readonly(av_key, false),
						AccountMeta::new(vault_ta.pubkey(), false),
						AccountMeta::new(rec_ta.pubkey(), false),
						AccountMeta::new(rel_ta.pubkey(), false),
						AccountMeta::new_readonly(cosigner().pubkey(), false),
						AccountMeta::new_readonly(token_program_id(), false),
						AccountMeta::new_readonly(system(), false),
					],
				),
				&[&rel],
			)
			.unwrap();

		s.program.stop().unwrap();
	});
}
