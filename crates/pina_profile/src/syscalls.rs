//! Solana syscall names and the call keys SBF `call` instructions carry.
//!
//! The SBPF v0 loader relocates every `call` in `.text` before execution. A
//! syscall site becomes `call <murmur3(symbol name)>` and an internal call
//! becomes `call <murmur3(target pc as 8 little-endian bytes)>`, both hashed
//! with 32-bit murmur3 (x86 variant, seed 0). Register traces record the
//! relocated instructions, so these keys identify syscalls by name.

/// Every syscall the Agave runtime registers for SBF programs.
pub const KNOWN_SYSCALLS: &[&str] = &[
	"abort",
	"sol_alloc_free_",
	"sol_alt_bn128_compression",
	"sol_alt_bn128_group_op",
	"sol_big_mod_exp",
	"sol_blake3",
	"sol_create_program_address",
	"sol_curve_decompress",
	"sol_curve_group_op",
	"sol_curve_multiscalar_mul",
	"sol_curve_pairing_map",
	"sol_curve_validate_point",
	"sol_get_clock_sysvar",
	"sol_get_epoch_rewards_sysvar",
	"sol_get_epoch_schedule_sysvar",
	"sol_get_epoch_stake",
	"sol_get_fees_sysvar",
	"sol_get_last_restart_slot",
	"sol_get_processed_sibling_instruction",
	"sol_get_rent_sysvar",
	"sol_get_return_data",
	"sol_get_stack_height",
	"sol_get_sysvar",
	"sol_invoke_signed_c",
	"sol_invoke_signed_rust",
	"sol_keccak256",
	"sol_log_",
	"sol_log_64_",
	"sol_log_compute_units_",
	"sol_log_data",
	"sol_log_pubkey",
	"sol_memcmp_",
	"sol_memcpy_",
	"sol_memmove_",
	"sol_memset_",
	"sol_panic_",
	"sol_poseidon",
	"sol_remaining_compute_units",
	"sol_secp256k1_recover",
	"sol_set_return_data",
	"sol_sha256",
	"sol_sha512",
	"sol_try_find_program_address",
];

/// 32-bit murmur3 (x86 variant, seed 0), the hash SBPF uses for call keys.
#[must_use]
pub fn murmur3_32(bytes: &[u8]) -> u32 {
	const C1: u32 = 0xcc9e_2d51;
	const C2: u32 = 0x1b87_3593;

	let mut hash = 0_u32;
	let (blocks, tail) = bytes.as_chunks::<4>();

	for block in blocks {
		let k = u32::from_le_bytes(*block)
			.wrapping_mul(C1)
			.rotate_left(15)
			.wrapping_mul(C2);
		hash = (hash ^ k)
			.rotate_left(13)
			.wrapping_mul(5)
			.wrapping_add(0xe654_6b64);
	}

	if !tail.is_empty() {
		let k = tail
			.iter()
			.rev()
			.fold(0_u32, |k, byte| (k << 8) | u32::from(*byte));
		hash ^= k.wrapping_mul(C1).rotate_left(15).wrapping_mul(C2);
	}

	// murmur3 mixes in the length modulo 2^32 by definition.
	hash ^= bytes.len() as u32;
	hash ^= hash >> 16;
	hash = hash.wrapping_mul(0x85eb_ca6b);
	hash ^= hash >> 13;
	hash = hash.wrapping_mul(0xc2b2_ae35);
	hash ^ (hash >> 16)
}

/// Name the syscall a relocated `call` immediate refers to.
#[must_use]
pub fn syscall_name(key: u32) -> Option<&'static str> {
	KNOWN_SYSCALLS
		.iter()
		.copied()
		.find(|name| murmur3_32(name.as_bytes()) == key)
}

/// The key the v0 loader writes into an internal `call` to `pc`.
#[must_use]
pub fn internal_call_key(pc: u64) -> u32 {
	murmur3_32(&pc.to_le_bytes())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn murmur3_matches_reference_vectors() {
		// Reference murmur3_x86_32 outputs with seed 0, covering every tail length.
		assert_eq!(murmur3_32(b""), 0);
		assert_eq!(murmur3_32(b"a"), 0x3c25_69b2);
		assert_eq!(murmur3_32(b"ab"), 0x9bbf_d75f);
		assert_eq!(murmur3_32(b"abc"), 0xb3dd_93fa);
		assert_eq!(murmur3_32(b"abcd"), 0x43ed_676a);
		assert_eq!(murmur3_32(b"Hello, world!"), 0xc036_3e43);
	}

	#[test]
	fn syscall_keys_match_the_sbpf_loader() {
		// Keys the SBPF loader registers, as published in the Solana SDK's
		// syscall definitions.
		assert_eq!(murmur3_32(b"entrypoint"), 0x71e3_cf81);
		assert_eq!(murmur3_32(b"abort"), 0xb6fc_1a11);
		assert_eq!(murmur3_32(b"sol_panic_"), 0x6860_93bb);
		assert_eq!(murmur3_32(b"sol_log_"), 0x2075_59bd);
		assert_eq!(murmur3_32(b"sol_invoke_signed_c"), 0xa22b_9c85);
	}

	#[test]
	fn syscall_name_resolves_known_keys_only() {
		assert_eq!(syscall_name(0x2075_59bd), Some("sol_log_"));
		assert_eq!(syscall_name(murmur3_32(b"sol_sha256")), Some("sol_sha256"));
		assert_eq!(syscall_name(0), None);
		assert_eq!(syscall_name(internal_call_key(42)), None);
	}

	#[test]
	fn known_syscall_keys_are_unique() {
		let mut keys: Vec<u32> = KNOWN_SYSCALLS
			.iter()
			.map(|name| murmur3_32(name.as_bytes()))
			.collect();
		keys.sort_unstable();
		keys.dedup();

		assert_eq!(keys.len(), KNOWN_SYSCALLS.len());
	}
}
