#![allow(missing_docs)]

//! Adversarial security and differential parity tests for PDA derivation
//! (runtime + `const fn` Ed25519 curve check + `#[pda]` const methods),
//! `const-rent` helpers, and `__initialize_zeroed`.

use pina::*;

const TEST_PROGRAM: Address = address!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");
const ALT_PROGRAM: Address = address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

#[cfg(feature = "derive")]
#[discriminator]
pub enum PdaAccountType {
	GlobalConfig = 1,
}

#[cfg(feature = "derive")]
#[account(discriminator = PdaAccountType)]
#[pda(seeds = [b"global-config", b"v2"], bump = bump)]
pub struct GlobalConfig {
	pub bump: u8,
	pub version: u64,
}

/// Compile-time constant evaluation of `#[pda]`-generated `find_pda_const` and
/// `try_find_pda_const` ensures the entire `const fn` SHA-256 + Ed25519 curve
/// check runs inside rustc's const evaluator with zero runtime cost.
#[cfg(feature = "derive")]
const CONFIG_PDA_FROM_MACRO: (Address, u8) = GlobalConfig::find_pda_const(&TEST_PROGRAM);
#[cfg(feature = "derive")]
const CONFIG_PDA_OPT_FROM_MACRO: Option<(Address, u8)> =
	GlobalConfig::try_find_pda_const(&TEST_PROGRAM);
const DIRECT_CONST_PDA: (Address, u8) =
	find_program_address_const(&[b"global-config", b"v2"], &TEST_PROGRAM);
const EMPTY_SEED_CONST_PDA: (Address, u8) = find_program_address_const(&[], &TEST_PROGRAM);

#[test]
fn const_pda_macro_and_direct_helpers_match_solana_address_byte_for_byte() {
	let expected = Address::try_find_program_address(&[b"global-config", b"v2"], &TEST_PROGRAM)
		.unwrap_or_else(|| panic!("expected valid PDA"));

	assert_eq!(DIRECT_CONST_PDA, expected);
	#[cfg(feature = "derive")]
	{
		let runtime_macro = GlobalConfig::find_pda(&TEST_PROGRAM);
		assert_eq!(CONFIG_PDA_FROM_MACRO, expected);
		assert_eq!(CONFIG_PDA_OPT_FROM_MACRO, Some(expected));
		assert_eq!(runtime_macro, expected);
		assert_eq!(GlobalConfig::try_find_pda(&TEST_PROGRAM), Some(expected));
	}

	let empty_expected = Address::try_find_program_address(&[], &TEST_PROGRAM)
		.unwrap_or_else(|| panic!("expected valid empty-seed PDA"));
	assert_eq!(EMPTY_SEED_CONST_PDA, empty_expected);
}

