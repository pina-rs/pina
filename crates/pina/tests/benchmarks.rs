//! Performance benchmarks for core pina operations.
//!
//! These tests measure the native execution time of key pina operations that
//! contribute to on-chain compute unit consumption. While they do not directly
//! measure Solana CU (which requires a BPF VM), they provide reproducible
//! performance baselines for the most performance-sensitive code paths.
//!
//! ## Running
//!
//! ```sh
//! cargo test -p pina --test benchmarks -- --nocapture
//! ```

use std::hint::black_box;
use std::time::Instant;

use pina::AccountDeserialize;
use pina::Address;
use pina::HasDiscriminator;
use pina::IntoDiscriminator;
use pina::PodU64;
use pina::create_program_address;
use pina::try_find_program_address;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

const SYSTEM_ID: Address = pina::address!("11111111111111111111111111111111");
const ITERATIONS: u32 = 1_000;

/// Run a closure `ITERATIONS` times and return the total and average duration.
fn bench<F: FnMut()>(label: &str, mut f: F) {
	// Warm up
	for _ in 0..10 {
		f();
	}

	let start = Instant::now();
	for _ in 0..ITERATIONS {
		f();
	}
	let elapsed = start.elapsed();
	let avg_ns = elapsed.as_nanos() / u128::from(ITERATIONS);

	eprintln!("[BENCH] {label}: total={elapsed:?}, avg={avg_ns}ns ({ITERATIONS} iterations)");
}

// ---------------------------------------------------------------------------
// Discriminator enum for benchmarks
// ---------------------------------------------------------------------------

#[pina::discriminator(crate = ::pina)]
#[derive(Debug, PartialEq)]
pub enum BenchInstruction {
	Initialize = 0,
	Update = 1,
	Close = 2,
}

#[pina::discriminator(crate = ::pina, primitive = u16)]
#[derive(Debug, PartialEq)]
pub enum BenchInstruction16 {
	Initialize = 0,
	Update = 1,
	Close = 2,
}

#[pina::discriminator(crate = ::pina, primitive = u32)]
#[derive(Debug, PartialEq)]
pub enum BenchInstruction32 {
	Initialize = 0,
	Update = 1,
	Close = 2,
}

#[pina::discriminator(crate = ::pina)]
#[derive(Debug, PartialEq)]
pub enum BenchAccountType {
	TestState = 1,
}

#[pina::account(crate = ::pina, discriminator = BenchAccountType)]
pub struct TestState {
	pub bump: u8,
	pub count: PodU64,
	pub authority: Address,
}

// ---------------------------------------------------------------------------
// Discriminator matching benchmarks
// ---------------------------------------------------------------------------

#[test]
fn benchmark_discriminator_u8_from_bytes() {
	bench("discriminator u8 from_bytes", || {
		let data = black_box([2u8, 0, 0, 0]);
		let _ = black_box(u8::discriminator_from_bytes(&data));
	});
}

#[test]
fn benchmark_discriminator_u8_matches() {
	let data = [2u8, 0, 0, 0];
	bench("discriminator u8 matches", || {
		let _ = black_box(2u8.matches_discriminator(black_box(&data)));
	});
}

#[test]
fn benchmark_discriminator_enum_from_bytes() {
	bench("discriminator enum (u8) from_bytes", || {
		let data = black_box([1u8]);
		let _ = black_box(BenchInstruction::discriminator_from_bytes(&data));
	});
}

#[test]
fn benchmark_discriminator_u16_from_bytes() {
	bench("discriminator u16 from_bytes", || {
		let data = black_box([1u8, 0]);
		let _ = black_box(BenchInstruction16::discriminator_from_bytes(&data));
	});
}

#[test]
fn benchmark_discriminator_u32_from_bytes() {
	bench("discriminator u32 from_bytes", || {
		let data = black_box([1u8, 0, 0, 0]);
		let _ = black_box(BenchInstruction32::discriminator_from_bytes(&data));
	});
}

#[test]
fn benchmark_discriminator_write() {
	let mut bytes = [0u8; 4];
	bench("discriminator u32 write", || {
		let val: u32 = black_box(0xDEAD_BEEF);
		val.write_discriminator(black_box(&mut bytes));
	});
}

#[test]
fn benchmark_has_discriminator_matches() {
	bench("HasDiscriminator matches (TestState)", || {
		let data = black_box([BenchAccountType::TestState as u8]);
		let _ = black_box(TestState::matches_discriminator(&data));
	});
}

// ---------------------------------------------------------------------------
// PDA derivation benchmarks
// ---------------------------------------------------------------------------

#[test]
fn benchmark_pda_try_find_simple() {
	bench("PDA try_find_program_address (1 seed)", || {
		let _ = black_box(try_find_program_address(
			black_box(&[b"test-seed"]),
			black_box(&SYSTEM_ID),
		));
	});
}

#[test]
fn benchmark_pda_try_find_two_seeds() {
	let authority = [42u8; 32];
	bench("PDA try_find_program_address (2 seeds)", || {
		let _ = black_box(try_find_program_address(
			black_box(&[b"counter", &authority]),
			black_box(&SYSTEM_ID),
		));
	});
}

#[test]
fn benchmark_pda_try_find_three_seeds() {
	let authority = [42u8; 32];
	let seed = 12345u64.to_le_bytes();
	bench("PDA try_find_program_address (3 seeds)", || {
		let _ = black_box(try_find_program_address(
			black_box(&[b"escrow", &authority, &seed]),
			black_box(&SYSTEM_ID),
		));
	});
}

