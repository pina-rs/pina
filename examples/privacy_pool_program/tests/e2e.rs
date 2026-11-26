//! SBF end-to-end tests for the upgraded privacy pool program.
//!
//! Build the program and run these ignored tests with:
//!
//! ```sh
//! cargo build-privacy-pool-program
//! ```
//!
//! ```sh
//! SBF_OUT_DIR="$PWD/target/deploy" \
//!     cargo test -p privacy_pool_program --features prover --test e2e -- --include-ignored
//! ```

#[path = "../src/lib.rs"]
mod program;

use mollusk_svm::Mollusk;
use mollusk_svm::result::Check;
use mollusk_svm::result::InstructionResult;
use pina::ProgramError;
pub use program::*;
use solana_account::Account;
use solana_instruction::AccountMeta;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;
use solana_sdk_ids::system_program;

const RENT_LAMPORTS: u64 = 100_000_000;
const FUND: u64 = 10_000_000_000;

fn program_id() -> Pubkey {
	let bytes: &[u8] = ID.as_ref();
	let array: [u8; 32] = bytes.try_into().unwrap_or_else(|_| panic!("id"));
	Pubkey::new_from_array(array)
}

fn pina_address(pubkey: &Pubkey) -> Address {
	Address::new_from_array(pubkey.to_bytes())
}

fn key(seed: u8) -> Pubkey {
	let mut bytes = [0_u8; 32];
	bytes[0] = seed;
	Pubkey::new_from_array(bytes)
}

fn bootstrap_authority() -> Pubkey {
	key(42)
}

fn program_data_fixture() -> (Pubkey, Account) {
	let loader = solana_pubkey::pubkey!("BPFLoaderUpgradeab1e11111111111111111111111");
	let address = Pubkey::find_program_address(&[program_id().as_ref()], &loader).0;
	let mut account = Account::new(RENT_LAMPORTS, 45, &loader);
	account.data[..4].copy_from_slice(&3_u32.to_le_bytes());
	account.data[12] = 1;
	account.data[13..45].copy_from_slice(bootstrap_authority().as_ref());
	(address, account)
}

fn create_mollusk() -> Mollusk {
	let so_name = "privacy_pool_program.so";
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
		"privacy_pool_program SBF binary not found; build it with `cargo \
		 build-privacy-pool-program`"
	);

	Mollusk::new(&program_id(), "privacy_pool_program")
}

// ---------------------------------------------------------------------------
// PDA derivations
// ---------------------------------------------------------------------------

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

fn vkey_pda(slot: u8) -> (Pubkey, u8) {
	let seed = u64::from(slot).to_le_bytes();
	Pubkey::find_program_address(&[b"privacy-pool-vkey", &seed], &program_id())
}

fn wallet(seed: u8) -> (Pubkey, Account) {
	(key(seed), Account::new(FUND, 0, &system_program::id()))
}

struct World {
	accounts: Vec<(Pubkey, Account)>,
}