#[test]
fn differential_pda_derivation_matches_solana_address_across_varied_seeds_and_all_bumps() {
	// Test 256 deterministic seed combinations across two distinct program IDs
	// for both runtime and `const fn` PDA derivation (bounded under Miri).
	let seed_count = if cfg!(miri) { 6_u16 } else { 256_u16 };
	for i in 0_u16..seed_count {
		let idx = i.to_le_bytes();
		let program = if i % 2 == 0 {
			&TEST_PROGRAM
		} else {
			&ALT_PROGRAM
		};

		let expected = Address::try_find_program_address(&[b"vault", &idx], program);
		let actual_runtime = try_find_program_address(&[b"vault", &idx], program);
		let actual_const = try_find_program_address_const(&[b"vault", &idx], program);

		assert_eq!(actual_runtime, expected, "runtime mismatch for seed {i}");
		assert_eq!(actual_const, expected, "const mismatch for seed {i}");

		if let Some((addr, bump)) = expected {
			assert!(is_derived_address(&addr, &[b"vault", &idx], bump, program));
			assert!(!is_derived_address(
				&addr,
				&[b"vault", &idx],
				bump.wrapping_sub(1),
				program
			));
		}
	}

	// Exhaustively check every bump 0..=255 for several prefixes so both
	// on-curve rejections AND off-curve acceptances are verified against
	// `Address::create_program_address` (bounded under Miri).
	let prefixes: &[&[u8]] = if cfg!(miri) {
		&[b"alpha"]
	} else {
		&[b"alpha", b"beta", b"gamma", b"delta"]
	};
	let bump_end = if cfg!(miri) { 15_u8 } else { 255_u8 };
	let min_each = if cfg!(miri) { 1_usize } else { 50_usize };
	for prefix in prefixes {
		let mut on_curve_count = 0_usize;
		let mut off_curve_count = 0_usize;

		for bump in 0_u8..=bump_end {
			let bump_seed = [bump];
			let seeds = [*prefix, &bump_seed];
			let expected = Address::create_program_address(&seeds, &TEST_PROGRAM);
			let actual = create_program_address(&seeds, &TEST_PROGRAM);

			if let Ok(expected_addr) = expected {
				off_curve_count += 1;
				assert_eq!(actual, Ok(expected_addr), "off-curve mismatch bump={bump}");
			} else {
				on_curve_count += 1;
				assert_eq!(
					actual,
					Err(ProgramError::InvalidSeeds),
					"on-curve point was NOT rejected for bump={bump}"
				);
			}
		}

		// Roughly half of random 256-bit hashes lie on Ed25519; ensure we
		// exercised both branches thoroughly.
		assert!(on_curve_count >= min_each, "expected on-curve bumps");
		assert!(off_curve_count >= min_each, "expected off-curve bumps");
	}
}

#[test]
fn adversarial_pda_seed_length_and_count_boundaries_are_enforced() {
	// 33-byte seed concatenation attack: `[0u8; 33]` has the same raw SHA-256
	// concatenation as `[0u8; 32] || [0u8; 1]`, so all APIs MUST reject > 32 B.
	let oversized = [0_u8; MAX_SEED_LEN + 1];
	assert_eq!(
		create_program_address(&[&oversized], &TEST_PROGRAM),
		Err(ProgramError::InvalidSeeds)
	);
	assert_eq!(try_find_program_address(&[&oversized], &TEST_PROGRAM), None);
	assert_eq!(
		try_find_program_address_const(&[&oversized], &TEST_PROGRAM),
		None
	);

	// Even if an attacker passes the exact hash derived from `[0; 32], [0; 1]`,
	// `is_derived_address` MUST reject the 33-byte seed.
	let (legit, bump) = try_find_program_address(&[&[0_u8; 32], &[0_u8; 1]], &TEST_PROGRAM)
		.unwrap_or_else(|| panic!("expected valid split-seed PDA"));
	assert!(is_derived_address(
		&legit,
		&[&[0_u8; 32], &[0_u8; 1]],
		bump,
		&TEST_PROGRAM
	));
	assert!(!is_derived_address(
		&legit,
		&[&oversized],
		bump,
		&TEST_PROGRAM
	));

	// Exact MAX_SEED_LEN (32 bytes) succeeds and matches `Address`.
	let max_seed = [0xAB_u8; MAX_SEED_LEN];
	assert_eq!(
		try_find_program_address(&[&max_seed], &TEST_PROGRAM),
		Address::try_find_program_address(&[&max_seed], &TEST_PROGRAM)
	);
	assert_eq!(
		try_find_program_address_const(&[&max_seed], &TEST_PROGRAM),
		Address::try_find_program_address(&[&max_seed], &TEST_PROGRAM)
	);

	// MAX_SEEDS boundary: 15 user seeds (+ 1 bump = 16) succeeds; 16 user seeds
	// (+ 1 bump = 17) is rejected by find; 17 seeds is rejected by create.
	let fifteen: [&[u8]; 15] = [b"s"; 15];
	let sixteen: [&[u8]; 16] = [b"s"; 16];
	let seventeen: [&[u8]; 17] = [b"s"; 17];

	let expected_15 = Address::try_find_program_address(&fifteen, &TEST_PROGRAM);
	assert_eq!(
		try_find_program_address(&fifteen, &TEST_PROGRAM),
		expected_15
	);
	assert_eq!(
		try_find_program_address_const(&fifteen, &TEST_PROGRAM),
		expected_15
	);
	assert_eq!(try_find_program_address(&sixteen, &TEST_PROGRAM), None);
	assert_eq!(
		create_program_address(&seventeen, &TEST_PROGRAM),
		Err(ProgramError::InvalidSeeds)
	);
}

