//! Compute unit (CU) benchmarks for the counter program.
//!
//! These tests execute the counter program instructions through `mollusk-svm`
//! and report the exact number of compute units consumed. Since mollusk is
//! deterministic, the CU numbers are fully reproducible across runs.
//!
//! ## Prerequisites
//!
//! The counter program must be compiled to an SBF binary before running these
//! tests:
//!
//! ```sh
//! cargo build --release --target bpfel-unknown-none -p counter_program \
//!     -Z build-std -F bpf-entrypoint
//! ```
//!
//! Then set `SBF_OUT_DIR` to the directory containing the `.so` file, or place
//! it in `tests/fixtures/`.
//!
//! ## Running
//!
//! ```sh
//! SBF_OUT_DIR=target/bpfel-unknown-none/release \
//!     cargo test -p counter_program --test cu_benchmarks -- --nocapture
//! ```

use counter_program::CounterInstruction;
use counter_program::CounterState;
use mollusk_svm::Mollusk;
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::result::Check;
use pina::PodU64;
use pina::bytemuck;
use solana_account::Account;
use solana_instruction::AccountMeta;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

fn program_id() -> Pubkey {
	let id = counter_program::ID;
	let bytes: &[u8] = id.as_ref();
	let array: [u8; 32] = bytes
		.try_into()
		.unwrap_or_else(|_| panic!("address must be 32 bytes"));
	Pubkey::new_from_array(array)
}

/// Try to create a mollusk instance for the counter program.
///
/// Returns `None` if the BPF binary cannot be found (e.g. the program has not
/// been compiled for SBF yet). This allows the tests to be skipped gracefully.
fn try_create_mollusk() -> Option<Mollusk> {
	// mollusk looks for the .so in SBF_OUT_DIR, BPF_OUT_DIR, tests/fixtures,
	// or CWD.
	let result = std::panic::catch_unwind(|| Mollusk::new(&program_id(), "counter_program"));
	result.ok()
}

/// Derive the counter PDA for a given authority.
fn derive_counter_pda(authority: &Pubkey) -> (Pubkey, u8) {
	Pubkey::find_program_address(&[b"counter", authority.as_ref()], &program_id())
}

/// Build the instruction data for Initialize.
fn initialize_ix_data(bump: u8) -> Vec<u8> {
	vec![CounterInstruction::Initialize as u8, bump]
}

/// Build the instruction data for Increment.
fn increment_ix_data() -> Vec<u8> {
	vec![CounterInstruction::Increment as u8]
}

/// Build a counter account with the given state for testing.
fn counter_account(bump: u8, count: u64, lamports: u64) -> Account {
	let state = CounterState::builder()
		.bump(bump)
		.count(PodU64::from_primitive(count))
		.build();
	let data = bytemuck::bytes_of(&state).to_vec();
	Account {
		lamports,
		data,
		owner: program_id(),
		executable: false,
		rent_epoch: 0,
	}
}

// ---------------------------------------------------------------------------
// CU Benchmark Tests
// ---------------------------------------------------------------------------

#[test]
fn benchmark_cu_initialize_counter() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!(
			"[SKIP] counter_program SBF binary not found. Build it first with `cargo build \
			 --release --target bpfel-unknown-none -p counter_program -Z build-std -F \
			 bpf-entrypoint`."
		);
		return;
	};

	let authority = Pubkey::new_unique();
	let (counter_pda, bump) = derive_counter_pda(&authority);

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&initialize_ix_data(bump),
		vec![
			AccountMeta::new(authority, true),
			AccountMeta::new(counter_pda, true),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);

	let authority_account = Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id());

	let accounts = vec![
		(authority, authority_account),
		(counter_pda, Account::default()),
		keyed_account_for_system_program(),
	];

	let result =
		mollusk.process_and_validate_instruction(&instruction, &accounts, &[Check::success()]);

	eprintln!(
		"[CU BENCHMARK] Initialize counter: {} compute units consumed",
		result.compute_units_consumed
	);
}

#[test]
fn benchmark_cu_increment_counter() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!(
			"[SKIP] counter_program SBF binary not found. Build it first with `cargo build \
			 --release --target bpfel-unknown-none -p counter_program -Z build-std -F \
			 bpf-entrypoint`."
		);
		return;
	};

	let authority = Pubkey::new_unique();
	let (counter_pda, bump) = derive_counter_pda(&authority);

	let lamports = mollusk
		.sysvars
		.rent
		.minimum_balance(size_of::<CounterState>());

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&increment_ix_data(),
		vec![
			AccountMeta::new_readonly(authority, true),
			AccountMeta::new(counter_pda, false),
		],
	);

	let authority_account = Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id());

	let accounts = vec![
		(authority, authority_account),
		(counter_pda, counter_account(bump, 0, lamports)),
	];

	let result =
		mollusk.process_and_validate_instruction(&instruction, &accounts, &[Check::success()]);

	eprintln!(
		"[CU BENCHMARK] Increment counter: {} compute units consumed",
		result.compute_units_consumed
	);
}

