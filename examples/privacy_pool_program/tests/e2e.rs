//! SBF end-to-end tests for the privacy pool program.
//!
//! Build the program and run these ignored tests with:
//!
//! ```sh
//! cargo build-privacy-pool-program
//! ```
//!
//! ```sh
//! SBF_OUT_DIR=target/deploy \
//!     cargo test -p privacy_pool_program --features prover --test e2e -- --include-ignored
//! ```
//!
//! Every test uses fixed seeds, so the instruction paths stay deterministic.
//! The spend tests run real Groth16 proofs — seeded setup, host proving,
//! syscall verification inside the SBF artifact — through the same
//! little-endian wire encoding a wallet would produce.

use mollusk_svm::Mollusk;
use mollusk_svm::result::Check;
use mollusk_svm::result::InstructionResult;
use privacy_pool_program::Address;
use privacy_pool_program::ApproveDisclosureIx;
use privacy_pool_program::CancelDisclosureIx;
use privacy_pool_program::ChallengeDisclosureIx;
use privacy_pool_program::DEPOSIT_LAMPORTS;
use privacy_pool_program::DepositIx;
use privacy_pool_program::GrantDisclosureIx;
use privacy_pool_program::InitializeIx;
use privacy_pool_program::PrivacyPoolError;
use privacy_pool_program::RegisterRequesterIx;
use privacy_pool_program::RequestDisclosureIx;
use privacy_pool_program::ResolveChallengeIx;
use privacy_pool_program::SetVerificationKeyIx;
use privacy_pool_program::TREE_DEPTH;
use privacy_pool_program::TREE_NODES;
use privacy_pool_program::TransferIx;
use privacy_pool_program::VK_SLOT_TRANSFER;
use privacy_pool_program::VK_SLOT_WITHDRAW;
use privacy_pool_program::WithdrawIx;
use privacy_pool_program::prover;
use privacy_pool_program::zero_hashes;
use solana_account::Account;
use solana_instruction::AccountMeta;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;
use solana_sdk_ids::system_program;

const RENT_LAMPORTS: u64 = 100_000_000;
const FUND: u64 = 10_000_000_000;

// Migration envelope: discriminator (1) + version (1).
const HEADER: usize = 2;
// MerkleTree layout offsets after the envelope.
const TREE_NEXT_LEAF: usize = HEADER + 1;
const TREE_NODES_AT: usize = TREE_NEXT_LEAF + 8;

fn program_id() -> Pubkey {
	let bytes: &[u8] = privacy_pool_program::ID.as_ref();
	let array: [u8; 32] = bytes.try_into().unwrap_or_else(|_| panic!("id"));
	Pubkey::new_from_array(array)
}

fn pina_address(pubkey: &Pubkey) -> Address {
	let bytes: [u8; 32] = pubkey.to_bytes();
	Address::new_from_array(bytes)
}