#[cfg(feature = "derive")]
#[test]
fn initialize_zeroed_matches_initialize_and_rejects_wrong_buffer_sizes() {
	const WIRE_LEN: usize = size_of::<<GlobalConfig as PinaPodFixed>::Zc>();
	let mut a = [0_u8; WIRE_LEN];
	let mut b = [0_u8; WIRE_LEN];

	GlobalConfig::initialize(&mut a, |cfg| {
		cfg.bump = 254;
		cfg.version.set(42);
		Ok(())
	})
	.unwrap_or_else(|e| panic!("initialize failed: {e:?}"));

	<GlobalConfig as PinaAccount>::__initialize_zeroed(&mut b, |cfg| {
		cfg.bump = 254;
		cfg.version.set(42);
		Ok(())
	})
	.unwrap_or_else(|e| panic!("__initialize_zeroed failed: {e:?}"));

	assert_eq!(a, b);
	assert_eq!(
		GlobalConfig::try_from_bytes(&b)
			.unwrap_or_else(|e| panic!("read failed: {e:?}"))
			.version
			.get(),
		42
	);

	// Short and oversized buffers are rejected.
	let mut short = [0_u8; WIRE_LEN - 1];
	assert_eq!(
		<GlobalConfig as PinaAccount>::__initialize_zeroed(&mut short, |_| Ok(())),
		Err(ProgramError::InvalidAccountData)
	);
	let mut long = [0_u8; WIRE_LEN + 1];
	assert_eq!(
		<GlobalConfig as PinaAccount>::__initialize_zeroed(&mut long, |_| Ok(())),
		Err(ProgramError::InvalidAccountData)
	);
}

#[cfg(feature = "derive")]
#[test]
#[allow(unsafe_code)]
fn manual_pina_account_initialize_override_is_preserved_by_initialize_zeroed() {
	struct ManualFixedAccount;

	impl PinaPod for ManualFixedAccount {}

	// SAFETY: Delegates every fixed zero-copy invariant directly to the
	// macro-derived `GlobalConfig` implementation.
	unsafe impl PinaPodFixed for ManualFixedAccount {
		type Zc = <GlobalConfig as PinaPodFixed>::Zc;
	}

	impl HasDiscriminator for ManualFixedAccount {
		type Type = <GlobalConfig as HasDiscriminator>::Type;

		const VALUE: Self::Type = GlobalConfig::VALUE;
	}

	impl PinaAccount for ManualFixedAccount {
		fn write_zc_discriminator(value: &mut Self::Zc) {
			<GlobalConfig as PinaAccount>::write_zc_discriminator(value);
		}

		fn initialize<F>(data: &mut [u8], initialize: F) -> Result<&mut Self::Zc, ProgramError>
		where
			F: FnOnce(&mut Self::Zc) -> Result<(), PinaPodError>,
		{
			<GlobalConfig as PinaAccount>::initialize(data, |value| {
				// Distinct non-zero default set by the manual override.
				value.version.set(0xCAFE_BABE);
				initialize(value)
			})
		}
	}

	const WIRE_LEN: usize = size_of::<<ManualFixedAccount as PinaPodFixed>::Zc>();
	let mut buf = [0_u8; WIRE_LEN];
	let created = <ManualFixedAccount as PinaAccount>::__initialize_zeroed(&mut buf, |value| {
		value.bump = 7;
		Ok(())
	})
	.unwrap_or_else(|e| panic!("manual __initialize_zeroed failed: {e:?}"));
	assert_eq!(created.bump, 7);
	assert_eq!(created.version.get(), 0xCAFE_BABE);
	<ManualFixedAccount as PinaAccount>::write_zc_discriminator(created);
	assert!(<ManualFixedAccount as HasDiscriminator>::matches_discriminator(&buf));
}

