//! PDA (Program Derived Address) functions.
//!
//! These wrapper functions provide PDA derivation across native and Solana
//! targets.
//!
//! Seed-based APIs require deterministic seed ordering and consistent program
//! IDs across derivation and verification.

use crate::Address;
use crate::ProgramError;

#[cfg(any(target_os = "solana", target_arch = "bpf"))]
#[repr(C)]
#[derive(Clone, Copy)]
struct SolBytes {
	addr: *const u8,
	len: u64,
}

#[cfg(any(target_os = "solana", target_arch = "bpf"))]
const _: () = {
	assert!(size_of::<SolBytes>() == size_of::<&[u8]>());
	assert!(align_of::<SolBytes>() == align_of::<&[u8]>());
};

#[inline(always)]
#[allow(unsafe_code)]
fn sha256_matches_address(inputs: &[&[u8]], address: &Address) -> bool {
	#[cfg(any(target_os = "solana", target_arch = "bpf"))]
	{
		let mut derived = core::mem::MaybeUninit::<[u8; crate::ADDRESS_BYTES]>::uninit();
		// SAFETY: `sol_sha256` reads `inputs.len()` `(ptr, len)` slice descriptors
		// from `inputs` and initializes all 32 bytes of `derived`.
		unsafe {
			solana_sha256_hasher::sol_sha256(
				inputs.as_ptr().cast::<u8>(),
				inputs.len() as u64,
				derived.as_mut_ptr().cast::<u8>(),
			);
			derived.assume_init_ref() == address.as_array()
		}
	}

	#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
	{
		solana_sha256_hasher::hashv(inputs).to_bytes() == *address.as_array()
	}
}