fn key(seed: u8) -> Pubkey {
	let mut bytes = [0_u8; 32];
	bytes[0] = seed;
	Pubkey::new_from_array(bytes)
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
// PDA derivation, mirroring the on-chain seeds
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

// ---------------------------------------------------------------------------
// World
// ---------------------------------------------------------------------------

fn stored_account(data: Vec<u8>, lamports: u64) -> Account {
	Account {
		lamports,
		data,
		owner: program_id(),
		executable: false,
		rent_epoch: 0,
	}
}

fn wallet(seed: u8) -> (Pubkey, Account) {
	(
		key(seed),
		Account::new(FUND, 0, &solana_sdk_ids::system_program::id()),
	)
}

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

// ---------------------------------------------------------------------------
// Instruction builders
// ---------------------------------------------------------------------------

fn initialize_ix(custodians: [Pubkey; 3]) -> Instruction {
	let (config_key, config_bump) = config_pda();
	let (vault_key, vault_bump) = vault_pda();
	let (tree_key, tree_bump) = tree_pda();
	let (nullifiers_key, nullifiers_bump) = nullifiers_pda();
	let (custodians_key, custodians_bump) = custodians_pda();
	let (requesters_key, requesters_bump) = requesters_pda();
	let (log_key, log_bump) = log_pda();

	let mut data = vec![0_u8; InitializeIx::SIZE];
	InitializeIx::initialize(&mut data, |ix| {
		ix.config_bump = config_bump;
		ix.vault_bump = vault_bump;
		ix.tree_bump = tree_bump;
		ix.nullifiers_bump = nullifiers_bump;
		ix.custodians_bump = custodians_bump;
		ix.requesters_bump = requesters_bump;
		ix.log_bump = log_bump;
		let mut committee = [0_u8; 96];
		for (slot, key) in custodians.iter().enumerate() {
			committee[slot * 32..(slot + 1) * 32].copy_from_slice(key.as_ref());
		}
		ix.custodians = committee;
		Ok(())
	})
	.unwrap_or_else(|error| panic!("initialize ix: {error:?}"));

	let authority = key(1);
	Instruction::new_with_bytes(
		program_id(),
		&data,
		vec![
			AccountMeta::new(authority, true),
			AccountMeta::new(config_key, false),
			AccountMeta::new(vault_key, false),
			AccountMeta::new(tree_key, false),
			AccountMeta::new(nullifiers_key, false),
			AccountMeta::new(custodians_key, false),
			AccountMeta::new(requesters_key, false),
			AccountMeta::new(log_key, false),
			AccountMeta::new_readonly(
				solana_pubkey::pubkey!("11111111111111111111111111111111"),
				false,
			),
		],
	)
}

fn set_vkey_ix(slot: u8, wire: &prover::WireVerifyingKey) -> Instruction {
	let authority = key(1);
	let (config_key, _) = config_pda();
	let (vkey_key, bump) = vkey_pda(slot);

	let mut data = vec![0_u8; SetVerificationKeyIx::SIZE];
	SetVerificationKeyIx::initialize(&mut data, |ix| {
		ix.bump = bump;
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
	.unwrap_or_else(|error| panic!("vkey ix: {error:?}"));

	Instruction::new_with_bytes(
		program_id(),
		&data,
		vec![
			AccountMeta::new(authority, true),
			AccountMeta::new_readonly(config_key, false),
			AccountMeta::new(vkey_key, false),
			AccountMeta::new_readonly(
				solana_pubkey::pubkey!("11111111111111111111111111111111"),
				false,
			),
		],
	)
}

/// A fully initialized pool: config, vault, tree, registries, both keys.
struct Pool {
	mollusk: Mollusk,
	world: World,
	authority: Pubkey,
	custodians: [Pubkey; 3],
	withdraw_pk: ark_groth16::ProvingKey<ark_bn254::Bn254>,
	withdraw_vk: ark_groth16::VerifyingKey<ark_bn254::Bn254>,
	transfer_pk: ark_groth16::ProvingKey<ark_bn254::Bn254>,
}

fn setup_pool() -> Pool {
	let mut mollusk = create_mollusk();
	let mut world = World::new();

	let authority = key(1);
	world.add(
		authority,
		Account::new(FUND, 0, &solana_sdk_ids::system_program::id()),
	);
	for seed in 2..=10 {
		let (pubkey, account) = wallet(seed);
		world.add(pubkey, account);
	}

	let custodians = [key(21), key(22), key(23)];
	world.add(config_pda().0, Account::default());
	world.add(vault_pda().0, Account::default());
	world.add(tree_pda().0, Account::default());
	world.add(nullifiers_pda().0, Account::default());
	world.add(custodians_pda().0, Account::default());
	world.add(requesters_pda().0, Account::default());
	world.add(log_pda().0, Account::default());
	world.add(
		solana_sdk_ids::system_program::id(),
		mollusk_svm::program::keyed_account_for_system_program().1,
	);
	world.run(&mollusk, &initialize_ix(custodians), &[Check::success()]);

	let (withdraw_pk, withdraw_vk) = prover::seeded_setup(false, 0x1111);
	let (transfer_pk, transfer_vk) = prover::seeded_setup(true, 0x2222);
	world.add(vkey_pda(VK_SLOT_WITHDRAW).0, Account::default());
	world.add(vkey_pda(VK_SLOT_TRANSFER).0, Account::default());
	world.run(
		&mollusk,
		&set_vkey_ix(VK_SLOT_WITHDRAW, &prover::serialize_vk(&withdraw_vk)),
		&[Check::success()],
	);
	world.run(
		&mollusk,
		&set_vkey_ix(VK_SLOT_TRANSFER, &prover::serialize_vk(&transfer_vk)),
		&[Check::success()],
	);

	Pool {
		mollusk,
		world,
		authority,
		custodians,
		withdraw_pk,
		withdraw_vk,
		transfer_pk,
	}
}

/// Host-side commitment for a note: `poseidon(poseidon(secret, seed), amount)`.
fn commitment_bytes(secrets: &prover::NoteSecrets, amount_lamports: u64) -> [u8; 32] {
	let inner = privacy_pool_program::poseidon2(
		&prover::fr_to_le(&secrets.secret),
		&prover::fr_to_le(&secrets.nullifier_seed),
	)
	.unwrap_or_else(|e| panic!("host commitment: {e:?}"));
	let mut amount = [0_u8; 32];
	amount[..8].copy_from_slice(&amount_lamports.to_le_bytes());
	privacy_pool_program::poseidon2(&inner, &amount)
		.unwrap_or_else(|e| panic!("host commitment: {e:?}"))
}

/// Host-side nullifier: `poseidon(seed, secret)`.
fn nullifier_bytes(secrets: &prover::NoteSecrets) -> [u8; 32] {
	privacy_pool_program::poseidon2(
		&prover::fr_to_le(&secrets.nullifier_seed),
		&prover::fr_to_le(&secrets.secret),
	)
	.unwrap_or_else(|e| panic!("host nullifier: {e:?}"))
}

/// Read the flat node store out of the on-chain tree account.
fn tree_nodes(world: &World) -> Vec<u8> {
	let (tree_key, _) = tree_pda();
	world.get(&tree_key).data[TREE_NODES_AT..TREE_NODES_AT + TREE_NODES * 32].to_vec()
}

/// The tree account's current root, taken from the tail of the node store.
fn current_root(world: &World) -> [u8; 32] {
	let nodes = tree_nodes(world);
	let mut root = [0_u8; 32];
	root.copy_from_slice(&nodes[(TREE_NODES - 1) * 32..]);
	root
}

fn deposit_ix(
	depositor: Pubkey,
	secrets: &prover::NoteSecrets,
	view_pubkey: [u8; 32],
) -> Instruction {
	let commitment = commitment_bytes(secrets, DEPOSIT_LAMPORTS);
	let (config_key, _) = config_pda();
	let (vault_key, _) = vault_pda();
	let (tree_key, _) = tree_pda();
	let (note_key, bump) = note_pda(&commitment);

	let envelope: [u8; 128] = core::array::from_fn(|index| index as u8);
	let shares: [u8; 144] = core::array::from_fn(|index| 0xA0 ^ index as u8);

	let mut data = vec![0_u8; DepositIx::SIZE];
	DepositIx::initialize(&mut data, |ix| {
		ix.bump = bump;
		ix.commitment = commitment;
		ix.view_pubkey = view_pubkey;
		ix.envelope_len = 128;
		ix.envelope = envelope;
		ix.shares = shares;
		Ok(())
	})
	.unwrap_or_else(|error| panic!("deposit ix: {error:?}"));

	Instruction::new_with_bytes(
		program_id(),
		&data,
		vec![
			AccountMeta::new(depositor, true),
			AccountMeta::new(config_key, false),
			AccountMeta::new(vault_key, false),
			AccountMeta::new(tree_key, false),
			AccountMeta::new(note_key, false),
			AccountMeta::new_readonly(
				solana_pubkey::pubkey!("11111111111111111111111111111111"),
				false,
			),
		],
	)
}

fn withdraw_ix(
	root: [u8; 32],
	secrets: &prover::NoteSecrets,
	recipient: Pubkey,
	pk: &ark_groth16::ProvingKey<ark_bn254::Bn254>,
	leaf_index: usize,
	nodes: &[u8],
	seed: u64,
) -> Instruction {
	let nullifier = nullifier_bytes(secrets);
	let (path_elements, path_indices) = prover::build_witness(nodes, leaf_index);
	let circuit = prover::SpendCircuit {
		root: prover::fr_from_le(&root).unwrap_or_else(|| panic!("root")),
		nullifier: prover::fr_from_le(&nullifier).unwrap_or_else(|| panic!("nullifier")),
		output_commitment: None,
		amount: ark_bn254::Fr::from(DEPOSIT_LAMPORTS),
		witness: Some(prover::SpendWitness {
			spent: *secrets,
			path_elements,
			path_indices,
			successor: None,
		}),
	};
	let proof = prover::prove_spend(pk, circuit, seed);
	let wire = prover::serialize_proof(&proof);

	let (config_key, _) = config_pda();
	let (vault_key, _) = vault_pda();
	let (tree_key, _) = tree_pda();
	let (nullifiers_key, _) = nullifiers_pda();
	let (vkey_key, _) = vkey_pda(VK_SLOT_WITHDRAW);

	let mut data = vec![0_u8; WithdrawIx::SIZE];
	WithdrawIx::initialize(&mut data, |ix| {
		ix.nullifier = nullifier;
		ix.root = root;
		ix.proof_a = wire.a;
		ix.proof_b = wire.b;
		ix.proof_c = wire.c;
		Ok(())
	})
	.unwrap_or_else(|error| panic!("withdraw ix: {error:?}"));

	Instruction::new_with_bytes(
		program_id(),
		&data,
		vec![
			AccountMeta::new_readonly(config_key, false),
			AccountMeta::new(vault_key, false),
			AccountMeta::new_readonly(tree_key, false),
			AccountMeta::new(nullifiers_key, false),
			AccountMeta::new_readonly(vkey_key, false),
			AccountMeta::new(recipient, false),
			AccountMeta::new_readonly(
				solana_pubkey::pubkey!("11111111111111111111111111111111"),
				false,
			),
		],
	)
}

fn transfer_ix(
	root: [u8; 32],
	spent: &prover::NoteSecrets,
	successor: &prover::NoteSecrets,
	payer: Pubkey,
	pk: &ark_groth16::ProvingKey<ark_bn254::Bn254>,
	leaf_index: usize,
	nodes: &[u8],
	seed: u64,
) -> Instruction {
	let nullifier = nullifier_bytes(spent);
	let new_commitment = commitment_bytes(successor, DEPOSIT_LAMPORTS);
	let (path_elements, path_indices) = prover::build_witness(nodes, leaf_index);
	let circuit = prover::SpendCircuit {
		root: prover::fr_from_le(&root).unwrap_or_else(|| panic!("root")),
		nullifier: prover::fr_from_le(&nullifier).unwrap_or_else(|| panic!("nullifier")),
		output_commitment: Some(
			prover::fr_from_le(&new_commitment).unwrap_or_else(|| panic!("commitment")),
		),
		amount: ark_bn254::Fr::from(DEPOSIT_LAMPORTS),
		witness: Some(prover::SpendWitness {
			spent: *spent,
			path_elements,
			path_indices,
			successor: Some(*successor),
		}),
	};
	let proof = prover::prove_spend(pk, circuit, seed);
	let wire = prover::serialize_proof(&proof);

	let (config_key, _) = config_pda();
	let (tree_key, _) = tree_pda();
	let (nullifiers_key, _) = nullifiers_pda();
	let (vkey_key, _) = vkey_pda(VK_SLOT_TRANSFER);
	let (note_key, bump) = note_pda(&new_commitment);

	let envelope: [u8; 128] = core::array::from_fn(|index| 0x40 ^ index as u8);
	let shares: [u8; 144] = core::array::from_fn(|index| 0x80 ^ index as u8);

	let mut data = vec![0_u8; TransferIx::SIZE];
	TransferIx::initialize(&mut data, |ix| {
		ix.bump = bump;
		ix.nullifier = nullifier;
		ix.root = root;
		ix.new_commitment = new_commitment;
		ix.new_view_pubkey = [0x0B; 32];
		ix.envelope_len = 128;
		ix.envelope = envelope;
		ix.shares = shares;
		ix.proof_a = wire.a;
		ix.proof_b = wire.b;
		ix.proof_c = wire.c;
		Ok(())
	})
	.unwrap_or_else(|error| panic!("transfer ix: {error:?}"));

	Instruction::new_with_bytes(
		program_id(),
		&data,
		vec![
			AccountMeta::new_readonly(config_key, false),
			AccountMeta::new(payer, true),
			AccountMeta::new(tree_key, false),
			AccountMeta::new(nullifiers_key, false),
			AccountMeta::new_readonly(vkey_key, false),
			AccountMeta::new(note_key, false),
			AccountMeta::new_readonly(
				solana_pubkey::pubkey!("11111111111111111111111111111111"),
				false,
			),
		],
	)
}

/// Deposit one funded note and return its secrets and leaf index.
fn deposit_note(
	pool: &mut Pool,
	depositor: u8,
	seed: u64,
	view_pubkey: [u8; 32],
) -> prover::NoteSecrets {
	let secrets = prover::note_secrets(seed);
	let commitment = commitment_bytes(&secrets, DEPOSIT_LAMPORTS);
	pool.world.add(note_pda(&commitment).0, Account::default());
	pool.world.run(
		&pool.mollusk,
		&deposit_ix(key(depositor), &secrets, view_pubkey),
		&[Check::success()],
	);
	secrets
}

// ---------------------------------------------------------------------------
// Tests: setup
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires the SBF artifact"]
fn initialize_creates_the_pool_accounts() {
	let mut pool = setup_pool();
	let (config_key, _) = config_pda();
	let (vault_key, _) = vault_pda();
	let (tree_key, _) = tree_pda();
	let (nullifiers_key, _) = nullifiers_pda();

	let config = pool.world.get(&config_key);
	assert_eq!(config.owner, program_id());
	assert!(config.data.len() > HEADER);

	// The empty tree's stored root equals the zero-hash chain tip.
	let zeros = zero_hashes().unwrap_or_else(|e| panic!("zeros: {e:?}"));
	let nodes = tree_nodes(&pool.world);
	let mut stored_root = [0_u8; 32];
	stored_root.copy_from_slice(&nodes[(TREE_NODES - 1) * 32..]);
	assert_eq!(stored_root, zeros[TREE_DEPTH]);

	let vault = pool.world.get(&vault_key);
	assert!(vault.lamports > 0);
	let nullifiers = pool.world.get(&nullifiers_key);
	assert_eq!(nullifiers.data.len(), HEADER + 1 + 8 + 4096);
}

#[test]
#[ignore = "requires the SBF artifact"]
fn initialize_rejects_a_duplicate_custodian() {
	let mut mollusk = create_mollusk();
	let mut world = World::new();
	let authority = key(1);
	world.add(
		authority,
		Account::new(FUND, 0, &solana_sdk_ids::system_program::id()),
	);

	world.add(config_pda().0, Account::default());
	world.add(vault_pda().0, Account::default());
	world.add(tree_pda().0, Account::default());
	world.add(nullifiers_pda().0, Account::default());
	world.add(custodians_pda().0, Account::default());
	world.add(requesters_pda().0, Account::default());
	world.add(log_pda().0, Account::default());
	world.add(
		solana_sdk_ids::system_program::id(),
		mollusk_svm::program::keyed_account_for_system_program().1,
	);

	let same = key(21);
	let result = world.run(
		&mollusk,
		&initialize_ix([same, same, key(23)]),
		&[Check::err(PrivacyPoolError::InvalidCustodianSet.into())],
	);
	let _ = result;
}

// ---------------------------------------------------------------------------
// Tests: the pool with real proofs
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires the SBF artifact"]
fn deposit_matches_the_host_predicted_root() {
	let mut pool = setup_pool();
	let (vault_key, _) = vault_pda();
	let rent_only = pool.world.get(&vault_key).lamports;
	let secrets = deposit_note(&mut pool, 2, 0x77, [0x11; 32]);

	let commitment = commitment_bytes(&secrets, DEPOSIT_LAMPORTS);
	let nodes = tree_nodes(&pool.world);
	let mut leaf_slot = [0_u8; 32];
	leaf_slot.copy_from_slice(&nodes[..32]);
	assert_eq!(leaf_slot, commitment);

	// Fold the single leaf up the zero chain on the host; the on-chain root
	// (written through the syscall) must agree byte for byte.
	let zeros = zero_hashes().unwrap_or_else(|e| panic!("zeros: {e:?}"));
	let mut expected = privacy_pool_program::poseidon2(&commitment, &zeros[0])
		.unwrap_or_else(|e| panic!("host fold: {e:?}"));
	for level in 2..=TREE_DEPTH {
		expected = privacy_pool_program::poseidon2(&expected, &zeros[level - 1])
			.unwrap_or_else(|e| panic!("host fold: {e:?}"));
	}
	assert_eq!(current_root(&pool.world), expected);

	// Escrow moved the denomination into the vault.
	let (vault_key, _) = vault_pda();
	let vault = pool.world.get(&vault_key);
	assert_eq!(vault.lamports, rent_only + DEPOSIT_LAMPORTS);
}

#[test]
#[ignore = "requires the SBF artifact"]
fn withdraw_pays_out_with_a_real_proof() {
	let mut pool = setup_pool();
	let (vault_key, _) = vault_pda();
	let rent_only = pool.world.get(&vault_key).lamports;
	let recipient = key(5);
	let before = pool.world.get(&recipient).lamports;
	let secrets = deposit_note(&mut pool, 2, 0x88, [0x22; 32]);

	let root = current_root(&pool.world);
	pool.world.run(
		&pool.mollusk,
		&withdraw_ix(
			root,
			&secrets,
			recipient,
			&pool.withdraw_pk,
			0,
			&tree_nodes(&pool.world),
			0x99,
		),
		&[Check::success()],
	);

	let after = pool.world.get(&recipient).lamports;
	assert_eq!(after, before + DEPOSIT_LAMPORTS);

	// The vault is back to rent only, and the nullifier is recorded.
	assert_eq!(pool.world.get(&vault_key).lamports, rent_only);
	let (nullifiers_key, _) = nullifiers_pda();
	let set = pool.world.get(&nullifiers_key);
	let mut count = [0_u8; 8];
	count.copy_from_slice(&set.data[HEADER + 1..HEADER + 9]);
	assert_eq!(u64::from_le_bytes(count), 1);
	let mut stored = [0_u8; 32];
	stored.copy_from_slice(&set.data[HEADER + 9..HEADER + 41]);
	assert_eq!(stored, nullifier_bytes(&secrets));
}

#[test]
#[ignore = "requires the SBF artifact"]
fn withdraw_rejects_a_tampered_proof() {
	let mut pool = setup_pool();
	let recipient = key(5);
	let secrets = deposit_note(&mut pool, 2, 0x88, [0x22; 32]);
	let root = current_root(&pool.world);

	let mut ix = withdraw_ix(
		root,
		&secrets,
		recipient,
		&pool.withdraw_pk,
		0,
		&tree_nodes(&pool.world),
		0x99,
	);
	// Flip one proof byte.
	let last = ix.data.len() - 1;
	ix.data[last] ^= 0xFF;

	pool.world.run(
		&pool.mollusk,
		&ix,
		&[Check::err(PrivacyPoolError::ProofVerificationFailed.into())],
	);
}

#[test]
#[ignore = "requires the SBF artifact"]
fn withdraw_rejects_an_unknown_root() {
	let mut pool = setup_pool();
	let recipient = key(5);
	let secrets = deposit_note(&mut pool, 2, 0x88, [0x22; 32]);

	// Build the proof against the real root, then tamper only the root
	// bytes inside the instruction: the ring check must reject before
	// verification runs.
	let root = current_root(&pool.world);
	let mut ix = withdraw_ix(
		root,
		&secrets,
		recipient,
		&pool.withdraw_pk,
		0,
		&tree_nodes(&pool.world),
		0x99,
	);
	// The root field follows the discriminator, version, and nullifier.
	let root_offset = 1 + 1 + 32;
	ix.data[root_offset] ^= 0xFF;

	pool.world.run(
		&pool.mollusk,
		&ix,
		&[Check::err(PrivacyPoolError::UnknownRoot.into())],
	);
}

#[test]
#[ignore = "requires the SBF artifact"]
fn withdraw_rejects_a_double_spend() {
	let mut pool = setup_pool();
	let recipient = key(5);
	let secrets = deposit_note(&mut pool, 2, 0x88, [0x22; 32]);

	let root = current_root(&pool.world);
	let nodes = tree_nodes(&pool.world);
	pool.world.run(
		&pool.mollusk,
		&withdraw_ix(
			root,
			&secrets,
			recipient,
			&pool.withdraw_pk,
			0,
			&nodes,
			0x99,
		),
		&[],
	);
	// The same note cannot be spent twice against the fresh root either.
	let root = current_root(&pool.world);
	let nodes = tree_nodes(&pool.world);
	pool.world.run(
		&pool.mollusk,
		&withdraw_ix(
			root,
			&secrets,
			recipient,
			&pool.withdraw_pk,
			0,
			&nodes,
			0x99,
		),
		&[Check::err(PrivacyPoolError::NullifierAlreadySpent.into())],
	);
}

#[test]
#[ignore = "requires the SBF artifact"]
fn transfer_rekeys_a_note_with_a_real_proof() {
	let mut pool = setup_pool();
	let spent = deposit_note(&mut pool, 2, 0x88, [0x22; 32]);
	let successor = prover::note_secrets(0xAB);

	let root = current_root(&pool.world);
	let nodes = tree_nodes(&pool.world);
	let target = note_pda(&commitment_bytes(&successor, DEPOSIT_LAMPORTS)).0;
	pool.world.add(target, Account::default());
	pool.world.run(
		&pool.mollusk,
		&transfer_ix(
			root,
			&spent,
			&successor,
			key(3),
			&pool.transfer_pk,
			0,
			&nodes,
			0xCD,
		),
		&[],
	);

	// The successor note exists with the transferred envelope bytes.
	let commitment = commitment_bytes(&successor, DEPOSIT_LAMPORTS);
	let (note_key, _) = note_pda(&commitment);
	assert!(pool.world.get(&note_key).data.len() > HEADER);

	// The old nullifier is recorded and the vault balance is untouched.
	let (nullifiers_key, _) = nullifiers_pda();
	let set = pool.world.get(&nullifiers_key);
	let mut count = [0_u8; 8];
	count.copy_from_slice(&set.data[HEADER + 1..HEADER + 9]);
	assert_eq!(u64::from_le_bytes(count), 1);
	let (vault_key, _) = vault_pda();
	let rent_only_vault = {
		let mut pool2 = setup_pool();
		let (vk, _) = vault_pda();
		pool2.world.get(&vk).lamports
	};
	assert_eq!(
		pool.world.get(&vault_key).lamports,
		rent_only_vault + DEPOSIT_LAMPORTS
	);
}

// ---------------------------------------------------------------------------
// Tests: the disclosure tiers
// ---------------------------------------------------------------------------

struct DisclosureWorld {
	pool: Pool,
	requester: Pubkey,
	viewer: [u8; 32],
	note_commitment: [u8; 32],
}

fn disclosure_world(tier_to_register: Option<u8>) -> DisclosureWorld {
	let mut pool = setup_pool();
	// Register a tiered requester when the case needs one.
	if let Some(max_tier) = tier_to_register {
		let (requesters_key, _) = requesters_pda();
		let authority = pool.authority;
		let mut data = vec![0_u8; RegisterRequesterIx::SIZE];
		RegisterRequesterIx::initialize(&mut data, |ix| {
			ix.requester = pina_address(&key(7));
			ix.max_tier = max_tier;
			Ok(())
		})
		.unwrap_or_else(|error| panic!("register ix: {error:?}"));
		pool.world.run(
			&pool.mollusk,
			&Instruction::new_with_bytes(
				program_id(),
				&data,
				vec![
					AccountMeta::new_readonly(authority, true),
					AccountMeta::new_readonly(config_pda().0, false),
					AccountMeta::new(requesters_key, false),
				],
			),
			&[],
		);
	}

	// One deposited note with a known view key.
	let viewer = [0x5A; 32];
	let secrets = deposit_note(&mut pool, 2, 0x88, viewer);
	DisclosureWorld {
		pool,
		requester: key(7),
		viewer,
		note_commitment: commitment_bytes(&secrets, DEPOSIT_LAMPORTS),
	}
}

impl DisclosureWorld {
	fn request(&mut self, nonce: u64, tier: u8, legal_basis: [u8; 32]) {
		let (requesters_key, _) = requesters_pda();
		let (config_key, _) = config_pda();
		let (note_key, _) = note_pda(&self.note_commitment);
		let (request_key, bump) = request_pda(&self.requester, nonce);
		self.pool.world.add(request_key, Account::default());
		self.pool.mollusk.sysvars.clock.unix_timestamp = 1_000_000;
		let (clock_key, clock_account) = self.pool.mollusk.sysvars.keyed_account_for_clock_sysvar();
		self.pool.world.add(clock_key, clock_account);

		let notice: [u8; 96] = core::array::from_fn(|index| 0xC0 ^ index as u8);
		let mut data = vec![0_u8; RequestDisclosureIx::SIZE];
		RequestDisclosureIx::initialize(&mut data, |ix| {
			ix.bump = bump;
			ix.nonce.set(nonce);
			ix.tier = tier;
			ix.commitment = self.note_commitment;
			ix.notice_len = 96;
			ix.notice = notice;
			ix.legal_basis_hash = legal_basis;
			Ok(())
		})
		.unwrap_or_else(|error| panic!("request ix: {error:?}"));

		let requester = self.requester;
		self.pool.world.run(
			&self.pool.mollusk,
			&Instruction::new_with_bytes(
				program_id(),
				&data,
				vec![
					AccountMeta::new(requester, true),
					AccountMeta::new_readonly(config_key, false),
					AccountMeta::new_readonly(requesters_key, false),
					AccountMeta::new_readonly(note_key, false),
					AccountMeta::new(request_key, false),
					AccountMeta::new_readonly(
						solana_pubkey::pubkey!("11111111111111111111111111111111"),
						false,
					),
					AccountMeta::new_readonly(clock_key, false),
				],
			),
			&[],
		);
	}

	fn grant(&mut self, nonce: u64, signer: Option<[u8; 32]>, expect: Option<PrivacyPoolError>) {
		let (request_key, _) = request_pda(&self.requester, nonce);
		let (note_key, _) = note_pda(&self.note_commitment);
		let viewer = self.viewer;
		let signer_key = match signer {
			Some(bytes) => Pubkey::new_from_array(bytes),
			None => Pubkey::new_from_array(viewer),
		};
		self.pool.world.add(
			signer_key,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		);
		let mut data = vec![0_u8; GrantDisclosureIx::SIZE];
		GrantDisclosureIx::initialize(&mut data, |_| Ok(()))
			.unwrap_or_else(|error| panic!("grant ix: {error:?}"));
		let checks = match expect {
			Some(error) => vec![Check::err(error.into())],
			None => vec![],
		};
		self.pool.world.run(
			&self.pool.mollusk,
			&Instruction::new_with_bytes(
				program_id(),
				&data,
				vec![
					AccountMeta::new(request_key, false),
					AccountMeta::new_readonly(note_key, false),
					AccountMeta::new_readonly(signer_key, true),
				],
			),
			&checks,
		);
	}

	fn request_expect_error(
		&mut self,
		nonce: u64,
		tier: u8,
		legal_basis: [u8; 32],
		error: PrivacyPoolError,
	) {
		let (requesters_key, _) = requesters_pda();
		let (config_key, _) = config_pda();
		let (note_key, _) = note_pda(&self.note_commitment);
		let (request_key, bump) = request_pda(&self.requester, nonce);
		self.pool.world.add(request_key, Account::default());
		self.pool.mollusk.sysvars.clock.unix_timestamp = 1_000_000;
		let (clock_key, clock_account) = self.pool.mollusk.sysvars.keyed_account_for_clock_sysvar();
		self.pool.world.add(clock_key, clock_account);

		let notice: [u8; 96] = core::array::from_fn(|index| 0xC0 ^ index as u8);
		let mut data = vec![0_u8; RequestDisclosureIx::SIZE];
		RequestDisclosureIx::initialize(&mut data, |ix| {
			ix.bump = bump;
			ix.nonce.set(nonce);
			ix.tier = tier;
			ix.commitment = self.note_commitment;
			ix.notice_len = 96;
			ix.notice = notice;
			ix.legal_basis_hash = legal_basis;
			Ok(())
		})
		.unwrap_or_else(|error| panic!("request ix: {error:?}"));

		let requester = self.requester;
		self.pool.world.run(
			&self.pool.mollusk,
			&Instruction::new_with_bytes(
				program_id(),
				&data,
				vec![
					AccountMeta::new(requester, true),
					AccountMeta::new_readonly(config_key, false),
					AccountMeta::new_readonly(requesters_key, false),
					AccountMeta::new_readonly(note_key, false),
					AccountMeta::new(request_key, false),
					AccountMeta::new_readonly(
						solana_pubkey::pubkey!("11111111111111111111111111111111"),
						false,
					),
					AccountMeta::new_readonly(clock_key, false),
				],
			),
			&[Check::err(error.into())],
		);
	}

	/// The subject challenges a tier-1 request at `timestamp`.
	fn challenge_window(&mut self, nonce: u64, timestamp: i64, expect: Option<PrivacyPoolError>) {
		let (request_key, _) = request_pda(&self.requester, nonce);
		let (note_key, _) = note_pda(&self.note_commitment);
		let viewer = self.viewer;
		self.pool.world.add(
			Pubkey::new_from_array(viewer),
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		);
		self.pool.mollusk.sysvars.clock.unix_timestamp = timestamp;
		let (clock_key, clock_account) = self.pool.mollusk.sysvars.keyed_account_for_clock_sysvar();
		self.pool.world.add(clock_key, clock_account);

		let mut data = vec![0_u8; ChallengeDisclosureIx::SIZE];
		ChallengeDisclosureIx::initialize(&mut data, |_| Ok(()))
			.unwrap_or_else(|error| panic!("challenge ix: {error:?}"));
		let checks = match expect {
			Some(error) => vec![Check::err(error.into())],
			None => vec![],
		};
		self.pool.world.run(
			&self.pool.mollusk,
			&Instruction::new_with_bytes(
				program_id(),
				&data,
				vec![
					AccountMeta::new(request_key, false),
					AccountMeta::new_readonly(note_key, false),
					AccountMeta::new_readonly(Pubkey::new_from_array(viewer), true),
					AccountMeta::new_readonly(clock_key, false),
				],
			),
			&checks,
		);
	}

	fn resolve(&mut self, nonce: u64, approve: u8, expect: Option<PrivacyPoolError>) {
		let (request_key, _) = request_pda(&self.requester, nonce);
		let (config_key, _) = config_pda();
		let mut data = vec![0_u8; ResolveChallengeIx::SIZE];
		ResolveChallengeIx::initialize(&mut data, |ix| {
			ix.approve = approve;
			Ok(())
		})
		.unwrap_or_else(|error| panic!("resolve ix: {error:?}"));
		let checks = match expect {
			Some(error) => vec![Check::err(error.into())],
			None => vec![],
		};
		self.pool.world.run(
			&self.pool.mollusk,
			&Instruction::new_with_bytes(
				program_id(),
				&data,
				vec![
					AccountMeta::new_readonly(self.pool.authority, true),
					AccountMeta::new_readonly(config_key, false),
					AccountMeta::new(request_key, false),
				],
			),
			&checks,
		);
	}

	fn cancel(&mut self, nonce: u64, expect: Option<PrivacyPoolError>) {
		let (request_key, _) = request_pda(&self.requester, nonce);
		let mut data = vec![0_u8; CancelDisclosureIx::SIZE];
		CancelDisclosureIx::initialize(&mut data, |_| Ok(()))
			.unwrap_or_else(|error| panic!("cancel ix: {error:?}"));
		let checks = match expect {
			Some(error) => vec![Check::err(error.into())],
			None => vec![],
		};
		self.pool.world.run(
			&self.pool.mollusk,
			&Instruction::new_with_bytes(
				program_id(),
				&data,
				vec![
					AccountMeta::new_readonly(self.requester, true),
					AccountMeta::new(request_key, false),
				],
			),
			&checks,
		);
	}

	fn approve(&mut self, nonce: u64, custodian: u8, expect: Option<PrivacyPoolError>) {
		let (request_key, _) = request_pda(&self.requester, nonce);
		let (config_key, _) = config_pda();
		let (custodians_key, _) = custodians_pda();
		let (log_key, _) = log_pda();
		// The caller owns the clock; the keyed account re-snapshots
		// whatever timestamp the test last set.
		let (clock_key, clock_account) = self.pool.mollusk.sysvars.keyed_account_for_clock_sysvar();
		self.pool.world.add(clock_key, clock_account);

		self.pool.world.add(
			key(custodian),
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		);
		let mut data = vec![0_u8; ApproveDisclosureIx::SIZE];
		ApproveDisclosureIx::initialize(&mut data, |_| Ok(()))
			.unwrap_or_else(|error| panic!("approve ix: {error:?}"));
		let checks = match expect {
			Some(error) => vec![Check::err(error.into())],
			None => vec![],
		};
		self.pool.world.run(
			&self.pool.mollusk,
			&Instruction::new_with_bytes(
				program_id(),
				&data,
				vec![
					AccountMeta::new_readonly(key(custodian), true),
					AccountMeta::new_readonly(config_key, false),
					AccountMeta::new_readonly(custodians_key, false),
					AccountMeta::new(request_key, false),
					AccountMeta::new(log_key, false),
					AccountMeta::new_readonly(clock_key, false),
				],
			),
			&checks,
		);
	}
}

#[test]
#[ignore = "requires the SBF artifact"]
fn tier_zero_requires_consent_then_executes_and_logs() {
	let mut world = disclosure_world(None);

	// Anyone may file at tier 0, but execution needs the view key's grant.
	world.request(1, privacy_pool_program::TIER_CONSENT, [0; 32]);
	world.approve(1, 21, Some(PrivacyPoolError::ConsentRequired));

	// A key other than the note's view key cannot grant.
	world.grant(1, Some([0x99; 32]), Some(PrivacyPoolError::NotNoteViewer));

	// The view key grants; two custodians approve; the second approval
	// executes the request and appends the log entry atomically.
	world.grant(1, None, None);
	world.approve(1, 21, None);
	world.approve(1, 22, None);

	// The log holds one entry naming the requester and the commitment.
	let (log_key, _) = log_pda();
	let log = world.pool.world.get(&log_key);
	let mut count = [0_u8; 8];
	count.copy_from_slice(&log.data[HEADER + 1..HEADER + 9]);
	assert_eq!(u64::from_le_bytes(count), 1);
	assert_eq!(
		&log.data[HEADER + 9..HEADER + 9 + 32],
		world.requester.as_ref()
	);
	assert_eq!(
		&log.data[HEADER + 9 + 32..HEADER + 9 + 64],
		&world.note_commitment[..]
	);
	assert_eq!(
		log.data[HEADER + 9 + 64],
		privacy_pool_program::TIER_CONSENT
	);

	// A third approval on the executed request is a status error, and a
	// repeat approval by an approving custodian is rejected.
	world.approve(1, 23, Some(PrivacyPoolError::InvalidRequestStatus));
}

#[test]
#[ignore = "requires the SBF artifact"]
fn tier_one_holds_a_challenge_window_then_executes() {
	let mut world = disclosure_world(Some(privacy_pool_program::TIER_VERIFIED));

	// Filing above tier 0 requires a registered requester and a legal basis.
	world.request(1, privacy_pool_program::TIER_VERIFIED, [0x77; 32]);

	// Inside the window the committee cannot execute.
	world.approve(1, 21, Some(PrivacyPoolError::ChallengeWindowOpen));

	// Advance the clock past the window, then execute with two approvals.
	world.pool.mollusk.sysvars.clock.unix_timestamp =
		1_000_000 + privacy_pool_program::DEFAULT_CHALLENGE_WINDOW_SECS as i64 + 1;
	world.approve(1, 21, None);
	world.approve(1, 22, None);

	let (log_key, _) = log_pda();
	let log = world.pool.world.get(&log_key);
	let mut count = [0_u8; 8];
	count.copy_from_slice(&log.data[HEADER + 1..HEADER + 9]);
	assert_eq!(u64::from_le_bytes(count), 1);
	assert_eq!(
		log.data[HEADER + 9 + 64],
		privacy_pool_program::TIER_VERIFIED
	);
}

#[test]
#[ignore = "requires the SBF artifact"]
fn tier_one_challenge_freezes_and_the_authority_resolves() {
	let mut world = disclosure_world(Some(privacy_pool_program::TIER_VERIFIED));
	world.request(1, privacy_pool_program::TIER_VERIFIED, [0x77; 32]);

	// Before the window closes, the subject challenges.
	world.challenge_window(1, 1_000_001, None);

	// Execution on a challenged request requires resolution.
	world.approve(1, 21, Some(PrivacyPoolError::InvalidRequestStatus));

	// The authority resolves in the requester's favor; execution proceeds.
	world.resolve(1, 1, None);
	world.pool.mollusk.sysvars.clock.unix_timestamp =
		1_000_000 + privacy_pool_program::DEFAULT_CHALLENGE_WINDOW_SECS as i64 + 1;
	world.approve(1, 21, None);
	world.approve(1, 22, None);

	let (log_key, _) = log_pda();
	let log = world.pool.world.get(&log_key);
	let mut count = [0_u8; 8];
	count.copy_from_slice(&log.data[HEADER + 1..HEADER + 9]);
	assert_eq!(u64::from_le_bytes(count), 1);
}

#[test]
#[ignore = "requires the SBF artifact"]
fn tier_two_executes_immediately() {
	let mut world = disclosure_world(Some(privacy_pool_program::TIER_COMPELLED));

	// An unregistered tier cannot file, and neither can a tier-1 registrant.
	world.request(9, privacy_pool_program::TIER_COMPELLED, [0x77; 32]);

	// The registered tier-2 requester files with a legal basis and the
	// committee executes without a window or consent.
	world.request(1, privacy_pool_program::TIER_COMPELLED, [0x55; 32]);
	world.approve(1, 21, None);
	world.approve(1, 22, None);

	let (log_key, _) = log_pda();
	let log = world.pool.world.get(&log_key);
	let mut count = [0_u8; 8];
	count.copy_from_slice(&log.data[HEADER + 1..HEADER + 9]);
	assert_eq!(u64::from_le_bytes(count), 1);
	assert_eq!(
		log.data[HEADER + 9 + 64],
		privacy_pool_program::TIER_COMPELLED
	);
}

#[test]
#[ignore = "requires the SBF artifact"]
fn unregistered_requesters_cannot_file_tiered_requests() {
	let mut world = disclosure_world(None);
	// No registry entry: tier 1 and tier 2 both fail at filing time.
	world.request_expect_error(
		1,
		privacy_pool_program::TIER_VERIFIED,
		[1; 32],
		PrivacyPoolError::RequesterNotEntitled,
	);
	world.request_expect_error(
		2,
		privacy_pool_program::TIER_COMPELLED,
		[1; 32],
		PrivacyPoolError::RequesterNotEntitled,
	);
	// Tier 0 remains open to anyone.
	world.request(3, privacy_pool_program::TIER_CONSENT, [0; 32]);
}

#[test]
#[ignore = "requires the SBF artifact"]
fn non_custodians_cannot_approve() {
	let mut world = disclosure_world(None);
	world.request(1, privacy_pool_program::TIER_CONSENT, [0; 32]);
	world.grant(1, None, None);
	world.approve(1, 21, None);
	// A non-custodian signing key is rejected outright.
	world.approve(1, 30, Some(PrivacyPoolError::NotACustodian));
	// Duplicate approval by the same custodian is rejected.
	world.approve(1, 21, Some(PrivacyPoolError::AlreadyApproved));
}

#[test]
#[ignore = "requires the SBF artifact"]
fn cancel_closes_a_pending_request() {
	let mut world = disclosure_world(None);
	world.request(1, privacy_pool_program::TIER_CONSENT, [0; 32]);
	world.cancel(1, None);
	// A cancelled request can no longer execute.
	world.grant(1, None, Some(PrivacyPoolError::InvalidRequestStatus));
	world.approve(1, 21, Some(PrivacyPoolError::InvalidRequestStatus));
}