impl World {
	fn new() -> Self {
		World {
			accounts: vec![program_data_fixture()],
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

fn initialize_ix(custodians: [Pubkey; 3]) -> Instruction {
	let (config_key, config_bump) = config_pda();
	let (vault_key, vault_bump) = vault_pda();
	let (tree_key, tree_bump) = tree_pda();
	let (custodians_key, custodians_bump) = custodians_pda();
	let (requesters_key, requesters_bump) = requesters_pda();
	let (log_key, log_bump) = log_pda();

	let mut data = vec![0_u8; InitializeIx::SIZE];
	InitializeIx::initialize(&mut data, |ix| {
		ix.config_bump = config_bump;
		ix.vault_bump = vault_bump;
		ix.tree_bump = tree_bump;
		ix.custodians_bump = custodians_bump;
		ix.requesters_bump = requesters_bump;
		ix.log_bump = log_bump;
		let mut committee = [0_u8; 96];
		for (slot, k) in custodians.iter().enumerate() {
			committee[slot * 32..(slot + 1) * 32].copy_from_slice(k.as_ref());
		}
		ix.custodians = committee;
		Ok(())
	})
	.unwrap();

	Instruction::new_with_bytes(
		program_id(),
		&data,
		vec![
			AccountMeta::new(bootstrap_authority(), true),
			AccountMeta::new(config_key, false),
			AccountMeta::new(vault_key, false),
			AccountMeta::new(tree_key, false),
			AccountMeta::new(custodians_key, false),
			AccountMeta::new(requesters_key, false),
			AccountMeta::new(log_key, false),
			AccountMeta::new_readonly(system_program::id(), false),
			AccountMeta::new_readonly(program_data_fixture().0, false),
		],
	)
}

fn set_vkey_ix(slot: u8, wire: &prover::WireVerifyingKey) -> Instruction {
	let (vkey_key, bump) = vkey_pda(slot);
	let mut data = vec![0_u8; SetVerificationKeyIx::SIZE];
	SetVerificationKeyIx::initialize(&mut data, |ix| {
		ix.bump = bump;
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

	Instruction::new_with_bytes(
		program_id(),
		&data,
		vec![
			AccountMeta::new(bootstrap_authority(), true),
			AccountMeta::new_readonly(config_pda().0, false),
			AccountMeta::new(vkey_key, false),
			AccountMeta::new_readonly(system_program::id(), false),
		],
	)
}

struct Pool {
	mollusk: Mollusk,
	world: World,
	tree: prover::HostMerkleTree,
	join_split_pk: ark_groth16::ProvingKey<ark_bn254::Bn254>,
	policy_pk: ark_groth16::ProvingKey<ark_bn254::Bn254>,
}

fn setup_pool() -> Pool {
	let mollusk = create_mollusk();
	let mut world = World::new();

	world.add(
		bootstrap_authority(),
		Account::new(FUND, 0, &system_program::id()),
	);
	for seed in 2..=12 {
		let (pubkey, account) = wallet(seed);
		world.add(pubkey, account);
	}

	let custodians = [key(21), key(22), key(23)];
	world.add(config_pda().0, Account::default());
	world.add(vault_pda().0, Account::default());
	world.add(tree_pda().0, Account::default());
	world.add(custodians_pda().0, Account::default());
	world.add(requesters_pda().0, Account::default());
	world.add(log_pda().0, Account::default());
	world.add(
		system_program::id(),
		mollusk_svm::program::keyed_account_for_system_program().1,
	);
	world.run(&mollusk, &initialize_ix(custodians), &[Check::success()]);

	let (join_split_pk, join_split_verifying) = prover::seeded_setup_join_split(0x1111);
	let (policy_pk, policy_verifying) = prover::seeded_setup_policy(0x2222);

	world.add(vkey_pda(VK_SLOT_JOIN_SPLIT).0, Account::default());
	world.add(vkey_pda(VK_SLOT_POLICY).0, Account::default());
	world.run(
		&mollusk,
		&set_vkey_ix(
			VK_SLOT_JOIN_SPLIT,
			&prover::serialize_vk(&join_split_verifying),
		),
		&[Check::success()],
	);
	world.run(
		&mollusk,
		&set_vkey_ix(VK_SLOT_POLICY, &prover::serialize_vk(&policy_verifying)),
		&[Check::success()],
	);

	Pool {
		mollusk,
		world,
		tree: prover::HostMerkleTree::new(),
		join_split_pk,
		policy_pk,
	}
}

fn deposit_sol(pool: &mut Pool, depositor: Pubkey, note: &prover::NoteSecrets) -> usize {
	let recipient_kp = shielded::ShieldedKeypair::from_seed([0x33; 32]);
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

	let mut data = vec![0_u8; DepositIx::SIZE];
	DepositIx::initialize(&mut data, |ix| {
		ix.amount.set(note.amount);
		ix.commitment = note.note_id_bytes();
		ix.view_pubkey = note.view_pubkey;
		ix.envelope_len = enc.len;
		ix.envelope = enc.bytes;
		ix.shares = shares;
		Ok(())
	})
	.unwrap();

	let ix = Instruction::new_with_bytes(
		program_id(),
		&data,
		vec![
			AccountMeta::new(depositor, true),
			AccountMeta::new(config_pda().0, false),
			AccountMeta::new(vault_pda().0, false),
			AccountMeta::new(tree_pda().0, false),
			AccountMeta::new_readonly(system_program::id(), false),
		],
	);
	pool.world.run(&pool.mollusk, &ix, &[Check::success()]);
	pool.tree.push(note.leaf_bytes())
}

#[test]
#[ignore = "requires SBF artifact; run with `cargo build-privacy-pool-program`"]
fn variable_deposit_withdraw_with_relayer_and_double_spend_rejection() {
	let mut pool = setup_pool();
	let depositor = key(2);
	let recipient = key(3);
	let relayer = key(4);
	let cosigner = key(5);
	let view_pk = key(6).to_bytes();

	let note = prover::note_secrets(0x101, 750_000_000, ark_bn254::Fr::from(0_u64), view_pk);
	let leaf_idx = deposit_sol(&mut pool, depositor, &note);
	let root = pool.tree.root();

	// Withdraw 500_000_000 to recipient, 5_000_000 to relayer, 245_000_000 as change.
	let change = prover::note_secrets(0x102, 245_000_000, ark_bn254::Fr::from(0_u64), view_pk);
	let (path_elements, path_indices) = pool.tree.witness(leaf_idx);
	let n0 = note.nullifier_bytes();
	let n1 = [0_u8; 32];
	let (n0_key, n0_bump) = nullifier_pda(&n0);
	let dummy_n1_key = key(9);
	pool.world.add(n0_key, Account::default());

	let zero_env = [0_u8; MAX_ENVELOPE_BYTES];
	let zero_shares = [0_u8; SHARE_BLOB_BYTES];
	let ext_hash = ext_data_digest(
		&pina_address(&recipient),
		&pina_address(&relayer),
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
	let amounts = pack_amounts(0, 500_000_000, 5_000_000);
	let statement = join_split_statement_hash(
		&root,
		&n0,
		&n1,
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
			relayer_fee: 5_000_000,
			inputs: [
				prover::InputWitness {
					note,
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
	let proof = prover::prove_join_split(&pool.join_split_pk, circuit, 0x303);
	let wire = prover::serialize_proof_compressed(&proof);

	let mut data = vec![0_u8; WithdrawIx::SIZE];
	WithdrawIx::initialize(&mut data, |ix| {
		ix.nullifier_0_bump = n0_bump;
		ix.nullifier_1_bump = 0;
		ix.nullifier_0 = n0;
		ix.nullifier_1 = n1;
		ix.root = root;
		ix.public_out.set(500_000_000);
		ix.relayer_fee.set(5_000_000);
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

	let recipient_before = pool.world.get(&recipient).lamports;
	let relayer_before = pool.world.get(&relayer).lamports;

	let ix = Instruction::new_with_bytes(
		program_id(),
		&data,
		vec![
			AccountMeta::new(relayer, true),
			AccountMeta::new_readonly(config_pda().0, false),
			AccountMeta::new(vault_pda().0, false),
			AccountMeta::new(tree_pda().0, false),
			AccountMeta::new(n0_key, false),
			AccountMeta::new(dummy_n1_key, false),
			AccountMeta::new_readonly(vkey_pda(VK_SLOT_JOIN_SPLIT).0, false),
			AccountMeta::new(recipient, false),
			AccountMeta::new_readonly(cosigner, false),
			AccountMeta::new_readonly(system_program::id(), false),
		],
	);
	pool.world.run(&pool.mollusk, &ix, &[Check::success()]);

	assert_eq!(
		pool.world.get(&recipient).lamports,
		recipient_before + 500_000_000
	);
	assert!(pool.world.get(&relayer).lamports > relayer_before);

	// Replaying the same nullifier must be rejected by the per-nullifier PDA.
	pool.world.run(
		&pool.mollusk,
		&ix,
		&[Check::err(ProgramError::Custom(
			PrivacyPoolError::NullifierAlreadySpent as u32,
		))],
	);
}

#[test]
#[ignore = "requires SBF artifact; run with `cargo build-privacy-pool-program`"]
fn policy_pause_cosigner_and_zk_policy_proof() {
	let mut pool = setup_pool();
	let cosigner = key(8);

	// Pause the pool and verify Deposit is rejected.
	let mut pause_data = vec![0_u8; SetPolicyIx::SIZE];
	SetPolicyIx::initialize(&mut pause_data, |ix| {
		ix.policy_flags = POLICY_PAUSED;
		ix.policy_cosigner = pina_address(&cosigner);
		Ok(())
	})
	.unwrap();
	let pause_ix = Instruction::new_with_bytes(
		program_id(),
		&pause_data,
		vec![
			AccountMeta::new_readonly(bootstrap_authority(), true),
			AccountMeta::new(config_pda().0, false),
		],
	);
	pool.world
		.run(&pool.mollusk, &pause_ix, &[Check::success()]);

	let note = prover::note_secrets(
		0x999,
		100_000_000,
		ark_bn254::Fr::from(0_u64),
		key(6).to_bytes(),
	);
	let mut dep_data = vec![0_u8; DepositIx::SIZE];
	DepositIx::initialize(&mut dep_data, |ix| {
		ix.amount.set(note.amount);
		ix.commitment = note.note_id_bytes();
		ix.view_pubkey = note.view_pubkey;
		ix.envelope_len = 0;
		ix.envelope = [0_u8; MAX_ENVELOPE_BYTES];
		ix.shares = [0_u8; SHARE_BLOB_BYTES];
		Ok(())
	})
	.unwrap();
	let dep_ix = Instruction::new_with_bytes(
		program_id(),
		&dep_data,
		vec![
			AccountMeta::new(key(2), true),
			AccountMeta::new(config_pda().0, false),
			AccountMeta::new(vault_pda().0, false),
			AccountMeta::new(tree_pda().0, false),
			AccountMeta::new_readonly(system_program::id(), false),
		],
	);
	pool.world.run(
		&pool.mollusk,
		&dep_ix,
		&[Check::err(ProgramError::Custom(
			PrivacyPoolError::PoolPaused as u32,
		))],
	);

	// Verify a 128-byte compressed Custom Ring ZK policy proof on VK_SLOT_POLICY.
	let id_secret = ark_bn254::Fr::from(12345_u64);
	let salt = ark_bn254::Fr::from(67890_u64);
	let policy_hash = poseidon2(&prover::fr_to_le(&id_secret), &prover::fr_to_le(&salt)).unwrap();
	let pcircuit = prover::PolicyCircuit {
		policy_hash: prover::fr_from_le(&policy_hash).unwrap(),
		identity_secret: Some(id_secret),
		policy_salt: Some(salt),
	};
	let proof = prover::prove_policy(&pool.policy_pk, pcircuit, 0x5555);
	let wire = prover::serialize_proof_compressed(&proof);

	let mut vp_data = vec![0_u8; VerifyPolicyIx::SIZE];
	VerifyPolicyIx::initialize(&mut vp_data, |ix| {
		ix.policy_hash = policy_hash;
		ix.proof_a = wire.a;
		ix.proof_b = wire.b;
		ix.proof_c = wire.c;
		Ok(())
	})
	.unwrap();
	let vp_ix = Instruction::new_with_bytes(
		program_id(),
		&vp_data,
		vec![AccountMeta::new_readonly(vkey_pda(VK_SLOT_POLICY).0, false)],
	);
	pool.world.run(&pool.mollusk, &vp_ix, &[Check::success()]);
}
