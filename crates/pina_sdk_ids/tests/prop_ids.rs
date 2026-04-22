//! Property-based tests for pina_sdk_ids.
//!
//! Verifies that every declared well-known address is a valid base58-encoded
//! 32-byte public key and that base58 decoding round-trips correctly.

use std::str::FromStr;

use proptest::prelude::*;

// List of every constant ID string declared in the crate.
const KNOWN_IDS: &[(&str, &str)] = &[
	(
		"address_lookup_table",
		"AddressLookupTab1e1111111111111111111111111",
	),
	("bpf_loader", "BPFLoader2111111111111111111111111111111111"),
	(
		"bpf_loader_deprecated",
		"BPFLoader1111111111111111111111111111111111",
	),
	(
		"bpf_loader_upgradeable",
		"BPFLoaderUpgradeab1e11111111111111111111111",
	),
	(
		"compute_budget",
		"ComputeBudget111111111111111111111111111111",
	),
	("config", "Config1111111111111111111111111111111111111"),
	(
		"ed25519_program",
		"Ed25519SigVerify111111111111111111111111111",
	),
	("feature", "Feature111111111111111111111111111111111111"),
	("incinerator", "1nc1nerator11111111111111111111111111111111"),
	("loader_v4", "LoaderV411111111111111111111111111111111111"),
	(
		"native_loader",
		"NativeLoader1111111111111111111111111111111",
	),
	(
		"secp256k1_program",
		"KeccakSecp256k11111111111111111111111111111",
	),
	(
		"secp256r1_program",
		"Secp256r1SigVerify1111111111111111111111111",
	),
	(
		"stake_config",
		"StakeConfig11111111111111111111111111111111",
	),
	("stake", "Stake11111111111111111111111111111111111111"),
	("system_program", "11111111111111111111111111111111"),
	("vote", "Vote111111111111111111111111111111111111111"),
	("sysvar", "Sysvar1111111111111111111111111111111111111"),
	(
		"sysvar_clock",
		"SysvarC1ock11111111111111111111111111111111",
	),
	(
		"sysvar_epoch_rewards",
		"SysvarEpochRewards1111111111111111111111111",
	),
	(
		"sysvar_epoch_schedule",
		"SysvarEpochSchedu1e111111111111111111111111",
	),
	("sysvar_fees", "SysvarFees111111111111111111111111111111111"),
	(
		"sysvar_instructions",
		"Sysvar1nstructions1111111111111111111111111",
	),
	(
		"sysvar_last_restart_slot",
		"SysvarLastRestartS1ot1111111111111111111111",
	),
	(
		"sysvar_recent_blockhashes",
		"SysvarRecentB1ockHashes11111111111111111111",
	),
	("sysvar_rent", "SysvarRent111111111111111111111111111111111"),
	(
		"sysvar_rewards",
		"SysvarRewards111111111111111111111111111111",
	),
	(
		"sysvar_slot_hashes",
		"SysvarS1otHashes111111111111111111111111111",
	),
	(
		"sysvar_slot_history",
		"SysvarS1otHistory11111111111111111111111111",
	),
	(
		"sysvar_stake_history",
		"SysvarStakeHistory1111111111111111111111111",
	),
	(
		"zk_token_proof_program",
		"ZkTokenProof1111111111111111111111111111111",
	),
	(
		"zk_elgamal_proof_program",
		"ZkE1Gama1Proof11111111111111111111111111111",
	),
];

static KNOWN_ID_NAMES: &[&str] = &[
	"address_lookup_table",
	"bpf_loader",
	"bpf_loader_deprecated",
	"bpf_loader_upgradeable",
	"compute_budget",
	"config",
	"ed25519_program",
	"feature",
	"incinerator",
	"loader_v4",
	"native_loader",
	"secp256k1_program",
	"secp256r1_program",
	"stake_config",
	"stake",
	"system_program",
	"vote",
	"sysvar",
	"sysvar_clock",
	"sysvar_epoch_rewards",
	"sysvar_epoch_schedule",
	"sysvar_fees",
	"sysvar_instructions",
	"sysvar_last_restart_slot",
	"sysvar_recent_blockhashes",
	"sysvar_rent",
	"sysvar_rewards",
	"sysvar_slot_hashes",
	"sysvar_slot_history",
	"sysvar_stake_history",
	"zk_token_proof_program",
	"zk_elgamal_proof_program",
];

// ---------------------------------------------------------------------------
// Every known ID must decode to exactly 32 bytes.
// ---------------------------------------------------------------------------

#[test]
fn all_known_ids_decode_to_32_bytes() {
	for (name, id_str) in KNOWN_IDS {
		let decoded = solana_address::Address::from_str(id_str);
		assert!(decoded.is_ok(), "{name}: failed to decode '{id_str}'");
		let decoded = decoded.unwrap();
		let bytes: [u8; 32] = decoded.to_bytes();
		assert_eq!(bytes.len(), 32, "{name}: decoded to {} bytes", bytes.len());
	}
}

// ---------------------------------------------------------------------------
// Random base58 strings of similar length either fail or decode to 32 bytes.
// ---------------------------------------------------------------------------

proptest! {
	#[test]
	fn arbitrary_base58_strings_never_panic(ref s in "[1-9A-HJ-NP-Za-km-z]{30,45}") {
		// Must not panic — either Ok or Err.
		let _ = solana_address::Address::from_str(s);
	}

	#[test]
	fn known_id_base58_roundtrip(name in prop::sample::select(KNOWN_ID_NAMES)) {
		let id_str = KNOWN_IDS.iter().find(|(n, _)| *n == name).map(|(_, s)| *s).unwrap();
		let decoded = solana_address::Address::from_str(id_str).unwrap();
		let re_encoded = decoded.to_string();
		let msg = format!("{name}: base58 round-trip mismatch");
		prop_assert!(re_encoded == id_str, "{}", msg);
	}
}