#[test]
fn benchmark_pda_create_program_address() {
	let (_, bump) = try_find_program_address(&[b"bench-pda"], &SYSTEM_ID)
		.unwrap_or_else(|| panic!("expected PDA"));
	let bump_seed = [bump];

	bench("PDA create_program_address (with bump)", || {
		let _ = black_box(create_program_address(
			black_box(&[b"bench-pda", &bump_seed]),
			black_box(&SYSTEM_ID),
		));
	});
}

#[test]
fn benchmark_pda_create_program_address_two_seeds() {
	let authority = [42u8; 32];
	let (_, bump) = try_find_program_address(&[b"counter", &authority], &SYSTEM_ID)
		.unwrap_or_else(|| panic!("expected PDA"));
	let bump_seed = [bump];

	bench("PDA create_program_address (2 seeds + bump)", || {
		let _ = black_box(create_program_address(
			black_box(&[b"counter", &authority, &bump_seed]),
			black_box(&SYSTEM_ID),
		));
	});
}

// ---------------------------------------------------------------------------
// Instruction parsing benchmarks
// ---------------------------------------------------------------------------

#[test]
fn benchmark_parse_instruction() {
	bench("parse_instruction (valid discriminator)", || {
		let data = black_box(&[1u8]);
		let _ = black_box(pina::parse_instruction::<BenchInstruction>(
			black_box(&SYSTEM_ID),
			black_box(&SYSTEM_ID),
			data,
		));
	});
}

#[test]
fn benchmark_parse_instruction_invalid() {
	bench("parse_instruction (invalid discriminator)", || {
		let data = black_box(&[99u8]);
		let _ = black_box(pina::parse_instruction::<BenchInstruction>(
			black_box(&SYSTEM_ID),
			black_box(&SYSTEM_ID),
			data,
		));
	});
}

#[test]
fn benchmark_parse_instruction_wrong_program_id() {
	let wrong_id = pina::address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
	bench("parse_instruction (wrong program ID)", || {
		let data = black_box(&[0u8]);
		let _ = black_box(pina::parse_instruction::<BenchInstruction>(
			black_box(&SYSTEM_ID),
			black_box(&wrong_id),
			data,
		));
	});
}

// ---------------------------------------------------------------------------
// Account deserialization benchmarks
// ---------------------------------------------------------------------------

#[test]
fn benchmark_account_try_from_bytes() {
	let state = TestState::builder()
		.bump(42)
		.count(PodU64::from_primitive(999))
		.authority([7u8; 32].into())
		.build();
	let bytes: &[u8] = bytemuck::bytes_of(&state);

	bench("AccountDeserialize try_from_bytes (TestState)", || {
		let _ = black_box(TestState::try_from_bytes(black_box(bytes)));
	});
}

#[test]
fn benchmark_account_try_from_bytes_wrong_discriminator() {
	let mut data = [0u8; size_of::<TestState>()];
	data[0] = 99; // wrong discriminator
	bench(
		"AccountDeserialize try_from_bytes (wrong discriminator)",
		|| {
			let _ = black_box(TestState::try_from_bytes(black_box(&data)));
		},
	);
}

// ---------------------------------------------------------------------------
// Pod type conversion benchmarks
// ---------------------------------------------------------------------------

#[test]
fn benchmark_pod_u64_from_primitive() {
	bench("PodU64::from_primitive", || {
		let _ = black_box(PodU64::from_primitive(black_box(42u64)));
	});
}

#[test]
fn benchmark_pod_u64_into_u64() {
	let pod = PodU64::from_primitive(42);
	bench("PodU64 -> u64 conversion", || {
		let _: u64 = black_box(black_box(pod).into());
	});
}

// ---------------------------------------------------------------------------
// Seed combination benchmarks
// ---------------------------------------------------------------------------

#[test]
fn benchmark_combine_seeds_with_bump() {
	let seed_a: &[u8] = b"escrow";
	let seed_b: &[u8] = &[1u8; 32];
	let bump = [42u8; 1];

	bench("combine_seeds_with_bump (2 seeds)", || {
		let _ = black_box(pina::combine_seeds_with_bump(
			black_box(&[seed_a, seed_b]),
			black_box(&bump),
		));
	});
}

#[test]
fn benchmark_combine_seeds_with_bump_many_seeds() {
	let seeds: Vec<&[u8]> = (0..10).map(|_| &[1u8][..]).collect();
	let bump = [7u8; 1];

	bench("combine_seeds_with_bump (10 seeds)", || {
		let _ = black_box(pina::combine_seeds_with_bump(
			black_box(&seeds),
			black_box(&bump),
		));
	});
}

// ---------------------------------------------------------------------------
// Summary
// ---------------------------------------------------------------------------

#[test]
fn benchmark_summary() {
	eprintln!();
	eprintln!("=== Pina Core Operation Benchmark Summary ===");
	eprintln!();
	eprintln!("  Run with: cargo test -p pina --test benchmarks -- --nocapture");
	eprintln!();
	eprintln!("  Operations measured:");
	eprintln!("    - Discriminator matching (u8, u16, u32, enum)");
	eprintln!("    - PDA derivation (try_find_program_address, create_program_address)");
	eprintln!("    - Instruction parsing (valid, invalid, wrong program)");
	eprintln!("    - Account deserialization (try_from_bytes)");
	eprintln!("    - Pod type conversions (PodU64)");
	eprintln!("    - Seed combination (combine_seeds_with_bump)");
	eprintln!();
	eprintln!("  For actual CU measurement, run the counter_program CU benchmarks:");
	eprintln!("    cargo test -p counter_program --test cu_benchmarks -- --nocapture");
	eprintln!();
}