#[cfg(feature = "const-rent")]
#[test]
fn const_rent_matches_default_rent_and_enforces_10_mib_limit() {
	const MAX_PERMITTED: usize = 10 * 1024 * 1024;
	const ZERO_RENT: u64 = match minimum_balance_const(0) {
		Ok(lamports) => lamports,
		Err(_) => panic!("zero space must be valid"),
	};
	const TEN_BYTE_RENT: u64 = match minimum_balance_const(10) {
		Ok(lamports) => lamports,
		Err(_) => panic!("10-byte space must be valid"),
	};

	let rent = default_rent();
	let expected_rent =
		sysvars::rent::Rent::from_bytes(&sysvars::rent::DEFAULT_LAMPORTS_PER_BYTE.to_le_bytes())
			.unwrap_or_else(|e| panic!("default rent bytes: {e:?}"));

	assert_eq!(ZERO_RENT, expected_rent.try_minimum_balance(0).unwrap());
	assert_eq!(
		TEN_BYTE_RENT,
		expected_rent.try_minimum_balance(10).unwrap()
	);

	for space in [0, 1, 10, 64, 128, 1_024, 10_240, 1_000_000, MAX_PERMITTED] {
		let expected = expected_rent
			.try_minimum_balance(space)
			.unwrap_or_else(|e| panic!("default rent failed for {space}: {e:?}"));
		assert_eq!(rent.try_minimum_balance(space), Ok(expected));
		assert_eq!(minimum_balance_const(space), Ok(expected));
	}

	// Adversarial over-limit / overflow sizes MUST return `InvalidArgument`.
	for bad_space in [MAX_PERMITTED + 1, usize::MAX / 2, usize::MAX] {
		assert_eq!(
			rent.try_minimum_balance(bad_space),
			Err(ProgramError::InvalidArgument)
		);
		assert_eq!(
			minimum_balance_const(bad_space),
			Err(ProgramError::InvalidArgument)
		);
	}
}

#[test]
fn find_program_address_const_runtime_matches_compile_time_value() {
	let runtime = find_program_address_const(&[b"global-config", b"v2"], &TEST_PROGRAM);
	assert_eq!(runtime, DIRECT_CONST_PDA);
	#[cfg(feature = "derive")]
	assert_eq!(
		GlobalConfig::find_pda_const(&TEST_PROGRAM),
		CONFIG_PDA_FROM_MACRO
	);
}

#[test]
#[should_panic(expected = "could not find program address from seeds")]
fn find_program_address_const_panics_on_oversized_seed() {
	let oversized = [0_u8; MAX_SEED_LEN + 1];
	let _ = find_program_address_const(&[&oversized], &TEST_PROGRAM);
}

#[cfg(all(feature = "derive", feature = "validation"))]
mod validated_zeroed_init {
	use pina::*;

	#[discriminator]
	pub enum ValidatedKind {
		GuardedAccount = 1,
	}

	#[account(discriminator = ValidatedKind)]
	pub struct GuardedAccount {
		#[pina(validate(value > 0))]
		pub amount: u64,
	}

	#[test]
	fn initialize_zeroed_clears_buffer_when_application_validation_fails() {
		const WIRE_LEN: usize = size_of::<<GuardedAccount as PinaPodFixed>::Zc>();
		let mut buf = [0_u8; WIRE_LEN];

		let err = <GuardedAccount as PinaAccount>::__initialize_zeroed(&mut buf, |acct| {
			acct.amount.set(0);
			Ok(())
		});
		assert!(err.is_err());
		// Adversarial security invariant: discriminator and partial bytes MUST
		// be wiped back to zero on validation failure.
		assert!(buf.iter().all(|&byte| byte == 0));

		// Valid input succeeds.
		<GuardedAccount as PinaAccount>::__initialize_zeroed(&mut buf, |acct| {
			acct.amount.set(5);
			Ok(())
		})
		.unwrap_or_else(|e| panic!("valid init should succeed: {e:?}"));
		assert_eq!(buf[0], ValidatedKind::GuardedAccount as u8);
	}
}

#[cfg(feature = "compact")]
mod compact_single_pass_adversarial {
	use pina::*;

	#[discriminator]
	pub enum CompactKind {
		CompactLedger = 1,
	}