#[test]
fn benchmark_cu_increment_counter_at_max() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("[SKIP] counter_program SBF binary not found. Build it first.");
		return;
	};

	let authority = Pubkey::new_unique();
	let (counter_pda, bump) = derive_counter_pda(&authority);

	let lamports = mollusk
		.sysvars
		.rent
		.minimum_balance(size_of::<CounterState>());

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&increment_ix_data(),
		vec![
			AccountMeta::new_readonly(authority, true),
			AccountMeta::new(counter_pda, false),
		],
	);

	let authority_account = Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id());

	// Counter at a high value to test worst-case arithmetic path.
	let accounts = vec![
		(authority, authority_account),
		(counter_pda, counter_account(bump, u64::MAX - 1, lamports)),
	];

	let result =
		mollusk.process_and_validate_instruction(&instruction, &accounts, &[Check::success()]);

	eprintln!(
		"[CU BENCHMARK] Increment counter (near-max value): {} compute units consumed",
		result.compute_units_consumed
	);
}

#[test]
fn benchmark_cu_initialize_full_flow() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("[SKIP] counter_program SBF binary not found. Build it first.");
		return;
	};

	let authority = Pubkey::new_unique();
	let (counter_pda, bump) = derive_counter_pda(&authority);

	// Initialize
	let init_ix = Instruction::new_with_bytes(
		program_id(),
		&initialize_ix_data(bump),
		vec![
			AccountMeta::new(authority, true),
			AccountMeta::new(counter_pda, true),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);

	let authority_account = Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id());

	let init_accounts = vec![
		(authority, authority_account),
		(counter_pda, Account::default()),
		keyed_account_for_system_program(),
	];

	let init_result =
		mollusk.process_and_validate_instruction(&init_ix, &init_accounts, &[Check::success()]);

	eprintln!(
		"[CU BENCHMARK] Full flow - Initialize: {} CU",
		init_result.compute_units_consumed
	);

	// Now increment using the resulting accounts.
	let incr_ix = Instruction::new_with_bytes(
		program_id(),
		&increment_ix_data(),
		vec![
			AccountMeta::new_readonly(authority, true),
			AccountMeta::new(counter_pda, false),
		],
	);

	let authority_after = init_result
		.get_account(&authority)
		.cloned()
		.unwrap_or_else(|| panic!("authority not found in resulting accounts"));
	let counter_after = init_result
		.get_account(&counter_pda)
		.cloned()
		.unwrap_or_else(|| panic!("counter PDA not found in resulting accounts"));

	let incr_accounts = vec![(authority, authority_after), (counter_pda, counter_after)];

	let incr_result =
		mollusk.process_and_validate_instruction(&incr_ix, &incr_accounts, &[Check::success()]);

	eprintln!(
		"[CU BENCHMARK] Full flow - Increment: {} CU",
		incr_result.compute_units_consumed
	);
	eprintln!(
		"[CU BENCHMARK] Full flow - Total: {} CU",
		init_result.compute_units_consumed + incr_result.compute_units_consumed
	);
}

/// Print a summary of all CU measurements.
#[test]
fn benchmark_cu_summary() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("[SKIP] counter_program SBF binary not found. Build it first.");
		return;
	};

	let authority = Pubkey::new_unique();
	let (counter_pda, bump) = derive_counter_pda(&authority);
	let lamports = mollusk
		.sysvars
		.rent
		.minimum_balance(size_of::<CounterState>());

	let authority_account = Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id());

	// --- Initialize ---
	let init_ix = Instruction::new_with_bytes(
		program_id(),
		&initialize_ix_data(bump),
		vec![
			AccountMeta::new(authority, true),
			AccountMeta::new(counter_pda, true),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);
	let init_result = mollusk.process_and_validate_instruction(
		&init_ix,
		&[
			(authority, authority_account.clone()),
			(counter_pda, Account::default()),
			keyed_account_for_system_program(),
		],
		&[Check::success()],
	);

	// --- Increment ---
	let incr_ix = Instruction::new_with_bytes(
		program_id(),
		&increment_ix_data(),
		vec![
			AccountMeta::new_readonly(authority, true),
			AccountMeta::new(counter_pda, false),
		],
	);
	let incr_result = mollusk.process_and_validate_instruction(
		&incr_ix,
		&[
			(authority, authority_account.clone()),
			(counter_pda, counter_account(bump, 0, lamports)),
		],
		&[Check::success()],
	);

	// --- Increment (near-max) ---
	let incr_max_result = mollusk.process_and_validate_instruction(
		&incr_ix,
		&[
			(authority, authority_account),
			(counter_pda, counter_account(bump, u64::MAX - 1, lamports)),
		],
		&[Check::success()],
	);

	eprintln!();
	eprintln!("=== Counter Program CU Benchmark Summary ===");
	eprintln!();
	eprintln!(
		"  Initialize (PDA creation + account init):  {} CU",
		init_result.compute_units_consumed
	);
	eprintln!(
		"  Increment (validation + state mutation):    {} CU",
		incr_result.compute_units_consumed
	);
	eprintln!(
		"  Increment (near-max value):                 {} CU",
		incr_max_result.compute_units_consumed
	);
	eprintln!();
	eprintln!("  Note: Initialize includes CPI to system program for CreateAccount.");
	eprintln!("  Note: Increment includes PDA seed verification and checked arithmetic.");
	eprintln!();
}