/// Hashes `count` contiguous `pinocchio::cpi::Seed` entries directly via
/// `sol_sha256` and compares the digest against `address`.
///
/// `Seed` is `#[repr(C)] { seed: *const u8, len: u64 }` — identical to
/// Solana's `SolBytes` syscall descriptor — allowing creation builders to
/// verify a PDA and construct its CPI `Signer` from one stack seed buffer.
///
/// # Safety
///
/// `seeds_ptr` must point to at least `count` initialized `Seed` entries whose
/// backing byte slices remain valid for the duration of the syscall.
#[cfg(any(target_os = "solana", target_arch = "bpf"))]
#[inline(always)]
#[allow(unsafe_code)]
pub(crate) unsafe fn sha256_seed_slices_matches_address(
	seeds_ptr: *const pinocchio::cpi::Seed<'_>,
	count: usize,
	address: &Address,
) -> bool {
	const {
		assert!(size_of::<pinocchio::cpi::Seed<'_>>() == size_of::<SolBytes>());
		assert!(align_of::<pinocchio::cpi::Seed<'_>>() == align_of::<SolBytes>());
	}
	let mut derived = core::mem::MaybeUninit::<[u8; crate::ADDRESS_BYTES]>::uninit();
	// SAFETY: `Seed` has the exact `#[repr(C)] { ptr, len }` layout expected by
	// `sol_sha256`, and the caller guarantees `count` initialized entries.
	unsafe {
		solana_sha256_hasher::sol_sha256(
			seeds_ptr.cast::<u8>(),
			count as u64,
			derived.as_mut_ptr().cast::<u8>(),
		);
		derived.assume_init_ref() == address.as_array()
	}
}

/// Shared SBF PDA derivation kernel for [`try_find_program_address`]
/// (`search_bump = true`) and [`create_program_address`] (`search_bump = false`).
///
/// Using one out-of-line kernel avoids duplicating the `SolBytes` setup and
/// `sol_sha256` + `bytes_are_curve_point` loop in programs that call both APIs,
/// and raw-pointer writes inside the already-bounds-checked `MAX_SEEDS + 2`
/// stack buffer avoid emitting per-index panic stubs.
#[cfg(any(target_os = "solana", target_arch = "bpf"))]
#[inline(never)]
#[allow(unsafe_code)]
fn sbf_derive_pda(
	seeds: &[&[u8]],
	program_id: &Address,
	search_bump: bool,
	out: &mut core::mem::MaybeUninit<[u8; crate::ADDRESS_BYTES]>,
) -> u8 {
	let max_seeds = if search_bump {
		crate::MAX_SEEDS - 1
	} else {
		crate::MAX_SEEDS
	};
	let seeds_len = seeds.len();
	if pinocchio::hint::unlikely(seeds_len > max_seeds) {
		return 0;
	}

	// When `search_bump` is false, starting at `1` makes `bump <= 1` exit after
	// a single hash with zero extra branch inside the hot loop.
	let mut bump = if search_bump { u8::MAX } else { 1 };
	let mut bump_seed = [bump];
	let bump_ptr = bump_seed.as_mut_ptr();
	let mut inputs = [const { core::mem::MaybeUninit::<SolBytes>::uninit() }; crate::MAX_SEEDS + 2];

	// SAFETY: `seeds_len <= MAX_SEEDS` (and `<= MAX_SEEDS - 1` when
	// `search_bump` is true), so every write at `0..count` stays strictly
	// within `inputs[0..MAX_SEEDS + 2]`. `bump_seed` is updated exclusively
	// through `bump_ptr` to preserve strict provenance across loop iterations,
	// and `sol_sha256` initializes all 32 bytes of `out`.
	unsafe {
		let dst = inputs.as_mut_ptr();
		let mut i = 0;

		while i < seeds_len {
			let seed = *seeds.get_unchecked(i);
			if pinocchio::hint::unlikely(seed.len() > crate::MAX_SEED_LEN) {
				return 0;
			}
			(*dst.add(i)).write(SolBytes {
				addr: seed.as_ptr(),
				len: seed.len() as u64,
			});
			i += 1;
		}

		if search_bump {
			(*dst.add(i)).write(SolBytes {
				addr: bump_ptr.cast_const(),
				len: 1,
			});
			i += 1;
		}

		(*dst.add(i)).write(SolBytes {
			addr: program_id.as_ref().as_ptr(),
			len: crate::ADDRESS_BYTES as u64,
		});
		(*dst.add(i + 1)).write(SolBytes {
			addr: solana_address::PDA_MARKER.as_ptr(),
			len: solana_address::PDA_MARKER.len() as u64,
		});

		let count = (i + 2) as u64;
		let out_ptr = out.as_mut_ptr().cast::<u8>();

		// Match Agave's `Pubkey::try_find_program_address` / `sol_try_find_program_address`
		// exact bump domain (`255` down through `1`, 255 iterations) when searching.
		loop {
			solana_sha256_hasher::sol_sha256(dst.cast::<u8>(), count, out_ptr);
			if !solana_address::bytes_are_curve_point(out.assume_init_ref()) {
				return bump;
			}

			if bump <= 1 {
				return 0;
			}
			bump -= 1;
			*bump_ptr = bump;
		}
	}
}

/// Find a valid program derived address and its corresponding bump seed.
///
/// Returns `None` if no valid PDA exists.
///
/// This is the preferred PDA derivation API in `pina` because it is explicit
/// about failure and avoids panics in on-chain code paths.
///
/// On Solana targets, searches using a single stack preimage over `sol_sha256`
/// and `sol_curve_validate_point` (~260 CU per bump) instead of
/// `sol_try_find_program_address` (~1,500 CU per bump), matching Agave's exact
/// `255..=1` bump range and seed validation limits.
///
/// # Examples
///
/// ```
/// use pina::try_find_program_address;
///
/// let program_id = pina::address!("11111111111111111111111111111111");
/// let seeds: &[&[u8]] = &[b"vault"];
///
/// if let Some((pda, bump)) = try_find_program_address(seeds, &program_id) {
/// 	// `pda` is the derived address, `bump` is the canonical bump seed.
/// 	assert!(bump <= 255);
/// }
/// ```
#[inline]
#[allow(unsafe_code)]
pub fn try_find_program_address(seeds: &[&[u8]], program_id: &Address) -> Option<(Address, u8)> {
	#[cfg(any(target_os = "solana", target_arch = "bpf"))]
	{
		let mut out = core::mem::MaybeUninit::<[u8; crate::ADDRESS_BYTES]>::uninit();
		let bump = sbf_derive_pda(seeds, program_id, true, &mut out);
		if bump == 0 {
			return None;
		}
		// SAFETY: `sbf_derive_pda` only returns non-zero after `sol_sha256`
		// initialized all 32 bytes of `out`.
		Some((Address::new_from_array(unsafe { out.assume_init() }), bump))
	}

	#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
	{
		if seeds.len() >= crate::MAX_SEEDS {
			return None;
		}
		Address::try_find_program_address(seeds, program_id)
	}
}

/// Create a valid program derived address without searching for a bump seed.
///
/// Use this when your instruction already carries a bump and you want to
/// verify exact PDA derivation against user-provided seeds.
///
/// On Solana targets, derives with `sol_sha256` + `sol_curve_validate_point`
/// (~260 CU) instead of `sol_create_program_address` (~1,500 CU), preserving
/// exact Agave seed limits and off-curve validation.
///
/// <!-- {=pinaPdaSeedContract|trim|linePrefix:"/// ":true} -->
/// Seed-based APIs require deterministic seed ordering.
///
/// Program IDs must stay consistent across derivation and verification.
///
/// When a bump is required, prefer canonical bump derivation.
///
/// Use explicit bumps when needed.<!-- {/pinaPdaSeedContract} -->
///
/// # Examples
///
/// ```
/// use pina::create_program_address;
/// use pina::try_find_program_address;
///
/// let program_id = pina::address!("11111111111111111111111111111111");
/// let seeds: &[&[u8]] = &[b"vault"];
///
/// // First derive the canonical PDA and bump:
/// let (pda, bump) =
/// 	try_find_program_address(seeds, &program_id).unwrap_or_else(|| panic!("no valid PDA"));
///
/// // Then recreate the address using the known bump:
/// let bump_seed = [bump];
/// let recreated = create_program_address(&[b"vault", &bump_seed], &program_id)
/// 	.unwrap_or_else(|e| panic!("failed to recreate: {e:?}"));
/// assert_eq!(pda, recreated);
/// ```
#[inline]
#[allow(unsafe_code)]
pub fn create_program_address(
	seeds: &[&[u8]],
	program_id: &Address,
) -> Result<Address, ProgramError> {
	#[cfg(any(target_os = "solana", target_arch = "bpf"))]
	{
		let mut out = core::mem::MaybeUninit::<[u8; crate::ADDRESS_BYTES]>::uninit();
		if sbf_derive_pda(seeds, program_id, false, &mut out) == 0 {
			return Err(ProgramError::InvalidSeeds);
		}
		// SAFETY: `sbf_derive_pda` only returns non-zero after `sol_sha256`
		// initialized all 32 bytes of `out`.
		Ok(Address::new_from_array(unsafe { out.assume_init() }))
	}

	#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
	{
		if seeds.len() > crate::MAX_SEEDS {
			return Err(ProgramError::InvalidSeeds);
		}
		let mut hasher = solana_sha256_hasher::Hasher::default();
		for seed in seeds {
			if seed.len() > crate::MAX_SEED_LEN {
				return Err(ProgramError::InvalidSeeds);
			}
			hasher.hash(seed);
		}
		hasher.hashv(&[program_id.as_ref(), solana_address::PDA_MARKER.as_slice()]);
		let derived = hasher.result().to_bytes();

		if solana_address::bytes_are_curve_point(derived) {
			return Err(ProgramError::InvalidSeeds);
		}

		Ok(Address::new_from_array(derived))
	}
}

/// Returns whether `address` is the program address `seeds` and `bump`
/// derive for `program_id`, without checking that it is off the ed25519
/// curve.
///
/// This hashes the same input as [`create_program_address`] with the
/// `sol_sha256` syscall instead of `sol_create_program_address`, which saves
/// about 1,350 compute units per call. Like [`create_program_address`], it
/// rejects a seed longer than [`MAX_SEED_LEN`](crate::MAX_SEED_LEN): the hash
/// concatenates the seeds, so without that check a 33-byte seed would hash like
/// a 32-byte seed followed by a 1-byte one. The only address it accepts that
/// [`create_program_address`] rejects is an on-curve hash of those inputs.
///
/// That makes it the right check for re-verifying an account the program
/// created at a derived address from the bump it stored: the program created
/// the account through `invoke_signed`, and the runtime only signs for an
/// off-curve address, so the stored bump already derived a valid program
/// address. Use [`create_program_address`] to check a bump a caller supplies,
/// and [`try_find_program_address`] when the bump must be canonical.
///
/// # Examples
///
/// ```
/// use pina::is_derived_address;
/// use pina::try_find_program_address;
///
/// let program_id = pina::address!("11111111111111111111111111111111");
/// let (pda, bump) = try_find_program_address(&[b"vault"], &program_id)
/// 	.unwrap_or_else(|| panic!("no valid PDA"));
///
/// assert!(is_derived_address(&pda, &[b"vault"], bump, &program_id));
/// assert!(!is_derived_address(&pda, &[b"other"], bump, &program_id));
/// ```
#[inline(always)]
#[allow(unsafe_code)]
pub fn is_derived_address<const N: usize>(
	address: &Address,
	seeds: &[&[u8]; N],
	bump: u8,
	program_id: &Address,
) -> bool {
	const {
		assert!(
			N < crate::MAX_SEEDS,
			"number of seeds must be less than MAX_SEEDS"
		);
	}

	// A seed of constant or fixed-size length folds this check away.
	if !seeds.iter().all(|seed| seed.len() <= crate::MAX_SEED_LEN) {
		return false;
	}

	let bump_slice = [bump];
	let mut inputs = [const { core::mem::MaybeUninit::<&[u8]>::uninit() }; crate::MAX_SEEDS + 2];
	let mut i = 0;

	while i < N {
		inputs[i].write(seeds[i]);
		i += 1;
	}

	inputs[N].write(bump_slice.as_slice());
	inputs[N + 1].write(program_id.as_ref());
	inputs[N + 2].write(solana_address::PDA_MARKER.as_slice());

	// SAFETY: slots `0..N + 3` were all initialized above, and `N < MAX_SEEDS`,
	// so `N + 3 <= MAX_SEEDS + 2`.
	let slice = unsafe { core::slice::from_raw_parts(inputs.as_ptr().cast::<&[u8]>(), N + 3) };

	sha256_matches_address(slice, address)
}

/// Returns whether `address` is the program address `seeds` and `bump`
/// derive for `program_id`, for a seed list whose length is only known at
/// run time, without checking that the address is off the ed25519 curve.
///
/// `inputs` is scratch space for the hash inputs: it must hold at least
/// `seeds.len() + 3` slices. This is the creation builders' counterpart of
/// [`is_derived_address`], used where the account is about to be created
/// through `invoke_signed` with the same seeds and bump. The runtime derives
/// the signer's address with the curve check during that call, so an on-curve
/// hash is still rejected, just by the runtime instead of by this check.
///
/// Like [`is_derived_address`], it rejects a seed longer than
/// [`MAX_SEED_LEN`](crate::MAX_SEED_LEN) before hashing, so the builder
/// returns `InvalidSeeds` for it as it did with `create_program_address`,
/// instead of leaving the rejection to the runtime.
#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
#[inline(always)]
pub(crate) fn hashes_to_program_address<'a>(
	address: &Address,
	seeds: &[&'a [u8]],
	bump: &'a [u8; 1],
	program_id: &'a Address,
	inputs: &mut [&'a [u8]],
) -> bool {
	if seeds.iter().any(|seed| seed.len() > crate::MAX_SEED_LEN) {
		return false;
	}

	let seeds_len = seeds.len();
	inputs[..seeds_len].copy_from_slice(seeds);
	inputs[seeds_len] = bump.as_slice();
	inputs[seeds_len + 1] = program_id.as_ref();
	inputs[seeds_len + 2] = solana_address::PDA_MARKER.as_slice();

	sha256_matches_address(&inputs[..seeds_len + 3], address)
}

/// Pure `const fn` Ed25519 prime-field element ($\mathbb{F}_{2^{255}-19}$) in
/// 5 radix-$2^{51}$ limbs, used for compile-time PDA off-curve verification.
#[derive(Clone, Copy)]
struct Fe51([u64; 5]);

impl Fe51 {
	/// Ed25519 curve constant $d = -121665 / 121666 \bmod (2^{255} - 19)$.
	const EDWARDS_D: Self = Self([
		929_955_233_495_203,
		466_365_720_129_213,
		1_662_059_464_998_953,
		2_033_849_074_728_123,
		1_442_794_654_840_575,
	]);
	const MASK: u64 = (1_u64 << 51) - 1;
	const ONE: Self = Self([1, 0, 0, 0, 0]);
	const ZERO: Self = Self([0, 0, 0, 0, 0]);

	const fn load8(bytes: &[u8; 32], offset: usize) -> u64 {
		u64::from_le_bytes([
			bytes[offset],
			bytes[offset + 1],
			bytes[offset + 2],
			bytes[offset + 3],
			bytes[offset + 4],
			bytes[offset + 5],
			bytes[offset + 6],
			bytes[offset + 7],
		])
	}

	/// Matches `curve25519_dalek`'s `FieldElement51::from_bytes` bit extraction
	/// (clears bit 255 and reduces mod $2^{255} - 19$).
	const fn from_bytes(bytes: &[u8; 32]) -> Self {
		Self([
			Self::load8(bytes, 0) & Self::MASK,
			(Self::load8(bytes, 6) >> 3) & Self::MASK,
			(Self::load8(bytes, 12) >> 6) & Self::MASK,
			(Self::load8(bytes, 19) >> 1) & Self::MASK,
			(Self::load8(bytes, 24) >> 12) & Self::MASK,
		])
	}

	const fn reduce(mut h: [u64; 5]) -> Self {
		let c0 = h[0] >> 51;
		h[0] &= Self::MASK;
		h[1] += c0;
		let c1 = h[1] >> 51;
		h[1] &= Self::MASK;
		h[2] += c1;
		let c2 = h[2] >> 51;
		h[2] &= Self::MASK;
		h[3] += c2;
		let c3 = h[3] >> 51;
		h[3] &= Self::MASK;
		h[4] += c3;
		let c4 = h[4] >> 51;
		h[4] &= Self::MASK;
		h[0] += c4 * 19;
		let c0 = h[0] >> 51;
		h[0] &= Self::MASK;
		h[1] += c0;
		Self(h)
	}

	const fn add(self, rhs: Self) -> Self {
		Self::reduce([
			self.0[0] + rhs.0[0],
			self.0[1] + rhs.0[1],
			self.0[2] + rhs.0[2],
			self.0[3] + rhs.0[3],
			self.0[4] + rhs.0[4],
		])
	}

	const fn sub(self, rhs: Self) -> Self {
		// Add 2p before subtracting so every limb remains positive.
		Self::reduce([
			(self.0[0] + 2 * (Self::MASK - 18)) - rhs.0[0],
			(self.0[1] + 2 * Self::MASK) - rhs.0[1],
			(self.0[2] + 2 * Self::MASK) - rhs.0[2],
			(self.0[3] + 2 * Self::MASK) - rhs.0[3],
			(self.0[4] + 2 * Self::MASK) - rhs.0[4],
		])
	}

	const fn mul(self, rhs: Self) -> Self {
		let a0 = self.0[0] as u128;
		let a1 = self.0[1] as u128;
		let a2 = self.0[2] as u128;
		let a3 = self.0[3] as u128;
		let a4 = self.0[4] as u128;

		let b0 = rhs.0[0] as u128;
		let b1 = rhs.0[1] as u128;
		let b2 = rhs.0[2] as u128;
		let b3 = rhs.0[3] as u128;
		let b4 = rhs.0[4] as u128;

		let b1_19 = b1 * 19;
		let b2_19 = b2 * 19;
		let b3_19 = b3 * 19;
		let b4_19 = b4 * 19;

		let c0 = a0 * b0 + a1 * b4_19 + a2 * b3_19 + a3 * b2_19 + a4 * b1_19;
		let mut c1 = a0 * b1 + a1 * b0 + a2 * b4_19 + a3 * b3_19 + a4 * b2_19;
		let mut c2 = a0 * b2 + a1 * b1 + a2 * b0 + a3 * b4_19 + a4 * b3_19;
		let mut c3 = a0 * b3 + a1 * b2 + a2 * b1 + a3 * b0 + a4 * b4_19;
		let mut c4 = a0 * b4 + a1 * b3 + a2 * b2 + a3 * b1 + a4 * b0;

		let mut h0 = (c0 as u64) & Self::MASK;
		c1 += c0 >> 51;
		let h1 = (c1 as u64) & Self::MASK;
		c2 += c1 >> 51;
		let h2 = (c2 as u64) & Self::MASK;
		c3 += c2 >> 51;
		let h3 = (c3 as u64) & Self::MASK;
		c4 += c3 >> 51;
		let h4 = (c4 as u64) & Self::MASK;
		h0 += ((c4 >> 51) as u64) * 19;

		Self::reduce([h0, h1, h2, h3, h4])
	}

	const fn pow2k(mut self, mut k: u32) -> Self {
		while k > 0 {
			self = self.mul(self);
			k -= 1;
		}
		self
	}

	/// Computes `self^((p - 5) / 8) = self^(2^252 - 3)`.
	const fn pow_p58(self) -> Self {
		let t0 = self.mul(self);
		let t1 = t0.pow2k(2).mul(self);
		let t2 = t1.mul(t0);
		let t3 = t2.mul(t2).mul(t1);
		let t4 = t3.pow2k(5).mul(t3);
		let t5 = t4.pow2k(10).mul(t4);
		let t6 = t5.pow2k(20).mul(t5);
		let t7 = t6.pow2k(10).mul(t4);
		let t8 = t7.pow2k(50).mul(t7);
		let t9 = t8.pow2k(100).mul(t8);
		let t10 = t9.pow2k(50).mul(t7);
		t10.pow2k(2).mul(self)
	}

	const fn canonical(self) -> [u64; 5] {
		// Two reductions guarantee every limb is strictly `< 2^51` even when the
		// first reduction's wrap carry leaves `h[1] == 2^51`.
		let h = Self::reduce(Self::reduce(self.0).0).0;
		let mut t0 = h[0] + 19;
		let mut t1 = h[1] + (t0 >> 51);
		t0 &= Self::MASK;
		let mut t2 = h[2] + (t1 >> 51);
		t1 &= Self::MASK;
		let mut t3 = h[3] + (t2 >> 51);
		t2 &= Self::MASK;
		let t4 = h[4] + (t3 >> 51);
		t3 &= Self::MASK;

		if (t4 >> 51) != 0 {
			[t0, t1, t2, t3, t4 & Self::MASK]
		} else {
			h
		}
	}

	const fn eq_canonical(self, other: Self) -> bool {
		let a = self.canonical();
		let b = other.canonical();
		a[0] == b[0] && a[1] == b[1] && a[2] == b[2] && a[3] == b[3] && a[4] == b[4]
	}
}

/// Pure `const fn` check matching `solana_address::bytes_are_curve_point`
/// (`curve25519_dalek::edwards::CompressedEdwardsY::decompress().is_some()`).
pub(crate) const fn is_on_curve_const(bytes: &[u8; 32]) -> bool {
	let y = Fe51::from_bytes(bytes);
	let yy = y.mul(y);
	let u = yy.sub(Fe51::ONE);
	let v = yy.mul(Fe51::EDWARDS_D).add(Fe51::ONE);

	let v3 = v.mul(v).mul(v);
	let v7 = v3.mul(v3).mul(v);
	let r = u.mul(v3).mul(u.mul(v7).pow_p58());
	let check = v.mul(r.mul(r));

	check.eq_canonical(u) || check.eq_canonical(Fe51::ZERO.sub(u))
}

/// Derive a canonical PDA and bump seed at compile time.
///
/// Evaluates the exact same SHA-256 preimage, Ed25519 off-curve check,
/// `255..=1` bump search order, and seed length limits as
/// [`try_find_program_address`], but runs purely in `const` evaluation so
/// programs with constant seeds pay **0 compute units** at runtime.
///
/// Only use this in `const` contexts where all seeds and `program_id` are
/// compile-time constants; at runtime, call [`try_find_program_address`] so
/// Solana targets use hardware-accelerated syscalls instead of software SHA-256
/// and field arithmetic.
///
/// # Examples
///
/// ```
/// use pina::Address;
/// use pina::try_find_program_address;
/// use pina::try_find_program_address_const;
///
/// const PROGRAM_ID: Address = pina::address!("11111111111111111111111111111111");
/// const CONFIG_PDA: Option<(Address, u8)> =
/// 	try_find_program_address_const(&[b"config"], &PROGRAM_ID);
///
/// assert_eq!(
/// 	CONFIG_PDA,
/// 	try_find_program_address(&[b"config"], &PROGRAM_ID)
/// );
/// ```
pub const fn try_find_program_address_const<const N: usize>(
	seeds: &[&[u8]; N],
	program_id: &Address,
) -> Option<(Address, u8)> {
	const {
		assert!(
			N < crate::MAX_SEEDS,
			"number of seeds must be less than MAX_SEEDS"
		);
	}

	let mut i = 0;
	while i < N {
		if seeds[i].len() > crate::MAX_SEED_LEN {
			return None;
		}
		i += 1;
	}

	let mut result = None;
	let mut bump = u8::MAX;
	while bump > 0 {
		let candidate = Address::derive_address_const(seeds, Some(bump), program_id);
		if !is_on_curve_const(candidate.as_array()) {
			result = Some((candidate, bump));
			break;
		}
		bump -= 1;
	}

	result
}

/// Derive a canonical PDA and bump seed at compile time, panicking during
/// compilation if the seeds are invalid or no bump exists.
///
/// # Panics
///
/// Panics if `N >= MAX_SEEDS`, any seed exceeds `MAX_SEED_LEN`, or no off-curve
/// bump exists in `1..=255`.
///
/// # Examples
///
/// ```
/// use pina::Address;
/// use pina::find_program_address_const;
/// use pina::try_find_program_address;
///
/// const PROGRAM_ID: Address = pina::address!("11111111111111111111111111111111");
/// const CONFIG_PDA: (Address, u8) = find_program_address_const(&[b"config"], &PROGRAM_ID);
///
/// assert_eq!(
/// 	Some(CONFIG_PDA),
/// 	try_find_program_address(&[b"config"], &PROGRAM_ID)
/// );
/// ```
pub const fn find_program_address_const<const N: usize>(
	seeds: &[&[u8]; N],
	program_id: &Address,
) -> (Address, u8) {
	match try_find_program_address_const(seeds, program_id) {
		Some(result) => result,
		None => panic!("could not find program address from seeds"),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn native_find_and_create_program_address_roundtrip() {
		let seeds: &[&[u8]] = &[b"pina-test"];
		let (pda, bump) =
			try_find_program_address(seeds, &crate::system::ID).unwrap_or_else(|| {
				panic!("expected to derive pda");
			});
		let bump_seed = [bump];
		let seeds_with_bump: &[&[u8]] = &[b"pina-test", &bump_seed];
		let recreated = create_program_address(seeds_with_bump, &crate::system::ID)
			.unwrap_or_else(|err| panic!("failed to recreate pda: {err:?}"));

		assert_eq!(pda, recreated);
	}

	/// Seeds are hashed back to back, so a 33-byte seed hashes like a valid
	/// 32-byte seed followed by a 1-byte one. The check must reject it, as
	/// `create_program_address` does, instead of accepting that address.
	#[test]
	fn derived_address_check_rejects_a_seed_longer_than_the_limit() {
		let bytes = [3_u8; 33];
		let split: [&[u8]; 2] = [&bytes[..32], &bytes[32..]];
		let (address, bump) = try_find_program_address(&split, &crate::system::ID)
			.expect("the split seeds derive a program address");
		let bump_seed = [bump];

		assert!(is_derived_address(
			&address,
			&split,
			bump,
			&crate::system::ID
		));
		assert_eq!(
			create_program_address(&[&bytes, &bump_seed], &crate::system::ID),
			Err(ProgramError::InvalidSeeds)
		);
		assert!(!is_derived_address(
			&address,
			&[&bytes],
			bump,
			&crate::system::ID
		));
	}

	/// The hash-only check agrees with `create_program_address` for every
	/// bump that derives a valid program address, and rejects other seeds,
	/// bumps, and program IDs.
	#[test]
	fn derived_address_check_matches_create_program_address() {
		let seeds: [&[u8]; 2] = [b"pina-test", b"derived"];
		let mut checked_bumps = 0;

		for bump in 0..=u8::MAX {
			let bump_seed = [bump];
			let Ok(address) =
				create_program_address(&[seeds[0], seeds[1], &bump_seed], &crate::system::ID)
			else {
				continue;
			};

			assert!(is_derived_address(
				&address,
				&seeds,
				bump,
				&crate::system::ID
			));
			assert!(!is_derived_address(
				&address,
				&seeds,
				bump.wrapping_add(1),
				&crate::system::ID
			));
			assert!(!is_derived_address(
				&address,
				&[seeds[0]],
				bump,
				&crate::system::ID
			));
			assert!(!is_derived_address(
				&address,
				&seeds,
				bump,
				&Address::new_from_array([9; 32])
			));
			checked_bumps += 1;
		}

		assert!(checked_bumps > 0);
	}

	/// For an on-curve hash, `create_program_address` fails while the
	/// hash-only check still matches the hash, which is the one case the two
	/// disagree on.
	#[test]
	fn derived_address_check_skips_only_the_curve_check() {
		let seeds: [&[u8]; 1] = [b"pina-test"];
		let on_curve_bump = (0..=u8::MAX).find(|bump| {
			create_program_address(&[seeds[0], &[*bump]], &crate::system::ID).is_err()
		});
		assert!(on_curve_bump.is_some(), "expected an on-curve bump");
		let on_curve_bump = on_curve_bump.unwrap_or_default();
		let hash = Address::derive_address(&seeds, Some(on_curve_bump), &crate::system::ID);

		assert!(is_derived_address(
			&hash,
			&seeds,
			on_curve_bump,
			&crate::system::ID
		));
	}

	/// Adversarial differential test for `is_on_curve_const` against
	/// `solana_address::bytes_are_curve_point`: covers $y \in \{0, 1, 2, p-1,
	/// p, p+1, p+18, 2^{255}-1\}$ with both sign bits, torsion points,
	/// basepoint, and 500 deterministic hashes.
	#[test]
	fn const_curve_check_matches_solana_address_on_all_edge_cases() {
		// p = 2^255 - 19 = 0x7fff...ffed
		let mut p_minus_1 = [0xFF_u8; 32];
		p_minus_1[0] = 0xEC;
		p_minus_1[31] = 0x7F;

		let mut p_exact = [0xFF_u8; 32];
		p_exact[0] = 0xED;
		p_exact[31] = 0x7F;

		let mut p_plus_1 = [0xFF_u8; 32];
		p_plus_1[0] = 0xEE;
		p_plus_1[31] = 0x7F;

		let mut max_255 = [0xFF_u8; 32];
		max_255[31] = 0x7F;

		let mut zero = [0_u8; 32];
		let mut one = [0_u8; 32];
		one[0] = 1;
		let mut two = [0_u8; 32];
		two[0] = 2;

		// Ed25519 basepoint y = 4/5 mod p.
		let basepoint: [u8; 32] = [
			0x58, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
			0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
			0x66, 0x66, 0x66, 0x66,
		];
		// Small-order torsion points on Ed25519.
		let torsion_order_4: [u8; 32] = [
			0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
			0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
			0x00, 0x00, 0x00, 0x00,
		];

		let edge_cases = [
			zero,
			one,
			two,
			p_minus_1,
			p_exact,
			p_plus_1,
			max_255,
			basepoint,
			torsion_order_4,
		];

		for base in edge_cases {
			for sign_mask in [0x00_u8, 0x80_u8] {
				let mut candidate = base;
				candidate[31] = (candidate[31] & 0x7F) | sign_mask;
				assert_eq!(
					is_on_curve_const(&candidate),
					solana_address::bytes_are_curve_point(candidate),
					"mismatch on edge point {candidate:02x?}"
				);
			}
		}

		// Deterministic sweep over SHA-256 outputs (bounded under Miri).
		let sweep = if cfg!(miri) { 16_u32 } else { 500_u32 };
		let mut on = 0_usize;
		let mut off = 0_usize;
		for counter in 0_u32..sweep {
			let hash = solana_sha256_hasher::hash(&counter.to_le_bytes()).to_bytes();
			let expected = solana_address::bytes_are_curve_point(hash);
			let actual = is_on_curve_const(&hash);
			assert_eq!(actual, expected, "mismatch on hash {counter}");
			if actual {
				on += 1;
			} else {
				off += 1;
			}
		}
		let min_expected = if cfg!(miri) { 1 } else { 200 };
		assert!(on >= min_expected && off >= min_expected);

		// Confirm zero-hash edge case exercises `y = 0` / `v == 0` paths cleanly.
		zero[0] = 0;
		assert_eq!(
			is_on_curve_const(&zero),
			solana_address::bytes_are_curve_point(zero)
		);

		// Confirm `Fe51::canonical` uniquely normalizes limb-1 carry states
		// (`h[1] == 2^51` when value < p, and all-MASK wrap carry).
		assert!(Fe51([0, 1_u64 << 51, 0, 0, 0]).eq_canonical(Fe51([0, 0, 1, 0, 0])));
		assert!(
			Fe51([Fe51::MASK, 1_u64 << 51, Fe51::MASK, Fe51::MASK, Fe51::MASK])
				.eq_canonical(Fe51([18, 1, 0, 0, 0]))
		);
	}
}