	#[account(discriminator = CompactKind, compact)]
	pub struct CompactLedger {
		pub owner_tag: u64,
		pub values: Vec<u64, 4>,
	}

	#[test]
	fn generated_compact_single_pass_update_and_initialize_zeroed() {
		let one = [PodU64::from(10)];
		let init_patch = CompactLedgerPatch::new()
			.owner_tag(7_u64)
			.replace_values(&one);
		let size_one = CompactLedger::projected_bytes(1)
			.unwrap_or_else(|error| panic!("projected size: {error:?}"));

		let mut a = vec![0_u8; size_one];
		let mut b = vec![0_u8; size_one];
		assert_eq!(CompactLedger::initialize(&mut a, &init_patch), Ok(size_one));
		assert_eq!(
			<CompactLedger as PinaCompactAccount>::__initialize_zeroed(&mut b, &init_patch),
			Ok(size_one)
		);
		assert_eq!(a, b);

		// Corrupt discriminator is rejected before mutating storage.
		let mut bad_disc = b.clone();
		bad_disc[0] ^= 0xFF;
		let same_patch = CompactLedgerPatch::new().owner_tag(9_u64);
		assert_eq!(
			<CompactLedger as PinaCompactAccount>::__try_update_or_len(&mut bad_disc, &same_patch),
			Err(ProgramError::InvalidAccountData)
		);

		// Truncated storage is rejected before mutating storage.
		let mut short = vec![0_u8; CompactLedger::MIN_SIZE - 1];
		assert_eq!(
			<CompactLedger as PinaCompactAccount>::__try_update_or_len(&mut short, &same_patch),
			Err(ProgramError::InvalidAccountData)
		);

		// Grow needed returns Ok(Err(needed)) without mutating storage, while
		// direct `update()` maps it to `InvalidAccountData`.
		let two = [PodU64::from(10), PodU64::from(20)];
		let grow_patch = CompactLedgerPatch::new().replace_values(&two);
		let size_two = CompactLedger::projected_bytes(2)
			.unwrap_or_else(|error| panic!("projected size 2: {error:?}"));
		let before_grow = b.clone();
		assert_eq!(
			<CompactLedger as PinaCompactAccount>::__try_update_or_len(&mut b, &grow_patch),
			Ok(Err(size_two))
		);
		assert_eq!(b, before_grow);
		assert_eq!(
			CompactLedger::update(&mut b, &grow_patch),
			Err(ProgramError::InvalidAccountData)
		);

		// Same-size and shrink succeed in a single pass.
		assert_eq!(
			<CompactLedger as PinaCompactAccount>::__try_update_or_len(&mut b, &same_patch),
			Ok(Ok(size_one))
		);
		let shrink_patch = CompactLedgerPatch::new().replace_values(&[]);
		assert_eq!(
			<CompactLedger as PinaCompactAccount>::__try_update_or_len(&mut b, &shrink_patch),
			Ok(Ok(CompactLedger::MIN_SIZE))
		);
		let view = CompactLedger::try_from_bytes(&b[..CompactLedger::MIN_SIZE])
			.unwrap_or_else(|error| panic!("valid shrunk view: {error:?}"));
		assert_eq!(view.owner_tag.get(), 9);
		assert_eq!(view.values(), &[] as &[PodU64]);
	}

	#[test]
	#[allow(unsafe_code)]
	fn default_compact_trait_hooks_cover_fit_grow_mismatch_and_zeroed_init() {
		struct ManualWrapper;
		struct ManualPatch<'a> {
			inner: CompactLedgerPatch<'a>,
			lie_len_delta: usize,
		}

		impl PinaPod for ManualWrapper {}

		// SAFETY: Delegates every compact layout invariant directly to the
		// macro-derived `CompactLedger` implementation.
		unsafe impl PinaPodCompact for ManualWrapper {
			type Header = <CompactLedger as PinaPodCompact>::Header;

			const HEADER_SIZE: usize = CompactLedger::HEADER_SIZE;
			const MAX_SIZE: usize = CompactLedger::MAX_SIZE;
			const MIN_SIZE: usize = CompactLedger::MIN_SIZE;
			const TAIL_ALIGNMENT: usize = CompactLedger::TAIL_ALIGNMENT;

			fn validate(data: &[u8]) -> Result<(), PinaPodError> {
				<CompactLedger as PinaPodCompact>::validate(data)
			}
		}

		impl PinaPodPatch<ManualWrapper> for ManualPatch<'_> {
			fn updated_len(&self, data: &[u8]) -> Result<usize, PinaPodError> {
				self.inner.updated_len(data)
			}

			fn update(&self, data: &mut [u8]) -> Result<usize, PinaPodError> {
				self.inner.update(data)
			}

			fn initialize(&self, data: &mut [u8]) -> Result<usize, PinaPodError> {
				self.inner.initialize(data)
			}
		}

		impl HasDiscriminator for ManualWrapper {
			type Type = <CompactLedger as HasDiscriminator>::Type;

			const VALUE: Self::Type = CompactLedger::VALUE;
		}

		impl PinaCompactAccount for ManualWrapper {
			type Patch<'patch>
				= ManualPatch<'patch>
			where
				Self: 'patch;
			type Ref<'data>
				= <CompactLedger as PinaCompactAccount>::Ref<'data>
			where
				Self: 'data;

			fn try_from_bytes(data: &[u8]) -> Result<Self::Ref<'_>, ProgramError> {
				CompactLedger::try_from_bytes(data)
			}

			fn updated_len(data: &[u8], patch: &Self::Patch<'_>) -> Result<usize, ProgramError> {
				CompactLedger::updated_len(data, &patch.inner).map(|len| len + patch.lie_len_delta)
			}

			fn update(data: &mut [u8], patch: &Self::Patch<'_>) -> Result<usize, ProgramError> {
				CompactLedger::update(data, &patch.inner)
			}

			fn initialize(data: &mut [u8], patch: &Self::Patch<'_>) -> Result<usize, ProgramError> {
				CompactLedger::initialize(data, &patch.inner)
			}
		}

		let size_one = CompactLedger::projected_bytes(1)
			.unwrap_or_else(|error| panic!("projected size 1: {error:?}"));
		let size_two = CompactLedger::projected_bytes(2)
			.unwrap_or_else(|error| panic!("projected size 2: {error:?}"));
		let one = [PodU64::from(1)];
		let two = [PodU64::from(1), PodU64::from(2)];

		// Default `__initialize_zeroed` forwards to `initialize`.
		let mut buf = vec![0_u8; size_one];
		let init = ManualPatch {
			inner: CompactLedgerPatch::new()
				.owner_tag(3_u64)
				.replace_values(&one),
			lie_len_delta: 0,
		};
		assert_eq!(
			<ManualWrapper as PinaCompactAccount>::__initialize_zeroed(&mut buf, &init),
			Ok(size_one)
		);

		// Default `__try_update_or_len` handles grow-needed (`Ok(Err)`).
		let grow = ManualPatch {
			inner: CompactLedgerPatch::new().replace_values(&two),
			lie_len_delta: 0,
		};
		assert_eq!(
			<ManualWrapper as PinaCompactAccount>::__try_update_or_len(&mut buf, &grow),
			Ok(Err(size_two))
		);

		// Default `__try_update_or_len` handles in-place fit (`Ok(Ok)`).
		let fit = ManualPatch {
			inner: CompactLedgerPatch::new().owner_tag(5_u64),
			lie_len_delta: 0,
		};
		assert_eq!(
			<ManualWrapper as PinaCompactAccount>::__try_update_or_len(&mut buf, &fit),
			Ok(Ok(size_one))
		);

		// Default `__try_update_or_len` rejects a mis-predicting `updated_len`
		// that still fits in the existing buffer.
		buf.resize(size_two, 0);
		let lying = ManualPatch {
			inner: CompactLedgerPatch::new().owner_tag(6_u64),
			lie_len_delta: 8,
		};
		assert_eq!(
			<ManualWrapper as PinaCompactAccount>::__try_update_or_len(&mut buf, &lying),
			Err(ProgramError::InvalidAccountData)
		);
	}
}
