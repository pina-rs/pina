//! A programmable, tiered-disclosure privacy pool built with pina.
//!
//! Combines Helius Solana Privacy Rings' production architecture with Pina's
//! governed, publicly-logged disclosure state machine — all inside one
//! `no_std` crate with no `unsafe` code outside `syscalls.rs` and zero
//! required off-chain infrastructure:
//!
//! - **Per-nullifier PDAs ([`NullifierRecord`])** — $O(1)$ double-spend
//!   detection, unbounded spend capacity, and zero shared nullifier write-lock
//!   contention. Dummy inputs (`nullifier == [0; 32]`) skip PDA creation.
//! - **Depth-16 frontier Merkle tree ([`MerkleTree`])** — holds 65,536 leaves
//!   in a 512-byte frontier plus a 32-root ring using compile-time
//!   [`ZERO_HASHES`], eliminating runtime zero-hash recomputation on
//!   initialization.
//! - **Variable-amount 2-in-2-out join-split circuit** — one unified Groth16
//!   circuit (`VK_SLOT_JOIN_SPLIT`) handles arbitrary-amount withdrawals,
//!   change outputs, UTXO splits, and UTXO merges with 64-bit range checks and
//!   single-input `statement_hash` compression (one `g1_mul` + `g1_add`).
//! - **SPL Token & Token-2022 escrows ([`AssetVault`])** — permissionless
//!   per-mint asset registration, variable-amount token deposits, and
//!   join-split token withdrawals.
//! - **128-byte compressed Groth16 proofs** — decompressed on-chain via
//!   `sol_alt_bn128_compression` before the four-pairing check.
//! - **Relayer & fee-payer decoupling** — optional relayer address and fee
//!   bound into the Groth16 statement so users can pay fees out of shielded
//!   balances without leaking a public fee payer.
//! - **Real shielded keypair & X25519 + AEAD (`shielded` module)** — host-side
//!   RFC 7748 X25519 viewing keys, per-transaction viewing keys for selective
//!   disclosure, and 2-of-3 GF(256) Shamir custodian shares.
//! - **Rent-free note emission & privacy-preserving [`DisclosureLog`]** —
//!   encrypted note envelopes and shares emit via [`NoteEncryptedEvent`] with
//!   zero per-note PDA rent; [`RequestDisclosureIx`] authenticates
//!   `view_pubkey` via Merkle membership; [`DisclosureLog`] stores a salted
//!   `scope_hash` rather than raw commitments; verifying keys support authority
//!   rotation.
//! - **Custom Ring policy & composable CPI** — configurable pause and
//!   co-signer / CPI authority gates ([`SetPolicyIx`]) plus composable ZK
//!   policy verification ([`VerifyPolicyIx`] on [`VK_SLOT_POLICY`]).

#![allow(missing_docs)]
#![allow(clippy::inline_always)]
#![expect(clippy::trivially_copy_pass_by_ref)]
#![cfg_attr(not(test), no_std)]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

pub use pina::Address;
use pina::*;

declare_id!("DGHJjbUsSzAiSypH4dupxkQK1WLVcevvYchmM7mNLn9D");

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const SEED_POOL_CONFIG: &[u8] = b"privacy-pool-config";
const SEED_VAULT: &[u8] = b"privacy-pool-vault";
const SEED_MERKLE_TREE: &[u8] = b"privacy-pool-tree";
const SEED_NULLIFIER: &[u8] = b"privacy-pool-nullifier";
const SEED_CUSTODIAN_REGISTRY: &[u8] = b"privacy-pool-custodians";
const SEED_REQUESTER_REGISTRY: &[u8] = b"privacy-pool-requesters";
const SEED_ASSET_VAULT: &[u8] = b"privacy-pool-asset";
const SEED_REQUEST: &[u8] = b"privacy-pool-request";
const SEED_LOG: &[u8] = b"privacy-pool-log";
const SEED_VKEY: &[u8] = b"privacy-pool-vkey";

/// Loader that owns the deployment's `ProgramData` and upgrade authority.
const UPGRADEABLE_LOADER: Address = address!("BPFLoaderUpgradeab1e11111111111111111111111");

/// Accepted SPL token programs for [`AssetVault`] registration.
const SPL_PROGRAM_IDS: [Address; 2] = [token::ID, token_2022::ID];

/// Frontier Merkle tree depth: `2^16 = 65,536` leaves in 512 bytes of frontier.
pub const TREE_DEPTH: usize = 16;

/// Leaves the tree can hold before it is full: `2^depth`.
pub const TREE_CAPACITY: usize = 1 << TREE_DEPTH;

/// Flat byte length of the frontier (`TREE_DEPTH * 32`).
pub const FRONTIER_BYTES: usize = TREE_DEPTH * 32;

/// Recent roots retained for proof freshness.
pub const ROOT_RING_CAPACITY: usize = 32;

/// Flat byte length of the root ring (`ROOT_RING_CAPACITY * 32`).
pub const ROOT_RING_BYTES: usize = ROOT_RING_CAPACITY * 32;

/// Flat byte length of a depth-16 Merkle witness (`TREE_DEPTH * 32`).
pub const MERKLE_PATH_BYTES: usize = TREE_DEPTH * 32;

/// Custodians on the disclosure committee.
pub const CUSTODIAN_COUNT: usize = 3;

/// Default custodian approvals required to execute a disclosure.
pub const DEFAULT_CUSTODIAN_THRESHOLD: u16 = 2;

/// Default tier-1 challenge window in seconds.
pub const DEFAULT_CHALLENGE_WINDOW_SECS: u32 = 600;

/// Registered disclosure requesters the registry can hold.
pub const MAX_REQUESTERS: usize = 16;

/// Encrypted note plaintext emitted per note (fits an 88-byte X25519 + AEAD
/// envelope: 32B ephemeral pubkey + 40B ciphertext + 16B MAC).
pub const MAX_ENVELOPE_BYTES: usize = 96;

/// Per-custodian encrypted key shares: three 48-byte X25519 + AEAD Shamir slots.
pub const SHARE_BLOB_BYTES: usize = 48 * CUSTODIAN_COUNT;

/// Encrypted subject-notice blob stored on a disclosure request.
pub const MAX_NOTICE_BYTES: usize = 96;

/// Disclosure log entries the append-only log can hold.
pub const LOG_CAPACITY: usize = 32;

/// One serialized log entry: requester (32), salted scope hash (32), tier (1),
/// padding (7), nonce (8), timestamp (8), padding (8).
pub const LOG_ENTRY_BYTES: usize = 96;

/// Flat byte length of the log store.
pub const LOG_BYTES: usize = LOG_CAPACITY * LOG_ENTRY_BYTES;

/// Verifying-key slot for the unified 2-in-2-out join-split circuit.
pub const VK_SLOT_JOIN_SPLIT: u8 = 0;

/// Verifying-key slot for optional Custom Ring ZK policy proofs.
pub const VK_SLOT_POLICY: u8 = 1;

/// Policy flag: pause all deposits, transfers, and withdrawals.
pub const POLICY_PAUSED: u8 = 1 << 0;

/// Policy flag: require `PoolConfig::policy_cosigner` to sign every spend.
pub const POLICY_REQUIRE_COSIGNER: u8 = 1 << 1;

/// Disclosure tiers.
pub const TIER_CONSENT: u8 = 0;
pub const TIER_VERIFIED: u8 = 1;
pub const TIER_COMPELLED: u8 = 2;

/// Disclosure request statuses.
pub const STATUS_PENDING: u8 = 0;
pub const STATUS_CHALLENGED: u8 = 1;
pub const STATUS_RESOLVED: u8 = 2;
pub const STATUS_REJECTED: u8 = 3;
pub const STATUS_EXECUTED: u8 = 4;
pub const STATUS_CANCELLED: u8 = 5;

/// Lamports the inline `Migrate` path may spend on reallocs in one transaction.
const MAX_INLINE_MIGRATION_LAMPORTS: u64 = 64 * 1024 * 1_000_000;

/// Schema array lengths must be integer literals for the ABI parser; these
/// assertions bind the literals to the named constants.
const _: () = assert!(FRONTIER_BYTES == 512);
const _: () = assert!(ROOT_RING_BYTES == 1024);
const _: () = assert!(MERKLE_PATH_BYTES == 512);
const _: () = assert!(MAX_ENVELOPE_BYTES == 96);
const _: () = assert!(SHARE_BLOB_BYTES == 144);
const _: () = assert!(MAX_NOTICE_BYTES == 96);
const _: () = assert!(LOG_BYTES == 3072);

/// Precomputed Poseidon-2 zero-hash chain for empty subtrees (levels `0..=16`,
/// BN254 circom x5, little-endian). Embedding this table eliminates 16 runtime
/// `sol_poseidon` syscalls during `Initialize` and `insert_leaf`.
pub const ZERO_HASHES: [[u8; 32]; TREE_DEPTH + 1] = [
	[0_u8; 32],
	[
		100, 72, 182, 70, 132, 238, 57, 168, 35, 213, 254, 95, 213, 36, 49, 220, 129, 228, 129,
		123, 242, 195, 234, 60, 171, 158, 35, 158, 251, 245, 152, 32,
	],
	[
		225, 241, 177, 96, 68, 119, 164, 103, 240, 141, 198, 157, 203, 68, 26, 38, 236, 167, 132,
		245, 111, 26, 48, 223, 99, 34, 177, 205, 61, 103, 105, 16,
	],
	[
		56, 210, 86, 184, 178, 126, 213, 40, 213, 29, 55, 80, 234, 110, 124, 70, 6, 33, 247, 80,
		141, 117, 61, 46, 175, 226, 126, 83, 49, 51, 244, 24,
	],
	[
		42, 149, 188, 157, 85, 151, 172, 202, 101, 130, 86, 26, 87, 40, 183, 241, 69, 35, 165, 59,
		233, 255, 32, 99, 211, 176, 23, 203, 55, 216, 249, 7,
	],
	[
		85, 63, 24, 57, 22, 236, 92, 123, 77, 173, 178, 148, 140, 197, 153, 166, 7, 41, 243, 93,
		76, 31, 99, 201, 245, 179, 70, 135, 94, 207, 148, 43,
	],
	[
		120, 157, 160, 46, 163, 221, 17, 29, 97, 83, 185, 81, 105, 30, 215, 254, 188, 225, 169,
		204, 34, 125, 234, 70, 150, 69, 102, 166, 197, 147, 238, 45,
	],
	[
		157, 52, 135, 60, 190, 170, 164, 168, 127, 172, 181, 140, 168, 21, 5, 139, 123, 89, 57,
		182, 30, 96, 207, 130, 233, 132, 43, 162, 229, 149, 130, 7,
	],
	[
		97, 204, 243, 153, 58, 190, 76, 68, 26, 33, 65, 74, 39, 46, 107, 97, 42, 71, 100, 69, 134,
		236, 27, 80, 166, 39, 96, 143, 241, 229, 165, 47,
	],
	[
		71, 215, 252, 20, 166, 86, 33, 62, 171, 40, 226, 227, 204, 122, 94, 228, 102, 31, 148, 158,
		56, 128, 183, 236, 33, 253, 216, 208, 118, 67, 136, 14,
	],
	[
		242, 10, 25, 218, 229, 117, 97, 222, 51, 53, 113, 87, 249, 146, 88, 249, 105, 180, 46, 165,
		209, 122, 113, 40, 30, 79, 73, 114, 218, 1, 114, 27,
	],
	[
		54, 118, 125, 206, 250, 107, 188, 190, 181, 8, 8, 101, 228, 225, 230, 166, 25, 152, 36, 1,
		178, 192, 0, 82, 56, 54, 94, 114, 34, 136, 141, 31,
	],
	[
		90, 248, 181, 113, 4, 154, 135, 208, 168, 136, 207, 42, 161, 176, 98, 97, 251, 252, 140,
		186, 137, 21, 112, 185, 175, 75, 145, 108, 246, 130, 93, 44,
	],
	[
		208, 191, 191, 224, 112, 242, 88, 100, 100, 244, 19, 161, 170, 196, 245, 78, 19, 161, 63,
		223, 90, 127, 149, 32, 184, 11, 148, 160, 72, 65, 197, 20,
	],
	[
		12, 232, 235, 244, 75, 142, 17, 22, 212, 137, 173, 140, 88, 37, 190, 17, 175, 185, 216, 68,
		238, 192, 16, 30, 150, 111, 152, 47, 177, 51, 13, 25,
	],
	[
		146, 108, 224, 37, 147, 100, 179, 165, 10, 81, 175, 150, 101, 174, 103, 17, 237, 115, 173,
		20, 73, 53, 23, 172, 82, 65, 112, 206, 169, 138, 249, 34,
	],
	[
		35, 115, 186, 139, 211, 83, 183, 248, 238, 204, 110, 198, 41, 111, 82, 90, 87, 106, 191,
		114, 141, 34, 111, 159, 11, 136, 229, 108, 155, 124, 124, 42,
	],
];

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyPoolError {
	/// The signer is not the pool authority.
	InvalidAuthority = 0,
	/// The commitment is the zero field element.
	ZeroCommitment = 1,
	/// The deposit or withdrawal amount is zero.
	ZeroAmount = 2,
	/// The Merkle tree is full.
	TreeFull = 3,
	/// The submitted root is not in the recent-root ring.
	UnknownRoot = 4,
	/// The nullifier has already been spent.
	NullifierAlreadySpent = 5,
	/// The pool is currently paused by policy.
	PoolPaused = 6,
	/// The Groth16 proof failed verification.
	ProofVerificationFailed = 7,
	/// The verifying key slot has not been installed.
	VerifyingKeyMissing = 8,
	/// The verifying key slot identifier is invalid.
	InvalidVerifyingKeySlot = 9,
	/// A custodian slot address is zero or duplicated.
	InvalidCustodianSet = 10,
	/// The signer is not a registered custodian.
	NotACustodian = 11,
	/// The custodian has already approved this request.
	AlreadyApproved = 12,
	/// The disclosure tier is unknown.
	InvalidTier = 13,
	/// The requester is not registered for this tier.
	RequesterNotEntitled = 14,
	/// The requester registry is full.
	RequesterRegistryFull = 15,
	/// The submitted Merkle membership proof does not match any known root.
	InvalidMerkleProof = 16,
	/// The request status does not allow this transition.
	InvalidRequestStatus = 17,
	/// Consent (tier 0) has not been granted by the note's view key.
	ConsentRequired = 18,
	/// The tier-1 challenge window is still open.
	ChallengeWindowOpen = 19,
	/// Only the note's view key may perform this action.
	NotNoteViewer = 20,
	/// The disclosure log is full.
	LogFull = 21,
	/// A slice operation went out of bounds.
	BufferOverflow = 22,
	/// A primitive failed its own bounds check.
	ArithmeticOverflow = 23,
	/// The required Custom Ring policy co-signer did not sign.
	PolicyCosignerRequired = 24,
	/// The token account or mint does not match the registered asset vault.
	InvalidAssetVault = 25,
}

// ---------------------------------------------------------------------------
// Discriminators
// ---------------------------------------------------------------------------

#[discriminator(
	entrypoint,
	migrations(
		PoolConfig,
		MerkleTree,
		NullifierRecord,
		CustodianRegistry,
		RequesterRegistry,
		DisclosureLog,
		AssetVault,
		DisclosureRequest,
		VerifyingKeyAccount
	),
	migrations_max_lamports = MAX_INLINE_MIGRATION_LAMPORTS,
	inline = "hint"
)]
pub enum PrivacyPoolInstruction {
	Initialize = 0,
	SetVerificationKey = 1,
	SetCustodians = 2,
	RegisterRequester = 3,
	Deposit = 4,
	Withdraw = 5,
	Transfer = 6,
	RequestDisclosure = 7,
	GrantDisclosure = 8,
	ChallengeDisclosure = 9,
	ResolveChallenge = 10,
	ApproveDisclosure = 11,
	CancelDisclosure = 12,
	RegisterAsset = 13,
	DepositToken = 14,
	WithdrawToken = 15,
	SetPolicy = 16,
	VerifyPolicy = 17,
}

#[discriminator]
pub enum PrivacyPoolAccountType {
	PoolConfig = 1,
	PoolVault = 2,
	MerkleTree = 3,
	NullifierRecord = 4,
	CustodianRegistry = 5,
	RequesterRegistry = 6,
	DisclosureLog = 7,
	AssetVault = 8,
	DisclosureRequest = 9,
	VerifyingKeyAccount = 10,
}

#[discriminator]
pub enum PrivacyPoolEventType {
	NoteEncryptedEvent = 1,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Rent-free encrypted note emission. Clients and indexers reconstruct the
/// Merkle tree and decrypt incoming notes from `Program data:` logs without
/// locking rent in per-note PDAs.
#[event(discriminator = PrivacyPoolEventType)]
pub struct NoteEncryptedEvent {
	pub leaf_index: u64,
	pub commitment: [u8; 32],
	pub view_pubkey: [u8; 32],
	pub envelope_len: u8,
	pub envelope: [u8; 96],
	pub shares: [u8; 144],
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Global pool configuration, governance root, and Custom Ring policy flags.
#[account(discriminator = PrivacyPoolAccountType)]
#[pda(seeds = [SEED_POOL_CONFIG], bump = bump)]
pub struct PoolConfig {
	pub bump: u8,
	/// Governance key that rotates custodians, requesters, verifying keys, and
	/// Custom Ring policy settings.
	pub authority: Address,
	/// Custodian approvals required to execute a disclosure.
	pub custodian_threshold: u16,
	/// Tier-1 challenge window length in seconds.
	pub challenge_window_secs: u32,
	/// Custom Ring policy bitmask (`POLICY_PAUSED`, `POLICY_REQUIRE_COSIGNER`).
	pub policy_flags: u8,
	/// Required co-signer or Custom Ring program PDA when
	/// `POLICY_REQUIRE_COSIGNER` is active.
	pub policy_cosigner: Address,
	/// Lifetime deposit count, for analysis only.
	pub total_deposits: u64,
}

/// The pool's native SOL custody PDA.
#[account(discriminator = PrivacyPoolAccountType)]
#[pda(seeds = [SEED_VAULT], bump = bump)]
pub struct PoolVault {
	pub bump: u8,
}

/// Depth-16 frontier incremental Merkle tree over Poseidon-2 hashes.
///
/// Holds up to 65,536 leaves in 512 bytes of `filled_subtrees` plus a 32-root
/// ring, avoiding the 8 KB full-node layout and runtime zero-hash prefill.
#[account(discriminator = PrivacyPoolAccountType)]
#[pda(seeds = [SEED_MERKLE_TREE], bump = bump)]
pub struct MerkleTree {
	pub bump: u8,
	/// Number of leaves written so far.
	pub next_leaf_index: u64,
	/// Frontier of rightmost filled subtrees, one 32-byte hash per level.
	pub filled_subtrees: [u8; 512],
	/// Roots currently in the ring; evictions hold it at capacity.
	pub roots_len: u16,
	/// Ring of 32 recent roots, oldest first.
	pub roots: [u8; 1024],
}

/// Per-nullifier spent marker PDA. Existence proves the nullifier was spent in
/// $O(1)$ without a global nullifier write-lock hotspot or fixed capacity cap.
#[account(discriminator = PrivacyPoolAccountType)]
#[pda(seeds = [SEED_NULLIFIER, nullifier: Address], bump = bump)]
pub struct NullifierRecord {
	pub bump: u8,
}

/// The disclosure committee.
#[account(discriminator = PrivacyPoolAccountType)]
#[pda(seeds = [SEED_CUSTODIAN_REGISTRY], bump = bump)]
pub struct CustodianRegistry {
	pub bump: u8,
	/// Committee members as concatenated 32-byte addresses.
	pub custodians: [u8; 96],
}

/// Entities allowed to file tier-1 and tier-2 disclosure requests.
#[account(discriminator = PrivacyPoolAccountType)]
#[pda(seeds = [SEED_REQUESTER_REGISTRY], bump = bump)]
pub struct RequesterRegistry {
	pub bump: u8,
	pub count: u8,
	pub keys: [u8; 512],
	pub max_tiers: [u8; 16],
}

/// Per-mint SPL Token / Token-2022 escrow metadata (mirroring SPR's SPL
/// Interface PDA).
#[account(discriminator = PrivacyPoolAccountType)]
#[pda(seeds = [SEED_ASSET_VAULT, mint: Address], bump = bump)]
pub struct AssetVault {
	pub bump: u8,
	pub mint: Address,
	pub token_program: Address,
	pub vault_token_account: Address,
}

/// One disclosure request and its state machine. Stores `view_pubkey` after
/// `RequestDisclosure` authenticates it against the Merkle tree so
/// `GrantDisclosure` and `ChallengeDisclosure` require no per-note PDA.
#[account(discriminator = PrivacyPoolAccountType)]
#[pda(seeds = [SEED_REQUEST, requester: Address, nonce: u64], bump = bump)]
pub struct DisclosureRequest {
	pub bump: u8,
	pub requester: Address,
	pub nonce: u64,
	pub tier: u8,
	pub commitment: [u8; 32],
	pub view_pubkey: [u8; 32],
	pub notice_len: u8,
	pub notice: [u8; 96],
	pub legal_basis_hash: [u8; 32],
	pub created_at: u64,
	pub challenge_deadline: u64,
	pub status: u8,
	pub granted: u8,
	pub approvals: u8,
}

/// Append-only public record of executed disclosures. Stores a salted
/// `scope_hash` rather than raw note commitments so observers cannot link log
/// entries back to shielded notes.
#[account(discriminator = PrivacyPoolAccountType)]
#[pda(seeds = [SEED_LOG], bump = bump)]
pub struct DisclosureLog {
	pub bump: u8,
	pub count: u64,
	pub entries: [u8; 3072],
}

/// Single-input Groth16 verifying key. Every circuit compresses its statement
/// into one BN254 scalar (`statement_hash`), so only `ic0` and `ic1` are
/// stored (saving 128 bytes per key and 2+ `g1_mul` syscalls per spend).
#[account(discriminator = PrivacyPoolAccountType)]
#[pda(seeds = [SEED_VKEY, slot: u64], bump = bump)]
pub struct VerifyingKeyAccount {
	pub bump: u8,
	pub slot: u8,
	pub alpha_g1: [u8; 64],
	pub beta_g2: [u8; 128],
	pub gamma_g2: [u8; 128],
	pub delta_g2: [u8; 128],
	pub ic0: [u8; 64],
	pub ic1: [u8; 64],
}

// ---------------------------------------------------------------------------
// Poseidon, Groth16, and Shielded Keypair modules
// ---------------------------------------------------------------------------

mod crypto;
#[cfg(all(feature = "prover", not(target_os = "solana")))]
pub mod shielded;
mod syscalls;

pub use crypto::Groth16Key;
pub use crypto::poseidon2;
pub use crypto::verify_groth16_compressed;
#[cfg(any(test, feature = "prover"))]
pub use crypto::zero_hashes;

/// Hash a 32-byte address or pubkey into one canonical BN254 scalar without
/// modular aliasing: `Poseidon(lo128(addr), hi128(addr))`.
pub fn address_hash(bytes: &[u8; 32]) -> Result<[u8; 32], ProgramError> {
	let mut low = [0_u8; 32];
	let mut high = [0_u8; 32];
	low[..16].copy_from_slice(&bytes[..16]);
	high[..16].copy_from_slice(&bytes[16..]);
	poseidon2(&low, &high)
}

/// Canonical asset identifier for note commitments: `[0; 32]` for native SOL,
/// or `address_hash(mint)` for SPL Token / Token-2022 mints.
pub fn asset_id(mint: &Address) -> Result<[u8; 32], ProgramError> {
	if mint.as_ref() == [0_u8; 32] {
		return Ok([0_u8; 32]);
	}
	let mut bytes = [0_u8; 32];
	bytes.copy_from_slice(mint.as_ref());
	address_hash(&bytes)
}

/// Compute the tree leaf `Poseidon(commitment, address_hash(view_pubkey))`,
/// binding a note's consent/challenge key directly into the Merkle tree so no
/// per-note PDA is required.
pub fn note_leaf(commitment: &[u8; 32], view_pubkey: &[u8; 32]) -> Result<[u8; 32], ProgramError> {
	let vk_hash = address_hash(view_pubkey)?;
	poseidon2(commitment, &vk_hash)
}

/// Compute a deposit note commitment `Poseidon(note_id, Poseidon(asset, amount))`
/// on-chain so the credited commitment is cryptographically bound to the
/// escrowed `asset` and `amount`.
pub fn compute_note_commitment(
	note_id: &[u8; 32],
	asset: &[u8; 32],
	amount: u64,
) -> Result<[u8; 32], ProgramError> {
	let mut amt = [0_u8; 32];
	amt[..8].copy_from_slice(&amount.to_le_bytes());
	let asset_amt = poseidon2(asset, &amt)?;
	poseidon2(note_id, &asset_amt)
}

/// Pack `(public_in, public_out, relayer_fee)` losslessly into one little-endian
/// 192-bit BN254 scalar: `public_in + public_out * 2^64 + relayer_fee * 2^128`.
pub fn pack_amounts(public_in: u64, public_out: u64, relayer_fee: u64) -> [u8; 32] {
	let mut out = [0_u8; 32];
	out[0..8].copy_from_slice(&public_in.to_le_bytes());
	out[8..16].copy_from_slice(&public_out.to_le_bytes());
	out[16..24].copy_from_slice(&relayer_fee.to_le_bytes());
	out
}

/// Canonical 248-bit SHA-256 digest authenticating external transaction data:
/// recipient, relayer, mint, and both output notes' disclosure metadata.
#[allow(clippy::too_many_arguments)]
pub fn ext_data_digest(
	recipient: &Address,
	relayer: &Address,
	mint: &Address,
	out0_view_pubkey: &[u8; 32],
	out0_envelope_len: u8,
	out0_envelope: &[u8; MAX_ENVELOPE_BYTES],
	out0_shares: &[u8; SHARE_BLOB_BYTES],
	out1_view_pubkey: &[u8; 32],
	out1_envelope_len: u8,
	out1_envelope: &[u8; MAX_ENVELOPE_BYTES],
	out1_shares: &[u8; SHARE_BLOB_BYTES],
) -> Result<[u8; 32], ProgramError> {
	let env0 = out0_envelope
		.get(..usize::from(out0_envelope_len))
		.ok_or(PrivacyPoolError::BufferOverflow)?;
	let env1 = out1_envelope
		.get(..usize::from(out1_envelope_len))
		.ok_or(PrivacyPoolError::BufferOverflow)?;
	let mut digest = solana_sha256_hasher::hashv(&[
		b"pina:privacy-pool:ext-data:v2",
		recipient.as_ref(),
		relayer.as_ref(),
		mint.as_ref(),
		out0_view_pubkey,
		&[out0_envelope_len],
		env0,
		out0_shares,
		out1_view_pubkey,
		&[out1_envelope_len],
		env1,
		out1_shares,
	])
	.to_bytes();
	digest[31] = 0;
	Ok(digest)
}

/// Single public input for `JoinSplitCircuit`, compressing all public inputs
/// into one BN254 scalar so on-chain Groth16 verification needs only one
/// `g1_mul` + `g1_add`.
#[allow(clippy::too_many_arguments)]
pub fn join_split_statement_hash(
	root: &[u8; 32],
	nullifier_0: &[u8; 32],
	nullifier_1: &[u8; 32],
	out_commitment_0: &[u8; 32],
	out_commitment_1: &[u8; 32],
	ext_data_hash: &[u8; 32],
	asset_id: &[u8; 32],
	amounts_packed: &[u8; 32],
) -> Result<[u8; 32], ProgramError> {
	let h_in = poseidon2(nullifier_0, nullifier_1)?;
	let h_out = poseidon2(out_commitment_0, out_commitment_1)?;
	let h_meta = poseidon2(ext_data_hash, asset_id)?;
	let h_ext = poseidon2(&h_meta, amounts_packed)?;
	let left = poseidon2(root, &h_in)?;
	let right = poseidon2(&h_out, &h_ext)?;
	poseidon2(&left, &right)
}

/// Salted disclosure scope hash stored in [`DisclosureLog`] instead of raw
/// commitments so public audit logs do not deanonymize shielded notes.
pub fn disclosure_scope_hash(commitment: &[u8; 32], requester: &Address, nonce: u64) -> [u8; 32] {
	solana_sha256_hasher::hashv(&[
		b"pina:privacy-pool:disclosure-scope:v2",
		commitment,
		requester.as_ref(),
		&nonce.to_le_bytes(),
	])
	.to_bytes()
}

fn nullifier_address(nullifier: &[u8; 32]) -> Address {
	Address::new_from_array(*nullifier)
}

// ---------------------------------------------------------------------------
// Frontier Merkle tree, root ring, and log helpers
// ---------------------------------------------------------------------------

/// Prefill the frontier with the compile-time [`ZERO_HASHES`] (zero syscalls).
pub fn prefill_frontier(frontier: &mut [u8]) -> Result<(), ProgramError> {
	for (level, zero) in ZERO_HASHES.iter().take(TREE_DEPTH).enumerate() {
		let start = level * 32;
		frontier
			.get_mut(start..start + 32)
			.ok_or(PrivacyPoolError::BufferOverflow)?
			.copy_from_slice(zero);
	}
	Ok(())
}

/// Append a leaf to the frontier incremental Merkle tree and return the new
/// root.
pub fn insert_leaf(
	frontier: &mut [u8],
	next_leaf_index: u64,
	leaf: &[u8; 32],
) -> Result<[u8; 32], ProgramError> {
	let index = usize::try_from(next_leaf_index).map_err(|_| PrivacyPoolError::TreeFull)?;
	if index >= TREE_CAPACITY {
		return Err(PrivacyPoolError::TreeFull.into());
	}

	let mut current = *leaf;
	let mut position = index;
	for (level, zero) in ZERO_HASHES.iter().take(TREE_DEPTH).enumerate() {
		let start = level * 32;
		if position & 1 == 0 {
			frontier
				.get_mut(start..start + 32)
				.ok_or(PrivacyPoolError::BufferOverflow)?
				.copy_from_slice(&current);
			current = poseidon2(&current, zero)?;
		} else {
			let left_slice = frontier
				.get(start..start + 32)
				.ok_or(PrivacyPoolError::BufferOverflow)?;
			let mut left = [0_u8; 32];
			left.copy_from_slice(left_slice);
			current = poseidon2(&left, &current)?;
		}
		position >>= 1;
	}
	Ok(current)
}

/// Compute the root implied by a depth-16 Merkle path for `leaf` at
/// `leaf_index`.
pub fn compute_merkle_root(
	leaf: &[u8; 32],
	leaf_index: u64,
	path_elements: &[u8; MERKLE_PATH_BYTES],
) -> Result<[u8; 32], ProgramError> {
	if leaf_index >= TREE_CAPACITY as u64 {
		return Err(PrivacyPoolError::InvalidMerkleProof.into());
	}
	let mut current = *leaf;
	let mut position = leaf_index as usize;
	for level in 0..TREE_DEPTH {
		let start = level * 32;
		let mut sibling = [0_u8; 32];
		sibling.copy_from_slice(&path_elements[start..start + 32]);
		current = if position & 1 == 0 {
			poseidon2(&current, &sibling)?
		} else {
			poseidon2(&sibling, &current)?
		};
		position >>= 1;
	}
	Ok(current)
}

/// Append a root to the ring, evicting the oldest entry when full.
pub fn push_root(
	roots: &mut [u8],
	roots_len: usize,
	root: &[u8; 32],
) -> Result<usize, ProgramError> {
	debug_assert_eq!(roots.len(), ROOT_RING_BYTES);
	if roots_len >= ROOT_RING_CAPACITY {
		roots.copy_within(32.., 0);
		roots
			.get_mut((ROOT_RING_CAPACITY - 1) * 32..ROOT_RING_CAPACITY * 32)
			.ok_or(ProgramError::from(PrivacyPoolError::BufferOverflow))?
			.copy_from_slice(root);
		Ok(ROOT_RING_CAPACITY)
	} else {
		roots
			.get_mut(roots_len * 32..(roots_len + 1) * 32)
			.ok_or(ProgramError::from(PrivacyPoolError::BufferOverflow))?
			.copy_from_slice(root);
		Ok(roots_len + 1)
	}
}

/// Whether the root appears in the recent-root ring.
pub fn root_known(roots: &[u8], roots_len: usize, root: &[u8; 32]) -> bool {
	roots
		.get(..roots_len * 32)
		.unwrap_or(&[])
		.as_chunks::<32>()
		.0
		.iter()
		.any(|candidate| candidate == root.as_slice())
}

/// Encode one disclosure-log entry into `out`.
pub fn encode_log_entry(
	out: &mut [u8],
	requester: &Address,
	scope_hash: &[u8; 32],
	tier: u8,
	nonce: u64,
	timestamp: u64,
) -> Result<(), ProgramError> {
	let entry = out
		.get_mut(..LOG_ENTRY_BYTES)
		.ok_or(PrivacyPoolError::BufferOverflow)?;
	entry.fill(0);
	entry[..32].copy_from_slice(requester.as_ref());
	entry[32..64].copy_from_slice(scope_hash);
	entry[64] = tier;
	entry[72..80].copy_from_slice(&nonce.to_le_bytes());
	entry[80..88].copy_from_slice(&timestamp.to_le_bytes());
	Ok(())
}

fn clock_timestamp(clock: &AccountView) -> Result<u64, ProgramError> {
	Ok(
		u64::try_from(sysvars::clock::Clock::from_account_view(clock)?.unix_timestamp)
			.map_err(|_| PrivacyPoolError::ArithmeticOverflow)?,
	)
}

fn challenge_deadline(now: u64, window: u64, tier: u8) -> Result<u64, ProgramError> {
	if tier != TIER_VERIFIED {
		return Ok(now);
	}
	now.checked_add(window)
		.ok_or(PrivacyPoolError::ArithmeticOverflow.into())
}

fn validate_custodian_set(custodians: &[u8; 96]) -> Result<(), ProgramError> {
	for slot in custodians.as_chunks::<32>().0 {
		if slot == [0_u8; 32].as_slice() {
			return Err(PrivacyPoolError::InvalidCustodianSet.into());
		}
	}
	for (i, slot) in custodians.as_chunks::<32>().0.iter().enumerate() {
		if custodians
			.as_chunks::<32>()
			.0
			.iter()
			.skip(i + 1)
			.any(|other| other == slot)
		{
			return Err(PrivacyPoolError::InvalidCustodianSet.into());
		}
	}
	Ok(())
}

/// Enforce only the pause gate on deposits (keeping deposits open under
/// spend-only co-signer policies).
fn check_not_paused(pool_config: &AccountView) -> Result<(), ProgramError> {
	let config = pool_config.as_account::<PoolConfig>(&ID)?;
	let flags = config.policy_flags;
	drop(config);
	if flags & POLICY_PAUSED != 0 {
		return Err(PrivacyPoolError::PoolPaused.into());
	}
	Ok(())
}

/// Enforce Custom Ring policy flags on spends (`POLICY_PAUSED` and optional
/// `POLICY_REQUIRE_COSIGNER`).
fn check_policy(
	pool_config: &AccountView,
	cosigner: Option<&AccountView>,
) -> Result<(), ProgramError> {
	let config = pool_config.as_account::<PoolConfig>(&ID)?;
	let flags = config.policy_flags;
	let required_cosigner = config.policy_cosigner;
	drop(config);

	if flags & POLICY_PAUSED != 0 {
		return Err(PrivacyPoolError::PoolPaused.into());
	}
	if flags & POLICY_REQUIRE_COSIGNER != 0 {
		let signer = cosigner.ok_or(PrivacyPoolError::PolicyCosignerRequired)?;
		if !signer.is_signer() || signer.address() != &required_cosigner {
			return Err(PrivacyPoolError::PolicyCosignerRequired.into());
		}
	}
	Ok(())
}

/// Record a spent nullifier as an individual `NullifierRecord` PDA.
///
/// Dummy inputs carry `nullifier == [0; 32]` and skip PDA creation completely,
/// saving rent and compute units on 1-input spends.
fn record_nullifier(
	payer: &mut AccountView,
	nullifier_account: &mut AccountView,
	nullifier: &[u8; 32],
	bump: u8,
) -> ProgramResult {
	if *nullifier == [0_u8; 32] {
		return Ok(());
	}
	if nullifier_account.data_len() > 0 || nullifier_account.owned_by(&ID) {
		return Err(PrivacyPoolError::NullifierAlreadySpent.into());
	}
	CreateProgramAccountWithBump {
		account: nullifier_account,
		payer,
		owner: &ID,
		seeds: &NullifierRecord::seeds(&nullifier_address(nullifier)).as_slices(),
		bump,
	}
	.invoke_with::<NullifierRecord>(|record| {
		record.bump = bump;
		Ok(())
	})
}

/// Insert a non-zero output note into the frontier tree and emit its encrypted
/// note event (zero per-note PDA rent).
#[allow(clippy::too_many_arguments)]
fn insert_and_emit_note(
	merkle_tree: &mut AccountView,
	commitment: &[u8; 32],
	view_pubkey: &[u8; 32],
	envelope_len: u8,
	envelope: &[u8; MAX_ENVELOPE_BYTES],
	shares: &[u8; SHARE_BLOB_BYTES],
) -> ProgramResult {
	if *commitment == [0_u8; 32] {
		return Ok(());
	}
	let env_len = usize::from(envelope_len);
	if env_len > MAX_ENVELOPE_BYTES {
		return Err(PrivacyPoolError::BufferOverflow.into());
	}

	let leaf = note_leaf(commitment, view_pubkey)?;
	let leaf_index = {
		let mut tree = merkle_tree.as_account_mut::<MerkleTree>(&ID)?;
		let next_index = tree.next_leaf_index.get();
		if next_index >= TREE_CAPACITY as u64 {
			drop(tree);
			return Err(PrivacyPoolError::TreeFull.into());
		}
		let root = insert_leaf(tree.filled_subtrees.as_mut_slice(), next_index, &leaf)?;
		tree.next_leaf_index.set(next_index + 1);
		let ring_len = usize::from(tree.roots_len.get());
		let next_len = push_root(tree.roots.as_mut_slice(), ring_len, &root)?;
		tree.roots_len.set(next_len as u16);
		next_index
	};

	NoteEncryptedEvent::emit(|event| {
		event.leaf_index.set(leaf_index);
		event.commitment = *commitment;
		event.view_pubkey = *view_pubkey;
		event.envelope_len = envelope_len;
		event.envelope.as_mut_slice().fill(0);
		event.envelope.as_mut_slice()[..env_len].copy_from_slice(&envelope[..env_len]);
		event.shares = *shares;
		Ok(())
	})
}

// ---------------------------------------------------------------------------
// Instruction arguments
// ---------------------------------------------------------------------------

#[instruction(discriminator = PrivacyPoolInstruction::Initialize)]
pub struct InitializeIx {
	pub config_bump: u8,
	pub vault_bump: u8,
	pub tree_bump: u8,
	pub custodians_bump: u8,
	pub requesters_bump: u8,
	pub log_bump: u8,
	/// Initial three-member disclosure committee as concatenated 32-byte keys.
	pub custodians: [u8; 96],
}

#[instruction(discriminator = PrivacyPoolInstruction::SetVerificationKey)]
pub struct SetVerificationKeyIx {
	pub bump: u8,
	/// Circuit slot: [`VK_SLOT_JOIN_SPLIT`] or [`VK_SLOT_POLICY`].
	pub slot: u8,
	pub alpha_g1: [u8; 64],
	pub beta_g2: [u8; 128],
	pub gamma_g2: [u8; 128],
	pub delta_g2: [u8; 128],
	pub ic0: [u8; 64],
	pub ic1: [u8; 64],
}

#[instruction(discriminator = PrivacyPoolInstruction::SetCustodians)]
pub struct SetCustodiansIx {
	pub custodians: [u8; 96],
}

#[instruction(discriminator = PrivacyPoolInstruction::RegisterRequester)]
pub struct RegisterRequesterIx {
	pub requester: Address,
	pub max_tier: u8,
}

#[instruction(discriminator = PrivacyPoolInstruction::Deposit)]
pub struct DepositIx {
	/// Variable SOL deposit amount in lamports.
	pub amount: u64,
	/// Client-computed Poseidon note commitment.
	pub commitment: [u8; 32],
	/// Per-note consent/notification public key.
	pub view_pubkey: [u8; 32],
	pub envelope_len: u8,
	pub envelope: [u8; 96],
	pub shares: [u8; 144],
}

#[instruction(discriminator = PrivacyPoolInstruction::Withdraw)]
pub struct WithdrawIx {
	pub nullifier_0_bump: u8,
	pub nullifier_1_bump: u8,
	pub nullifier_0: [u8; 32],
	pub nullifier_1: [u8; 32],
	pub root: [u8; 32],
	/// SOL withdrawn to `recipient` in lamports.
	pub public_out: u64,
	/// Optional fee paid from shielded balance to `relayer`.
	pub relayer_fee: u64,
	/// Optional change output commitment (`[0; 32]` if no change).
	pub change_commitment: [u8; 32],
	pub change_view_pubkey: [u8; 32],
	pub envelope_len: u8,
	pub envelope: [u8; 96],
	pub shares: [u8; 144],
	/// 192-byte compressed Groth16 proof (`A: 32B, B: 128B, C: 32B`).
	pub proof_a: [u8; 32],
	pub proof_b: [u8; 128],
	pub proof_c: [u8; 32],
}

#[instruction(discriminator = PrivacyPoolInstruction::Transfer)]
pub struct TransferIx {
	pub nullifier_0_bump: u8,
	pub nullifier_1_bump: u8,
	pub nullifier_0: [u8; 32],
	pub nullifier_1: [u8; 32],
	pub root: [u8; 32],
	/// Optional SOL relayer fee deducted from shielded inputs.
	pub relayer_fee: u64,
	pub out0_commitment: [u8; 32],
	pub out0_view_pubkey: [u8; 32],
	pub out0_envelope_len: u8,
	pub out0_envelope: [u8; 96],
	pub out1_commitment: [u8; 32],
	pub out1_view_pubkey: [u8; 32],
	pub out1_envelope_len: u8,
	pub out1_envelope: [u8; 96],
	/// 192-byte compressed Groth16 proof (`A: 32B, B: 128B, C: 32B`).
	pub proof_a: [u8; 32],
	pub proof_b: [u8; 128],
	pub proof_c: [u8; 32],
}

#[instruction(discriminator = PrivacyPoolInstruction::RequestDisclosure)]
pub struct RequestDisclosureIx {
	pub bump: u8,
	pub nonce: u64,
	pub tier: u8,
	pub commitment: [u8; 32],
	/// Claimed consent key for `commitment`, authenticated via `path_elements`.
	pub view_pubkey: [u8; 32],
	pub leaf_index: u64,
	pub path_elements: [u8; 512],
	pub notice_len: u8,
	pub notice: [u8; 96],
	pub legal_basis_hash: [u8; 32],
}

#[instruction(discriminator = PrivacyPoolInstruction::ResolveChallenge)]
pub struct ResolveChallengeIx {
	pub approve: u8,
}

#[instruction(discriminator = PrivacyPoolInstruction::GrantDisclosure)]
pub struct GrantDisclosureIx {
	pub reserved: u8,
}

#[instruction(discriminator = PrivacyPoolInstruction::ChallengeDisclosure)]
pub struct ChallengeDisclosureIx {
	pub reserved: u8,
}

#[instruction(discriminator = PrivacyPoolInstruction::ApproveDisclosure)]
pub struct ApproveDisclosureIx {
	pub reserved: u8,
}

#[instruction(discriminator = PrivacyPoolInstruction::CancelDisclosure)]
pub struct CancelDisclosureIx {
	pub reserved: u8,
}

#[instruction(discriminator = PrivacyPoolInstruction::RegisterAsset)]
pub struct RegisterAssetIx {
	pub bump: u8,
}

#[instruction(discriminator = PrivacyPoolInstruction::DepositToken)]
pub struct DepositTokenIx {
	pub amount: u64,
	pub commitment: [u8; 32],
	pub view_pubkey: [u8; 32],
	pub envelope_len: u8,
	pub envelope: [u8; 96],
	pub shares: [u8; 144],
}

#[instruction(discriminator = PrivacyPoolInstruction::WithdrawToken)]
pub struct WithdrawTokenIx {
	pub nullifier_0_bump: u8,
	pub nullifier_1_bump: u8,
	pub nullifier_0: [u8; 32],
	pub nullifier_1: [u8; 32],
	pub root: [u8; 32],
	pub public_out: u64,
	pub relayer_fee: u64,
	pub change_commitment: [u8; 32],
	pub change_view_pubkey: [u8; 32],
	pub envelope_len: u8,
	pub envelope: [u8; 96],
	pub proof_a: [u8; 32],
	pub proof_b: [u8; 128],
	pub proof_c: [u8; 32],
}

#[instruction(discriminator = PrivacyPoolInstruction::SetPolicy)]
pub struct SetPolicyIx {
	pub policy_flags: u8,
	pub policy_cosigner: Address,
}

#[instruction(discriminator = PrivacyPoolInstruction::VerifyPolicy)]
pub struct VerifyPolicyIx {
	/// Single BN254 scalar representing the Custom Ring policy statement.
	pub policy_hash: [u8; 32],
	/// 192-byte compressed Groth16 policy proof (`A: 32B, B: 128B, C: 32B`).
	pub proof_a: [u8; 32],
	pub proof_b: [u8; 128],
	pub proof_c: [u8; 32],
}

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct InitializeAccounts<'a> {
	pub authority: &'a mut AccountView,
	pub pool_config: &'a mut AccountView,
	pub pool_vault: &'a mut AccountView,
	pub merkle_tree: &'a mut AccountView,
	pub custodian_registry: &'a mut AccountView,
	pub requester_registry: &'a mut AccountView,
	pub disclosure_log: &'a mut AccountView,
	pub system_program: &'a AccountView,
	pub program_data: &'a AccountView,
}

#[derive(Accounts)]
pub struct SetVerificationKeyAccounts<'a> {
	pub authority: &'a mut AccountView,
	pub pool_config: &'a AccountView,
	pub verifying_key_account: &'a mut AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts)]
pub struct SetCustodiansAccounts<'a> {
	pub authority: &'a AccountView,
	pub pool_config: &'a AccountView,
	pub custodian_registry: &'a mut AccountView,
}

#[derive(Accounts)]
pub struct RegisterRequesterAccounts<'a> {
	pub authority: &'a AccountView,
	pub pool_config: &'a AccountView,
	pub requester_registry: &'a mut AccountView,
}

#[derive(Accounts)]
pub struct DepositAccounts<'a> {
	pub depositor: &'a mut AccountView,
	pub pool_config: &'a mut AccountView,
	pub pool_vault: &'a mut AccountView,
	pub merkle_tree: &'a mut AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts)]
pub struct WithdrawAccounts<'a> {
	/// Relayer / transaction fee payer; receives `relayer_fee` directly.
	pub payer: &'a mut AccountView,
	pub pool_config: &'a AccountView,
	pub pool_vault: &'a mut AccountView,
	pub merkle_tree: &'a mut AccountView,
	pub nullifier_record_0: &'a mut AccountView,
	pub nullifier_record_1: &'a mut AccountView,
	pub verifying_key_account: &'a AccountView,
	pub recipient: &'a mut AccountView,
	pub policy_cosigner: &'a AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts)]
pub struct TransferAccounts<'a> {
	/// Relayer / transaction fee payer; receives `relayer_fee` directly.
	pub payer: &'a mut AccountView,
	pub pool_config: &'a AccountView,
	pub pool_vault: &'a mut AccountView,
	pub merkle_tree: &'a mut AccountView,
	pub nullifier_record_0: &'a mut AccountView,
	pub nullifier_record_1: &'a mut AccountView,
	pub verifying_key_account: &'a AccountView,
	pub policy_cosigner: &'a AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts)]
pub struct RequestDisclosureAccounts<'a> {
	pub requester: &'a mut AccountView,
	pub pool_config: &'a AccountView,
	pub requester_registry: &'a AccountView,
	pub merkle_tree: &'a AccountView,
	pub disclosure_request: &'a mut AccountView,
	pub system_program: &'a AccountView,
	pub clock: &'a AccountView,
}

#[derive(Accounts)]
pub struct GrantDisclosureAccounts<'a> {
	pub disclosure_request: &'a mut AccountView,
	pub viewer: &'a AccountView,
}

#[derive(Accounts)]
pub struct ChallengeDisclosureAccounts<'a> {
	pub disclosure_request: &'a mut AccountView,
	pub viewer: &'a AccountView,
	pub clock: &'a AccountView,
}

#[derive(Accounts)]
pub struct ResolveChallengeAccounts<'a> {
	pub authority: &'a AccountView,
	pub pool_config: &'a AccountView,
	pub disclosure_request: &'a mut AccountView,
}

#[derive(Accounts)]
pub struct ApproveDisclosureAccounts<'a> {
	pub custodian: &'a AccountView,
	pub pool_config: &'a AccountView,
	pub custodian_registry: &'a AccountView,
	pub disclosure_request: &'a mut AccountView,
	pub disclosure_log: &'a mut AccountView,
	pub clock: &'a AccountView,
}

#[derive(Accounts)]
pub struct CancelDisclosureAccounts<'a> {
	pub requester: &'a AccountView,
	pub disclosure_request: &'a mut AccountView,
}

#[derive(Accounts)]
pub struct RegisterAssetAccounts<'a> {
	pub payer: &'a mut AccountView,
	pub mint: &'a AccountView,
	pub asset_vault: &'a mut AccountView,
	pub vault_token_account: &'a AccountView,
	pub token_program: &'a AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts)]
pub struct DepositTokenAccounts<'a> {
	pub depositor: &'a mut AccountView,
	pub pool_config: &'a mut AccountView,
	pub merkle_tree: &'a mut AccountView,
	pub mint: &'a AccountView,
	pub asset_vault: &'a AccountView,
	pub depositor_token_account: &'a mut AccountView,
	pub vault_token_account: &'a mut AccountView,
	pub token_program: &'a AccountView,
}

#[derive(Accounts)]
pub struct WithdrawTokenAccounts<'a> {
	pub payer: &'a mut AccountView,
	pub pool_config: &'a AccountView,
	pub merkle_tree: &'a mut AccountView,
	pub nullifier_record_0: &'a mut AccountView,
	pub nullifier_record_1: &'a mut AccountView,
	pub verifying_key_account: &'a AccountView,
	pub mint: &'a AccountView,
	pub asset_vault: &'a AccountView,
	pub vault_token_account: &'a mut AccountView,
	pub recipient_token_account: &'a mut AccountView,
	pub relayer_token_account: &'a mut AccountView,
	pub policy_cosigner: &'a AccountView,
	pub token_program: &'a AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts)]
pub struct SetPolicyAccounts<'a> {
	pub authority: &'a AccountView,
	pub pool_config: &'a mut AccountView,
}

#[derive(Accounts)]
pub struct VerifyPolicyAccounts<'a> {
	pub verifying_key_account: &'a AccountView,
}

// ---------------------------------------------------------------------------
// Instruction: Initialize
// ---------------------------------------------------------------------------

fn assert_bootstrap_authority(
	authority: &AccountView,
	program_data: &AccountView,
) -> ProgramResult {
	const PROGRAM_DATA: Address =
		find_program_address_const(&[ID.as_array().as_slice()], &UPGRADEABLE_LOADER).0;
	program_data
		.assert_address(&PROGRAM_DATA)?
		.assert_owner(&UPGRADEABLE_LOADER)?;
	let data = program_data.try_borrow()?;

	if data.get(..4) != Some(&3_u32.to_le_bytes())
		|| data.get(12) != Some(&1)
		|| data.get(13..45) != Some(authority.address().as_ref())
	{
		return Err(PrivacyPoolError::InvalidAuthority.into());
	}

	Ok(())
}

impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = InitializeIx::try_from_bytes(data)?;

		self.authority.assert_signer()?.assert_writable()?;
		assert_bootstrap_authority(self.authority, self.program_data)?;
		self.system_program.assert_address(&system::ID)?;
		validate_custodian_set(&args.custodians)?;

		CreateProgramAccountWithBump {
			account: self.pool_config,
			payer: self.authority,
			owner: &ID,
			seeds: &PoolConfig::seeds().as_slices(),
			bump: args.config_bump,
		}
		.invoke_with::<PoolConfig>(|config| {
			config.bump = args.config_bump;
			config.authority = *self.authority.address();
			config.custodian_threshold = DEFAULT_CUSTODIAN_THRESHOLD.into();
			config.challenge_window_secs = DEFAULT_CHALLENGE_WINDOW_SECS.into();
			config.policy_flags = 0;
			config.policy_cosigner = Address::default();
			config.total_deposits = 0.into();
			Ok(())
		})?;

		CreateProgramAccountWithBump {
			account: self.pool_vault,
			payer: self.authority,
			owner: &ID,
			seeds: &PoolVault::seeds().as_slices(),
			bump: args.vault_bump,
		}
		.invoke_with::<PoolVault>(|vault| {
			vault.bump = args.vault_bump;
			Ok(())
		})?;

		CreateProgramAccountWithBump {
			account: self.merkle_tree,
			payer: self.authority,
			owner: &ID,
			seeds: &MerkleTree::seeds().as_slices(),
			bump: args.tree_bump,
		}
		.invoke_with::<MerkleTree>(|tree| {
			tree.bump = args.tree_bump;
			tree.next_leaf_index = 0.into();
			tree.roots_len = 0.into();
			Ok(())
		})?;

		{
			let mut tree = self.merkle_tree.as_account_mut::<MerkleTree>(&ID)?;
			prefill_frontier(tree.filled_subtrees.as_mut_slice())?;
			tree.roots.as_mut_slice().fill(0);
			let next_len = push_root(tree.roots.as_mut_slice(), 0, &ZERO_HASHES[TREE_DEPTH])?;
			tree.roots_len.set(next_len as u16);
		}

		CreateProgramAccountWithBump {
			account: self.custodian_registry,
			payer: self.authority,
			owner: &ID,
			seeds: &CustodianRegistry::seeds().as_slices(),
			bump: args.custodians_bump,
		}
		.invoke_with::<CustodianRegistry>(|registry| {
			registry.bump = args.custodians_bump;
			registry.custodians.copy_from_slice(&args.custodians);
			Ok(())
		})?;

		CreateProgramAccountWithBump {
			account: self.requester_registry,
			payer: self.authority,
			owner: &ID,
			seeds: &RequesterRegistry::seeds().as_slices(),
			bump: args.requesters_bump,
		}
		.invoke_with::<RequesterRegistry>(|registry| {
			registry.bump = args.requesters_bump;
			registry.count = 0;
			registry.keys = [0; 512];
			registry.max_tiers = [0; MAX_REQUESTERS];
			Ok(())
		})?;

		CreateProgramAccountWithBump {
			account: self.disclosure_log,
			payer: self.authority,
			owner: &ID,
			seeds: &DisclosureLog::seeds().as_slices(),
			bump: args.log_bump,
		}
		.invoke_with::<DisclosureLog>(|log| {
			log.bump = args.log_bump;
			log.count = 0.into();
			log.entries.as_mut_slice().fill(0);
			Ok(())
		})?;

		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Instruction: SetVerificationKey (supports creation AND authority rotation)
// ---------------------------------------------------------------------------

impl<'a> ProcessAccountInfos<'a> for SetVerificationKeyAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = SetVerificationKeyIx::try_from_bytes(data)?;

		self.authority.assert_signer()?.assert_writable()?;
		if args.slot != VK_SLOT_JOIN_SPLIT && args.slot != VK_SLOT_POLICY {
			return Err(PrivacyPoolError::InvalidVerifyingKeySlot.into());
		}

		{
			let config = self.pool_config.as_account::<PoolConfig>(&ID)?;
			if config.authority != *self.authority.address() {
				drop(config);
				return Err(PrivacyPoolError::InvalidAuthority.into());
			}
		}

		if self.verifying_key_account.owned_by(&ID) && self.verifying_key_account.data_len() > 0 {
			VerifyingKeyAccount::assert_seeds(
				self.verifying_key_account,
				u64::from(args.slot),
				&ID,
			)?;
			let mut key = self
				.verifying_key_account
				.as_account_mut::<VerifyingKeyAccount>(&ID)?;
			key.slot = args.slot;
			key.alpha_g1 = args.alpha_g1;
			key.beta_g2 = args.beta_g2;
			key.gamma_g2 = args.gamma_g2;
			key.delta_g2 = args.delta_g2;
			key.ic0 = args.ic0;
			key.ic1 = args.ic1;
			return Ok(());
		}

		self.system_program.assert_address(&system::ID)?;
		CreateProgramAccountWithBump {
			account: self.verifying_key_account,
			payer: self.authority,
			owner: &ID,
			seeds: &VerifyingKeyAccount::seeds(u64::from(args.slot)).as_slices(),
			bump: args.bump,
		}
		.invoke_with::<VerifyingKeyAccount>(|key| {
			key.bump = args.bump;
			key.slot = args.slot;
			key.alpha_g1 = args.alpha_g1;
			key.beta_g2 = args.beta_g2;
			key.gamma_g2 = args.gamma_g2;
			key.delta_g2 = args.delta_g2;
			key.ic0 = args.ic0;
			key.ic1 = args.ic1;
			Ok(())
		})
	}
}

// ---------------------------------------------------------------------------
// Instruction: SetCustodians / RegisterRequester / SetPolicy / VerifyPolicy
// ---------------------------------------------------------------------------

impl<'a> ProcessAccountInfos<'a> for SetCustodiansAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = SetCustodiansIx::try_from_bytes(data)?;

		self.authority.assert_signer()?;
		validate_custodian_set(&args.custodians)?;

		{
			let config = self.pool_config.as_account::<PoolConfig>(&ID)?;
			if config.authority != *self.authority.address() {
				drop(config);
				return Err(PrivacyPoolError::InvalidAuthority.into());
			}
		}

		let mut registry = self
			.custodian_registry
			.as_account_mut::<CustodianRegistry>(&ID)?;
		registry.custodians.copy_from_slice(&args.custodians);
		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for RegisterRequesterAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = RegisterRequesterIx::try_from_bytes(data)?;

		self.authority.assert_signer()?;
		if args.max_tier > TIER_COMPELLED {
			return Err(PrivacyPoolError::InvalidTier.into());
		}

		{
			let config = self.pool_config.as_account::<PoolConfig>(&ID)?;
			if config.authority != *self.authority.address() {
				drop(config);
				return Err(PrivacyPoolError::InvalidAuthority.into());
			}
		}

		let mut registry = self
			.requester_registry
			.as_account_mut::<RequesterRegistry>(&ID)?;
		let requester_bytes = args.requester.as_ref();
		if let Some(index) = registry
			.keys
			.as_chunks::<32>()
			.0
			.iter()
			.take(usize::from(registry.count))
			.position(|key| key == requester_bytes)
		{
			registry.max_tiers[index] = args.max_tier;
		} else {
			if usize::from(registry.count) >= MAX_REQUESTERS {
				return Err(PrivacyPoolError::RequesterRegistryFull.into());
			}
			let index = usize::from(registry.count);
			registry.keys[index * 32..(index + 1) * 32].copy_from_slice(requester_bytes);
			registry.max_tiers[index] = args.max_tier;
			registry.count += 1;
		}
		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for SetPolicyAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = SetPolicyIx::try_from_bytes(data)?;
		self.authority.assert_signer()?;

		let mut config = self.pool_config.as_account_mut::<PoolConfig>(&ID)?;
		if config.authority != *self.authority.address() {
			drop(config);
			return Err(PrivacyPoolError::InvalidAuthority.into());
		}
		config.policy_flags = args.policy_flags;
		config.policy_cosigner = args.policy_cosigner;
		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for VerifyPolicyAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = VerifyPolicyIx::try_from_bytes(data)?;
		verify_slot_proof(
			self.verifying_key_account,
			VK_SLOT_POLICY,
			&args.proof_a,
			&args.proof_b,
			&args.proof_c,
			&args.policy_hash,
		)
	}
}

// ---------------------------------------------------------------------------
// Instruction: Deposit (variable-amount SOL, zero per-note PDA rent)
// ---------------------------------------------------------------------------

impl<'a> ProcessAccountInfos<'a> for DepositAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = DepositIx::try_from_bytes(data)?;

		self.depositor.assert_signer()?.assert_writable()?;
		self.system_program.assert_address(&system::ID)?;
		PoolVault::assert_seeds(self.pool_vault, &ID)?;
		check_not_paused(self.pool_config)?;

		let amount = args.amount.get();
		if amount == 0 {
			return Err(PrivacyPoolError::ZeroAmount.into());
		}
		if args.commitment == [0_u8; 32] {
			return Err(PrivacyPoolError::ZeroCommitment.into());
		}
		// `args.commitment` carries the private `note_id = Poseidon(secret, nullifier_seed)`;
		// bind it on-chain to the actual escrowed SOL `amount` and native asset id `[0; 32]`.
		let commitment = compute_note_commitment(&args.commitment, &[0_u8; 32], amount)?;

		system::instructions::Transfer {
			from: self.depositor,
			to: self.pool_vault,
			lamports: amount,
		}
		.invoke()?;

		insert_and_emit_note(
			self.merkle_tree,
			&commitment,
			&args.view_pubkey,
			args.envelope_len,
			&args.envelope,
			&args.shares,
		)?;

		let mut config = self.pool_config.as_account_mut::<PoolConfig>(&ID)?;
		let deposits = config.total_deposits.get();
		config.total_deposits.set(
			deposits
				.checked_add(1)
				.ok_or(PrivacyPoolError::ArithmeticOverflow)?,
		);
		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Shared join-split verification
// ---------------------------------------------------------------------------

fn verify_slot_proof(
	verifying_key: &AccountView,
	slot: u8,
	proof_a: &[u8; 32],
	proof_b: &[u8; 128],
	proof_c: &[u8; 32],
	statement_hash: &[u8; 32],
) -> ProgramResult {
	VerifyingKeyAccount::assert_seeds(verifying_key, u64::from(slot), &ID)?;
	let key = verifying_key.as_account::<VerifyingKeyAccount>(&ID)?;
	if key.slot != slot {
		drop(key);
		return Err(PrivacyPoolError::VerifyingKeyMissing.into());
	}
	let material = Groth16Key {
		alpha_g1: key.alpha_g1,
		beta_g2: key.beta_g2,
		gamma_g2: key.gamma_g2,
		delta_g2: key.delta_g2,
		ic0: key.ic0,
		ic1: key.ic1,
	};
	drop(key);
	let ok = verify_groth16_compressed(&material, proof_a, proof_b, proof_c, statement_hash)?;
	if !ok {
		return Err(PrivacyPoolError::ProofVerificationFailed.into());
	}
	Ok(())
}

#[allow(clippy::too_many_arguments)]
fn verify_and_nullify_join_split(
	payer: &mut AccountView,
	merkle_tree: &AccountView,
	nullifier_record_0: &mut AccountView,
	nullifier_record_1: &mut AccountView,
	verifying_key: &AccountView,
	root: &[u8; 32],
	nullifier_0: &[u8; 32],
	nullifier_0_bump: u8,
	nullifier_1: &[u8; 32],
	nullifier_1_bump: u8,
	statement_hash: &[u8; 32],
	proof_a: &[u8; 32],
	proof_b: &[u8; 128],
	proof_c: &[u8; 32],
) -> ProgramResult {
	// Reject duplicate non-zero nullifiers within the same transaction or
	// already-spent nullifiers before paying for Groth16 verification.
	if *nullifier_0 != [0_u8; 32]
		&& (nullifier_0 == nullifier_1
			|| nullifier_record_0.data_len() > 0
			|| nullifier_record_0.owned_by(&ID))
	{
		return Err(PrivacyPoolError::NullifierAlreadySpent.into());
	}
	if *nullifier_1 != [0_u8; 32]
		&& (nullifier_record_1.data_len() > 0 || nullifier_record_1.owned_by(&ID))
	{
		return Err(PrivacyPoolError::NullifierAlreadySpent.into());
	}
	{
		let tree = merkle_tree.as_account::<MerkleTree>(&ID)?;
		let known = root_known(
			tree.roots.as_slice(),
			usize::from(tree.roots_len.get()),
			root,
		);
		drop(tree);
		if !known {
			return Err(PrivacyPoolError::UnknownRoot.into());
		}
	}

	verify_slot_proof(
		verifying_key,
		VK_SLOT_JOIN_SPLIT,
		proof_a,
		proof_b,
		proof_c,
		statement_hash,
	)?;

	record_nullifier(payer, nullifier_record_0, nullifier_0, nullifier_0_bump)?;
	record_nullifier(payer, nullifier_record_1, nullifier_1, nullifier_1_bump)?;
	Ok(())
}

fn debit_sol_vault(
	pool_vault: &mut AccountView,
	destination: &mut AccountView,
	lamports: u64,
) -> ProgramResult {
	if lamports == 0 {
		return Ok(());
	}
	let remaining = pool_vault
		.lamports()
		.checked_sub(lamports)
		.ok_or(PrivacyPoolError::ArithmeticOverflow)?;
	pool_vault.set_lamports(remaining);
	let updated = destination
		.lamports()
		.checked_add(lamports)
		.ok_or(PrivacyPoolError::ArithmeticOverflow)?;
	destination.set_lamports(updated);
	Ok(())
}

// ---------------------------------------------------------------------------
// Instruction: Withdraw (2-in, variable SOL + optional change + relayer)
// ---------------------------------------------------------------------------

impl<'a> ProcessAccountInfos<'a> for WithdrawAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = WithdrawIx::try_from_bytes(data)?;

		self.payer.assert_signer()?.assert_writable()?;
		self.recipient.assert_writable()?;
		self.system_program.assert_address(&system::ID)?;
		PoolVault::assert_seeds(self.pool_vault, &ID)?;
		check_policy(self.pool_config, Some(self.policy_cosigner))?;

		let public_out = args.public_out.get();
		if public_out == 0 {
			return Err(PrivacyPoolError::ZeroAmount.into());
		}
		let relayer_fee = args.relayer_fee.get();
		let zero_env = [0_u8; MAX_ENVELOPE_BYTES];
		let zero_shares = [0_u8; SHARE_BLOB_BYTES];
		let sol_mint = Address::default();
		let ext_hash = ext_data_digest(
			self.recipient.address(),
			self.payer.address(),
			&sol_mint,
			&args.change_view_pubkey,
			args.envelope_len,
			&args.envelope,
			&args.shares,
			&[0_u8; 32],
			0,
			&zero_env,
			&zero_shares,
		)?;
		let amounts = pack_amounts(0, public_out, relayer_fee);
		let statement_hash = join_split_statement_hash(
			&args.root,
			&args.nullifier_0,
			&args.nullifier_1,
			&args.change_commitment,
			&[0_u8; 32],
			&ext_hash,
			&[0_u8; 32],
			&amounts,
		)?;

		verify_and_nullify_join_split(
			self.payer,
			self.merkle_tree,
			self.nullifier_record_0,
			self.nullifier_record_1,
			self.verifying_key_account,
			&args.root,
			&args.nullifier_0,
			args.nullifier_0_bump,
			&args.nullifier_1,
			args.nullifier_1_bump,
			&statement_hash,
			&args.proof_a,
			&args.proof_b,
			&args.proof_c,
		)?;

		insert_and_emit_note(
			self.merkle_tree,
			&args.change_commitment,
			&args.change_view_pubkey,
			args.envelope_len,
			&args.envelope,
			&args.shares,
		)?;

		debit_sol_vault(self.pool_vault, self.recipient, public_out)?;
		debit_sol_vault(self.pool_vault, self.payer, relayer_fee)?;
		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Instruction: Transfer (2-in-2-out variable join-split + relayer)
// ---------------------------------------------------------------------------

impl<'a> ProcessAccountInfos<'a> for TransferAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = TransferIx::try_from_bytes(data)?;

		self.payer.assert_signer()?.assert_writable()?;
		self.system_program.assert_address(&system::ID)?;
		PoolVault::assert_seeds(self.pool_vault, &ID)?;
		check_policy(self.pool_config, Some(self.policy_cosigner))?;

		if args.out0_commitment == [0_u8; 32] && args.out1_commitment == [0_u8; 32] {
			return Err(PrivacyPoolError::ZeroCommitment.into());
		}

		let relayer_fee = args.relayer_fee.get();
		let zero_addr = Address::default();
		let zero_shares = [0_u8; SHARE_BLOB_BYTES];
		let ext_hash = ext_data_digest(
			&zero_addr,
			self.payer.address(),
			&zero_addr,
			&args.out0_view_pubkey,
			args.out0_envelope_len,
			&args.out0_envelope,
			&zero_shares,
			&args.out1_view_pubkey,
			args.out1_envelope_len,
			&args.out1_envelope,
			&zero_shares,
		)?;
		let amounts = pack_amounts(0, 0, relayer_fee);
		let statement_hash = join_split_statement_hash(
			&args.root,
			&args.nullifier_0,
			&args.nullifier_1,
			&args.out0_commitment,
			&args.out1_commitment,
			&ext_hash,
			&[0_u8; 32],
			&amounts,
		)?;

		verify_and_nullify_join_split(
			self.payer,
			self.merkle_tree,
			self.nullifier_record_0,
			self.nullifier_record_1,
			self.verifying_key_account,
			&args.root,
			&args.nullifier_0,
			args.nullifier_0_bump,
			&args.nullifier_1,
			args.nullifier_1_bump,
			&statement_hash,
			&args.proof_a,
			&args.proof_b,
			&args.proof_c,
		)?;

		insert_and_emit_note(
			self.merkle_tree,
			&args.out0_commitment,
			&args.out0_view_pubkey,
			args.out0_envelope_len,
			&args.out0_envelope,
			&zero_shares,
		)?;
		insert_and_emit_note(
			self.merkle_tree,
			&args.out1_commitment,
			&args.out1_view_pubkey,
			args.out1_envelope_len,
			&args.out1_envelope,
			&zero_shares,
		)?;

		debit_sol_vault(self.pool_vault, self.payer, relayer_fee)?;
		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Instructions: RegisterAsset / DepositToken / WithdrawToken (SPL & Token-2022)
// ---------------------------------------------------------------------------

impl<'a> ProcessAccountInfos<'a> for RegisterAssetAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = RegisterAssetIx::try_from_bytes(data)?;

		self.payer.assert_signer()?.assert_writable()?;
		self.system_program.assert_address(&system::ID)?;
		self.token_program.assert_addresses(&SPL_PROGRAM_IDS)?;
		let token_program = *self.token_program.address();

		// Validate mint (rejecting Token-2022 extensions such as transfer fees
		// or hooks that would break shielded pool accounting) and confirm the
		// escrow token account belongs to this mint and is controlled by the
		// `AssetVault` PDA.
		let mint = self
			.mint
			.as_token_mint_for_program(&token_program)?
			.assert_no_extensions()?;
		drop(mint);
		let vault_ta = self
			.vault_token_account
			.as_token_account_for_program(&token_program)?;
		if vault_ta.mint() != self.mint.address() || vault_ta.owner() != self.asset_vault.address()
		{
			return Err(PrivacyPoolError::InvalidAssetVault.into());
		}
		drop(vault_ta);

		CreateProgramAccountWithBump {
			account: self.asset_vault,
			payer: self.payer,
			owner: &ID,
			seeds: &AssetVault::seeds(self.mint.address()).as_slices(),
			bump: args.bump,
		}
		.invoke_with::<AssetVault>(|vault| {
			vault.bump = args.bump;
			vault.mint = *self.mint.address();
			vault.token_program = token_program;
			vault.vault_token_account = *self.vault_token_account.address();
			Ok(())
		})
	}
}

impl<'a> ProcessAccountInfos<'a> for DepositTokenAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = DepositTokenIx::try_from_bytes(data)?;

		self.depositor.assert_signer()?;
		check_not_paused(self.pool_config)?;

		let amount = args.amount.get();
		if amount == 0 {
			return Err(PrivacyPoolError::ZeroAmount.into());
		}
		if args.commitment == [0_u8; 32] {
			return Err(PrivacyPoolError::ZeroCommitment.into());
		}

		AssetVault::assert_seeds(self.asset_vault, self.mint.address(), &ID)?;
		let (token_program, expected_vault_ta) = {
			let vault = self.asset_vault.as_account::<AssetVault>(&ID)?;
			(vault.token_program, vault.vault_token_account)
		};
		self.token_program.assert_address(&token_program)?;
		if self.vault_token_account.address() != &expected_vault_ta {
			return Err(PrivacyPoolError::InvalidAssetVault.into());
		}

		let decimals = self
			.mint
			.as_token_mint_for_program(&token_program)?
			.assert_no_extensions()?
			.decimals();
		let vault_before = self
			.vault_token_account
			.as_token_account_for_program(&token_program)?
			.amount();

		token::instructions::TransferChecked::new(
			self.depositor_token_account,
			self.mint,
			self.vault_token_account,
			self.depositor,
			amount,
			decimals,
		)
		.invoke_with_program(&token_program)?;

		let vault_after = self
			.vault_token_account
			.as_token_account_for_program(&token_program)?
			.amount();
		let received = vault_after
			.checked_sub(vault_before)
			.ok_or(PrivacyPoolError::ArithmeticOverflow)?;
		if received != amount {
			return Err(PrivacyPoolError::InvalidAssetVault.into());
		}

		// Bind `note_id` on-chain to the verified mint asset id and received token amount.
		let mint_asset = asset_id(self.mint.address())?;
		let commitment = compute_note_commitment(&args.commitment, &mint_asset, received)?;

		insert_and_emit_note(
			self.merkle_tree,
			&commitment,
			&args.view_pubkey,
			args.envelope_len,
			&args.envelope,
			&args.shares,
		)?;

		let mut config = self.pool_config.as_account_mut::<PoolConfig>(&ID)?;
		let deposits = config.total_deposits.get();
		config.total_deposits.set(
			deposits
				.checked_add(1)
				.ok_or(PrivacyPoolError::ArithmeticOverflow)?,
		);
		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for WithdrawTokenAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = WithdrawTokenIx::try_from_bytes(data)?;

		self.payer.assert_signer()?.assert_writable()?;
		self.system_program.assert_address(&system::ID)?;
		check_policy(self.pool_config, Some(self.policy_cosigner))?;

		let public_out = args.public_out.get();
		if public_out == 0 {
			return Err(PrivacyPoolError::ZeroAmount.into());
		}
		let relayer_fee = args.relayer_fee.get();

		AssetVault::assert_seeds(self.asset_vault, self.mint.address(), &ID)?;
		let (vault_bump, token_program, expected_vault_ta) = {
			let vault = self.asset_vault.as_account::<AssetVault>(&ID)?;
			(vault.bump, vault.token_program, vault.vault_token_account)
		};
		self.token_program.assert_address(&token_program)?;
		if self.vault_token_account.address() != &expected_vault_ta {
			return Err(PrivacyPoolError::InvalidAssetVault.into());
		}

		let zero_env = [0_u8; MAX_ENVELOPE_BYTES];
		let zero_shares = [0_u8; SHARE_BLOB_BYTES];
		let ext_hash = ext_data_digest(
			self.recipient_token_account.address(),
			self.relayer_token_account.address(),
			self.mint.address(),
			&args.change_view_pubkey,
			args.envelope_len,
			&args.envelope,
			&zero_shares,
			&[0_u8; 32],
			0,
			&zero_env,
			&zero_shares,
		)?;
		let mint_asset = asset_id(self.mint.address())?;
		let amounts = pack_amounts(0, public_out, relayer_fee);
		let statement_hash = join_split_statement_hash(
			&args.root,
			&args.nullifier_0,
			&args.nullifier_1,
			&args.change_commitment,
			&[0_u8; 32],
			&ext_hash,
			&mint_asset,
			&amounts,
		)?;

		verify_and_nullify_join_split(
			self.payer,
			self.merkle_tree,
			self.nullifier_record_0,
			self.nullifier_record_1,
			self.verifying_key_account,
			&args.root,
			&args.nullifier_0,
			args.nullifier_0_bump,
			&args.nullifier_1,
			args.nullifier_1_bump,
			&statement_hash,
			&args.proof_a,
			&args.proof_b,
			&args.proof_c,
		)?;

		insert_and_emit_note(
			self.merkle_tree,
			&args.change_commitment,
			&args.change_view_pubkey,
			args.envelope_len,
			&args.envelope,
			&zero_shares,
		)?;

		let decimals = self
			.mint
			.as_token_mint_for_program(&token_program)?
			.assert_no_extensions()?
			.decimals();
		let bump_bytes = [vault_bump];
		let signer_seeds = [
			Seed::from(SEED_ASSET_VAULT),
			Seed::from(self.mint.address().as_ref()),
			Seed::from(&bump_bytes),
		];
		let signer = Signer::from(&signer_seeds);

		token::instructions::TransferChecked::new(
			self.vault_token_account,
			self.mint,
			self.recipient_token_account,
			self.asset_vault,
			public_out,
			decimals,
		)
		.invoke_signed_with_program(core::slice::from_ref(&signer), &token_program)?;

		if relayer_fee > 0 {
			token::instructions::TransferChecked::new(
				self.vault_token_account,
				self.mint,
				self.relayer_token_account,
				self.asset_vault,
				relayer_fee,
				decimals,
			)
			.invoke_signed_with_program(&[signer], &token_program)?;
		}
		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Instructions: Tiered Disclosure State Machine
// ---------------------------------------------------------------------------

impl<'a> ProcessAccountInfos<'a> for RequestDisclosureAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = RequestDisclosureIx::try_from_bytes(data)?;

		self.requester.assert_signer()?.assert_writable()?;
		if args.tier > TIER_COMPELLED {
			return Err(PrivacyPoolError::InvalidTier.into());
		}
		if args.tier > TIER_CONSENT {
			let entitled = {
				let registry = self
					.requester_registry
					.as_account::<RequesterRegistry>(&ID)?;
				registry
					.keys
					.as_chunks::<32>()
					.0
					.iter()
					.take(usize::from(registry.count))
					.zip(registry.max_tiers.iter())
					.any(|(key, tier)| {
						key == self.requester.address().as_ref() && *tier >= args.tier
					})
			};
			if !entitled {
				return Err(PrivacyPoolError::RequesterNotEntitled.into());
			}
			if args.legal_basis_hash == [0_u8; 32] {
				return Err(PrivacyPoolError::RequesterNotEntitled.into());
			}
		}
		let notice_len = usize::from(args.notice_len);
		if notice_len > MAX_NOTICE_BYTES {
			return Err(PrivacyPoolError::BufferOverflow.into());
		}

		// Authenticate that `(commitment, view_pubkey)` is a genuine leaf in a
		// known tree root — eliminating per-note PDA storage while preserving
		// cryptographic consent-key binding.
		let leaf = note_leaf(&args.commitment, &args.view_pubkey)?;
		let computed_root = compute_merkle_root(&leaf, args.leaf_index.get(), &args.path_elements)?;
		{
			let tree = self.merkle_tree.as_account::<MerkleTree>(&ID)?;
			let known = root_known(
				tree.roots.as_slice(),
				usize::from(tree.roots_len.get()),
				&computed_root,
			);
			drop(tree);
			if !known {
				return Err(PrivacyPoolError::InvalidMerkleProof.into());
			}
		}

		let window = {
			let config = self.pool_config.as_account::<PoolConfig>(&ID)?;
			u64::from(config.challenge_window_secs.get())
		};
		let now = clock_timestamp(self.clock)?;
		let deadline = challenge_deadline(now, window, args.tier)?;

		CreateProgramAccountWithBump {
			account: self.disclosure_request,
			payer: self.requester,
			owner: &ID,
			seeds: &DisclosureRequest::seeds(self.requester.address(), args.nonce.get())
				.as_slices(),
			bump: args.bump,
		}
		.invoke_with::<DisclosureRequest>(|request| {
			request.bump = args.bump;
			request.requester = *self.requester.address();
			request.nonce.set(args.nonce.get());
			request.tier = args.tier;
			request.commitment = args.commitment;
			request.view_pubkey = args.view_pubkey;
			request.notice_len = args.notice_len;
			request.notice.as_mut_slice().fill(0);
			request.notice.as_mut_slice()[..notice_len].copy_from_slice(&args.notice[..notice_len]);
			request.legal_basis_hash = args.legal_basis_hash;
			request.created_at.set(now);
			request.challenge_deadline.set(deadline);
			request.status = STATUS_PENDING;
			request.granted = 0;
			request.approvals = 0;
			Ok(())
		})
	}
}

fn require_request_viewer(
	request: &AccountView,
	viewer: &AccountView,
) -> Result<(u8, u8, u64), ProgramError> {
	viewer.assert_signer()?;
	let request_view = request.as_account::<DisclosureRequest>(&ID)?;
	if viewer.address().as_ref() != request_view.view_pubkey.as_ref() {
		return Err(PrivacyPoolError::NotNoteViewer.into());
	}
	Ok((
		request_view.status,
		request_view.tier,
		request_view.challenge_deadline.get(),
	))
}

impl<'a> ProcessAccountInfos<'a> for GrantDisclosureAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = GrantDisclosureIx::try_from_bytes(data)?;
		let (status, ..) = require_request_viewer(self.disclosure_request, self.viewer)?;
		if status != STATUS_PENDING {
			return Err(PrivacyPoolError::InvalidRequestStatus.into());
		}

		let mut request = self
			.disclosure_request
			.as_account_mut::<DisclosureRequest>(&ID)?;
		request.granted = 1;
		let _ = args.reserved;
		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for ChallengeDisclosureAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = ChallengeDisclosureIx::try_from_bytes(data)?;
		let (status, tier, deadline) =
			require_request_viewer(self.disclosure_request, self.viewer)?;
		if status != STATUS_PENDING || tier != TIER_VERIFIED {
			return Err(PrivacyPoolError::InvalidRequestStatus.into());
		}
		let now = clock_timestamp(self.clock)?;
		if now >= deadline {
			return Err(PrivacyPoolError::InvalidRequestStatus.into());
		}
		let _ = args.reserved;

		let mut request = self
			.disclosure_request
			.as_account_mut::<DisclosureRequest>(&ID)?;
		request.status = STATUS_CHALLENGED;
		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for ResolveChallengeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = ResolveChallengeIx::try_from_bytes(data)?;
		self.authority.assert_signer()?;
		{
			let config = self.pool_config.as_account::<PoolConfig>(&ID)?;
			if config.authority != *self.authority.address() {
				drop(config);
				return Err(PrivacyPoolError::InvalidAuthority.into());
			}
		}

		let mut request = self
			.disclosure_request
			.as_account_mut::<DisclosureRequest>(&ID)?;
		if request.status != STATUS_CHALLENGED {
			return Err(PrivacyPoolError::InvalidRequestStatus.into());
		}
		request.status = if args.approve != 0 {
			STATUS_RESOLVED
		} else {
			STATUS_REJECTED
		};
		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for ApproveDisclosureAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _args = ApproveDisclosureIx::try_from_bytes(data)?;
		self.custodian.assert_signer()?;

		let slot = {
			let registry = self
				.custodian_registry
				.as_account::<CustodianRegistry>(&ID)?;
			registry
				.custodians
				.as_chunks::<32>()
				.0
				.iter()
				.position(|key| key == self.custodian.address().as_ref())
		};
		let slot = slot.ok_or(PrivacyPoolError::NotACustodian)?;
		let bit = 1_u8 << slot;

		let threshold = {
			let config = self.pool_config.as_account::<PoolConfig>(&ID)?;
			u32::from(config.custodian_threshold.get())
		};
		let now = clock_timestamp(self.clock)?;

		let (requester, commitment, nonce, tier);
		{
			let mut request = self
				.disclosure_request
				.as_account_mut::<DisclosureRequest>(&ID)?;
			let status = request.status;
			let granted = request.granted;
			let deadline = request.challenge_deadline;
			let approvals = request.approvals;
			tier = request.tier;

			if status != STATUS_PENDING && status != STATUS_RESOLVED {
				return Err(PrivacyPoolError::InvalidRequestStatus.into());
			}
			match tier {
				TIER_CONSENT => {
					if granted == 0 {
						return Err(PrivacyPoolError::ConsentRequired.into());
					}
				}
				TIER_VERIFIED => {
					if status == STATUS_PENDING && now < deadline.get() {
						return Err(PrivacyPoolError::ChallengeWindowOpen.into());
					}
				}
				TIER_COMPELLED => {}
				_ => return Err(PrivacyPoolError::InvalidTier.into()),
			}

			if approvals & bit != 0 {
				return Err(PrivacyPoolError::AlreadyApproved.into());
			}
			let next_approvals = approvals | bit;
			request.approvals = next_approvals;

			if next_approvals.count_ones() < threshold {
				return Ok(());
			}

			request.status = STATUS_EXECUTED;
			requester = request.requester;
			commitment = request.commitment;
			nonce = request.nonce.get();
		}

		// Record a domain-separated scope hash instead of the raw commitment in
		// the append-only log.
		let scope_hash = disclosure_scope_hash(&commitment, &requester, nonce);
		let mut log = self.disclosure_log.as_account_mut::<DisclosureLog>(&ID)?;
		let count = log.count.get();
		if count >= LOG_CAPACITY as u64 {
			return Err(PrivacyPoolError::LogFull.into());
		}
		let entries = log.entries.as_mut_slice();
		let start = count as usize * LOG_ENTRY_BYTES;
		let entry = entries
			.get_mut(start..start + LOG_ENTRY_BYTES)
			.ok_or(PrivacyPoolError::BufferOverflow)?;
		encode_log_entry(entry, &requester, &scope_hash, tier, nonce, now)?;
		log.count.set(count + 1);
		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for CancelDisclosureAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = CancelDisclosureIx::try_from_bytes(data)?;
		self.requester.assert_signer()?;

		let mut request = self
			.disclosure_request
			.as_account_mut::<DisclosureRequest>(&ID)?;
		if request.requester != *self.requester.address() {
			return Err(PrivacyPoolError::RequesterNotEntitled.into());
		}
		if request.status != STATUS_PENDING {
			return Err(PrivacyPoolError::InvalidRequestStatus.into());
		}
		request.status = STATUS_CANCELLED;
		let _ = args.reserved;
		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Entrypoint
// ---------------------------------------------------------------------------

#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use super::*;

	nostd_entrypoint_alloc!(PrivacyPoolInstruction::process_instruction);
}

// ---------------------------------------------------------------------------
// Host-only proving toolkit (`prover` feature)
// ---------------------------------------------------------------------------

#[cfg(all(feature = "prover", not(target_os = "solana")))]
pub mod prover {
	extern crate alloc;

	use alloc::vec;
	use alloc::vec::Vec;

	use ark_bn254::Bn254;
	use ark_bn254::Fr;
	use ark_bn254::g1::G1Affine;
	use ark_bn254::g2::G2Affine;
	use ark_ff::BigInteger;
	use ark_ff::PrimeField;
	use ark_ff::Zero;
	use ark_groth16::Groth16;
	use ark_groth16::Proof;
	use ark_groth16::ProvingKey;
	use ark_groth16::VerifyingKey;
	use ark_r1cs_std::fields::FieldVar;
	use ark_r1cs_std::fields::fp::FpVar;
	use ark_r1cs_std::prelude::*;
	use ark_relations::r1cs::ConstraintSynthesizer;
	use ark_relations::r1cs::ConstraintSystemRef;
	use ark_relations::r1cs::SynthesisError;
	use ark_serialize::CanonicalSerialize;
	use ark_serialize::Compress;
	use ark_snark::SNARK;
	use ark_std::UniformRand;
	use light_poseidon::PoseidonParameters;
	use light_poseidon::parameters::bn254_x5;
	use rand::SeedableRng;
	use rand::rngs::StdRng;

	use super::MERKLE_PATH_BYTES;
	use super::TREE_DEPTH;
	use super::ZERO_HASHES;

	// -------------------------------------------------------------------
	// Wire encoding: field elements and compressed/uncompressed curve points
	// -------------------------------------------------------------------

	pub fn fr_to_le(value: &Fr) -> [u8; 32] {
		let mut out = [0_u8; 32];
		let bytes = value.into_bigint().to_bytes_le();
		out.copy_from_slice(&bytes);
		out
	}

	pub fn fr_from_le(bytes: &[u8; 32]) -> Option<Fr> {
		let mut limbs = [0_u64; 4];
		for (index, chunk) in bytes.as_chunks::<8>().0.iter().enumerate() {
			let mut limb = [0_u8; 8];
			limb.copy_from_slice(chunk);
			limbs[index] = u64::from_le_bytes(limb);
		}
		Fr::from_bigint(ark_ff::BigInteger256::new(limbs))
	}

	pub fn g1_to_be(affine: &G1Affine) -> [u8; 64] {
		let mut le = [0_u8; 64];
		affine
			.x
			.serialize_with_mode(&mut le[..32], Compress::No)
			.unwrap_or_else(|error| panic!("g1 x: {error:?}"));
		affine
			.y
			.serialize_with_mode(&mut le[32..], Compress::No)
			.unwrap_or_else(|error| panic!("g1 y: {error:?}"));
		le[..32].reverse();
		le[32..].reverse();
		le
	}

	pub fn g2_to_be(affine: &G2Affine) -> [u8; 128] {
		let mut le = [0_u8; 128];
		affine
			.x
			.serialize_with_mode(&mut le[..64], Compress::No)
			.unwrap_or_else(|error| panic!("g2 x: {error:?}"));
		affine
			.y
			.serialize_with_mode(&mut le[64..], Compress::No)
			.unwrap_or_else(|error| panic!("g2 y: {error:?}"));
		le[..64].reverse();
		le[64..].reverse();
		le
	}

	/// Compress a G₁ affine point to Solana's 32-byte big-endian format
	/// (`sol_alt_bn128_compression` `ALT_BN128_G1_DECOMPRESS_BE` input).
	pub fn g1_compress_be(affine: &G1Affine) -> [u8; 32] {
		let mut out = [0_u8; 32];
		affine
			.serialize_compressed(out.as_mut_slice())
			.unwrap_or_else(|error| panic!("g1 compress: {error:?}"));
		out.reverse();
		out
	}

	/// Compress a G₂ affine point to Solana's 64-byte big-endian format
	/// (`sol_alt_bn128_compression` `ALT_BN128_G2_DECOMPRESS_BE` input).
	pub fn g2_compress_be(affine: &G2Affine) -> [u8; 64] {
		let mut out = [0_u8; 64];
		affine
			.serialize_compressed(out.as_mut_slice())
			.unwrap_or_else(|error| panic!("g2 compress: {error:?}"));
		out.reverse();
		out
	}

	// -------------------------------------------------------------------
	// In-circuit Poseidon-2 (circom x5, state width 3) & u64 range check
	// -------------------------------------------------------------------

	fn width3_parameters() -> PoseidonParameters<Fr> {
		bn254_x5::get_poseidon_parameters::<Fr>(3)
			.unwrap_or_else(|error| panic!("poseidon parameters: {error:?}"))
	}

	fn poseidon2_var(
		params: &PoseidonParameters<Fr>,
		left: &FpVar<Fr>,
		right: &FpVar<Fr>,
	) -> Result<FpVar<Fr>, SynthesisError> {
		let zero = FpVar::constant(Fr::zero());
		let mut state = vec![zero.clone(), left.clone(), right.clone()];

		let half = params.full_rounds / 2;
		let all_rounds = params.full_rounds + params.partial_rounds;
		for round in 0..all_rounds {
			for (element, slot) in state.iter_mut().enumerate() {
				let constant = FpVar::constant(params.ark[round * params.width + element]);
				*slot = slot.clone() + constant;
			}
			let sbox = |value: &FpVar<Fr>| -> Result<FpVar<Fr>, SynthesisError> {
				Ok(value.square()?.square()? * value)
			};
			if round < half || round >= half + params.partial_rounds {
				for slot in &mut state {
					*slot = sbox(slot)?;
				}
			} else {
				state[0] = sbox(&state[0])?;
			}
			let mut next = Vec::with_capacity(params.width);
			for matrix_row in &params.mds {
				let mut acc = zero.clone();
				for (coefficient, slot) in matrix_row.iter().zip(state.iter()) {
					if coefficient.is_zero() {
						continue;
					}
					acc += slot.clone() * FpVar::constant(*coefficient);
				}
				next.push(acc);
			}
			state = next;
		}
		Ok(state[0].clone())
	}

	/// Allocate a 64-bit unsigned integer witness and constrain it to `[0, 2^64)`.
	fn alloc_u64_var(
		cs: &ConstraintSystemRef<Fr>,
		value: Option<u64>,
	) -> Result<FpVar<Fr>, SynthesisError> {
		let mut acc = FpVar::constant(Fr::zero());
		let mut coeff = Fr::from(1_u64);
		for bit_idx in 0..64 {
			let bit = Boolean::new_witness(cs.clone(), || {
				value
					.map(|v| ((v >> bit_idx) & 1) == 1)
					.ok_or(SynthesisError::AssignmentMissing)
			})?;
			acc += FpVar::from(bit) * FpVar::constant(coeff);
			coeff += coeff;
		}
		Ok(acc)
	}

	fn select_left(
		index: &Boolean<Fr>,
		on_zero: &FpVar<Fr>,
		on_one: &FpVar<Fr>,
	) -> Result<FpVar<Fr>, SynthesisError> {
		index.select(on_one, on_zero)
	}

	// -------------------------------------------------------------------
	// Host-side Merkle tree mirror (clients maintain from NoteEncryptedEvent)
	// -------------------------------------------------------------------

	/// Lightweight host-side Merkle tree for building depth-16 witnesses.
	#[derive(Clone, Debug)]
	pub struct HostMerkleTree {
		leaves: Vec<[u8; 32]>,
	}

	impl Default for HostMerkleTree {
		fn default() -> Self {
			Self::new()
		}
	}

	impl HostMerkleTree {
		pub fn new() -> Self {
			Self { leaves: Vec::new() }
		}

		pub fn push(&mut self, leaf: [u8; 32]) -> usize {
			let idx = self.leaves.len();
			self.leaves.push(leaf);
			idx
		}

		pub fn root(&self) -> [u8; 32] {
			if self.leaves.is_empty() {
				return ZERO_HASHES[TREE_DEPTH];
			}
			let mut layer = self.leaves.clone();
			for &zero in &ZERO_HASHES[..TREE_DEPTH] {
				let mut next = Vec::with_capacity(layer.len().div_ceil(2));
				let mut i = 0;
				while i < layer.len() {
					let left = layer[i];
					let right = if i + 1 < layer.len() {
						layer[i + 1]
					} else {
						zero
					};
					next.push(
						super::poseidon2(&left, &right)
							.unwrap_or_else(|e| panic!("host tree poseidon: {e:?}")),
					);
					i += 2;
				}
				layer = next;
			}
			layer[0]
		}

		/// Build a depth-16 witness `(path_elements, path_indices)` for
		/// `leaf_index`.
		pub fn witness(&self, leaf_index: usize) -> ([Fr; TREE_DEPTH], [bool; TREE_DEPTH]) {
			let mut path_elements = [Fr::zero(); TREE_DEPTH];
			let mut path_indices = [false; TREE_DEPTH];
			let mut layer = self.leaves.clone();
			let mut pos = leaf_index;

			for level in 0..TREE_DEPTH {
				let sibling_pos = pos ^ 1;
				let sibling_bytes = if sibling_pos < layer.len() {
					layer[sibling_pos]
				} else {
					ZERO_HASHES[level]
				};
				path_elements[level] =
					fr_from_le(&sibling_bytes).unwrap_or_else(|| panic!("canonical sibling"));
				path_indices[level] = pos & 1 == 1;

				let mut next = Vec::with_capacity(layer.len().div_ceil(2));
				let mut i = 0;
				while i < layer.len() {
					let left = layer[i];
					let right = if i + 1 < layer.len() {
						layer[i + 1]
					} else {
						ZERO_HASHES[level]
					};
					next.push(
						super::poseidon2(&left, &right)
							.unwrap_or_else(|e| panic!("host witness poseidon: {e:?}")),
					);
					i += 2;
				}
				layer = next;
				pos >>= 1;
			}
			(path_elements, path_indices)
		}

		/// Serialize `path_elements` into the flat 512-byte wire format for
		/// `RequestDisclosureIx`.
		pub fn witness_bytes(&self, leaf_index: usize) -> [u8; MERKLE_PATH_BYTES] {
			let (elements, _) = self.witness(leaf_index);
			let mut out = [0_u8; MERKLE_PATH_BYTES];
			for (i, el) in elements.iter().enumerate() {
				out[i * 32..(i + 1) * 32].copy_from_slice(&fr_to_le(el));
			}
			out
		}
	}

	// -------------------------------------------------------------------
	// Note helpers
	// -------------------------------------------------------------------

	#[derive(Clone, Copy, Debug)]
	pub struct NoteSecrets {
		pub secret: Fr,
		pub nullifier_seed: Fr,
		pub amount: u64,
		pub asset_id: Fr,
		pub view_pubkey: [u8; 32],
	}

	impl NoteSecrets {
		pub fn dummy(asset_id: Fr) -> Self {
			Self {
				secret: Fr::zero(),
				nullifier_seed: Fr::zero(),
				amount: 0,
				asset_id,
				view_pubkey: [0_u8; 32],
			}
		}

		pub fn note_id_bytes(&self) -> [u8; 32] {
			if self.amount == 0 {
				return [0_u8; 32];
			}
			super::poseidon2(&fr_to_le(&self.secret), &fr_to_le(&self.nullifier_seed))
				.unwrap_or_else(|e| panic!("note_id: {e:?}"))
		}

		pub fn commitment_bytes(&self) -> [u8; 32] {
			if self.amount == 0 {
				return [0_u8; 32];
			}
			let note_id = self.note_id_bytes();
			super::compute_note_commitment(&note_id, &fr_to_le(&self.asset_id), self.amount)
				.unwrap_or_else(|e| panic!("commitment: {e:?}"))
		}

		pub fn leaf_bytes(&self) -> [u8; 32] {
			let commitment = self.commitment_bytes();
			super::note_leaf(&commitment, &self.view_pubkey)
				.unwrap_or_else(|e| panic!("leaf: {e:?}"))
		}

		pub fn nullifier_bytes(&self) -> [u8; 32] {
			if self.amount == 0 {
				return [0_u8; 32];
			}
			super::poseidon2(&fr_to_le(&self.nullifier_seed), &fr_to_le(&self.secret))
				.unwrap_or_else(|e| panic!("nullifier: {e:?}"))
		}
	}

	/// Fresh deterministic note secrets for tests.
	pub fn note_secrets(
		seed: u64,
		amount: u64,
		asset_id: Fr,
		view_pubkey: [u8; 32],
	) -> NoteSecrets {
		let mut rng = StdRng::seed_from_u64(seed);
		NoteSecrets {
			secret: Fr::rand(&mut rng),
			nullifier_seed: Fr::rand(&mut rng),
			amount,
			asset_id,
			view_pubkey,
		}
	}

	// -------------------------------------------------------------------
	// Circuit 1: Unified 2-in-2-out Join-Split Circuit
	// -------------------------------------------------------------------

	#[derive(Clone)]
	pub struct InputWitness {
		pub note: NoteSecrets,
		pub path_elements: [Fr; TREE_DEPTH],
		pub path_indices: [bool; TREE_DEPTH],
	}

	impl InputWitness {
		pub fn dummy(asset_id: Fr) -> Self {
			Self {
				note: NoteSecrets::dummy(asset_id),
				path_elements: [Fr::zero(); TREE_DEPTH],
				path_indices: [false; TREE_DEPTH],
			}
		}
	}

	#[derive(Clone)]
	pub struct JoinSplitWitness {
		pub root: Fr,
		pub asset_id: Fr,
		pub ext_data_hash: Fr,
		pub public_in: u64,
		pub public_out: u64,
		pub relayer_fee: u64,
		pub inputs: [InputWitness; 2],
		pub outputs: [NoteSecrets; 2],
	}

	#[derive(Clone)]
	pub struct JoinSplitCircuit {
		/// Single public input compressing all statement fields.
		pub statement_hash: Fr,
		pub witness: Option<JoinSplitWitness>,
	}

	impl ConstraintSynthesizer<Fr> for JoinSplitCircuit {
		fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
			let params = width3_parameters();
			let statement_pub = FpVar::new_input(cs.clone(), || Ok(self.statement_hash))?;
			let w = self.witness.as_ref();

			let root = FpVar::new_witness(cs.clone(), || {
				w.map(|x| x.root).ok_or(SynthesisError::AssignmentMissing)
			})?;
			let asset_id = FpVar::new_witness(cs.clone(), || {
				w.map(|x| x.asset_id)
					.ok_or(SynthesisError::AssignmentMissing)
			})?;
			let ext_data_hash = FpVar::new_witness(cs.clone(), || {
				w.map(|x| x.ext_data_hash)
					.ok_or(SynthesisError::AssignmentMissing)
			})?;

			let public_in = alloc_u64_var(&cs, w.map(|x| x.public_in))?;
			let public_out = alloc_u64_var(&cs, w.map(|x| x.public_out))?;
			let relayer_fee = alloc_u64_var(&cs, w.map(|x| x.relayer_fee))?;

			let zero = FpVar::constant(Fr::zero());
			let mut sum_in = public_in.clone();
			let mut nullifiers = Vec::with_capacity(2);

			for i in 0..2 {
				let secret = FpVar::new_witness(cs.clone(), || {
					w.map(|x| x.inputs[i].note.secret)
						.ok_or(SynthesisError::AssignmentMissing)
				})?;
				let seed = FpVar::new_witness(cs.clone(), || {
					w.map(|x| x.inputs[i].note.nullifier_seed)
						.ok_or(SynthesisError::AssignmentMissing)
				})?;
				let amount = alloc_u64_var(&cs, w.map(|x| x.inputs[i].note.amount))?;
				let view_hash = FpVar::new_witness(cs.clone(), || {
					w.and_then(|x| {
						let h = super::address_hash(&x.inputs[i].note.view_pubkey).ok()?;
						fr_from_le(&h)
					})
					.ok_or(SynthesisError::AssignmentMissing)
				})?;

				let is_zero = amount.is_eq(&zero)?;
				sum_in += &amount;

				let note_id = poseidon2_var(&params, &secret, &seed)?;
				let asset_amt = poseidon2_var(&params, &asset_id, &amount)?;
				let commitment = poseidon2_var(&params, &note_id, &asset_amt)?;
				let mut current = poseidon2_var(&params, &commitment, &view_hash)?;

				for level in 0..TREE_DEPTH {
					let sibling = FpVar::new_witness(cs.clone(), || {
						w.map(|x| x.inputs[i].path_elements[level])
							.ok_or(SynthesisError::AssignmentMissing)
					})?;
					let index = Boolean::new_witness(cs.clone(), || {
						w.map(|x| x.inputs[i].path_indices[level])
							.ok_or(SynthesisError::AssignmentMissing)
					})?;
					let chosen_left = select_left(&index, &current, &sibling)?;
					let chosen_right = select_left(&index, &sibling, &current)?;
					current = poseidon2_var(&params, &chosen_left, &chosen_right)?;
				}

				// Enforce Merkle membership iff input amount != 0.
				((current - &root) * &amount).enforce_equal(&zero)?;

				// Nullifier is 0 for dummy inputs (`amount == 0`).
				let raw_nullifier = poseidon2_var(&params, &seed, &secret)?;
				let nullifier = is_zero.select(&zero, &raw_nullifier)?;
				nullifiers.push(nullifier);
			}

			let mut sum_out = &public_out + &relayer_fee;
			let mut out_commitments = Vec::with_capacity(2);

			for j in 0..2 {
				let secret = FpVar::new_witness(cs.clone(), || {
					w.map(|x| x.outputs[j].secret)
						.ok_or(SynthesisError::AssignmentMissing)
				})?;
				let seed = FpVar::new_witness(cs.clone(), || {
					w.map(|x| x.outputs[j].nullifier_seed)
						.ok_or(SynthesisError::AssignmentMissing)
				})?;
				let amount = alloc_u64_var(&cs, w.map(|x| x.outputs[j].amount))?;
				let is_zero = amount.is_eq(&zero)?;
				sum_out += &amount;

				let note_id = poseidon2_var(&params, &secret, &seed)?;
				let asset_amt = poseidon2_var(&params, &asset_id, &amount)?;
				let raw_commitment = poseidon2_var(&params, &note_id, &asset_amt)?;
				let commitment = is_zero.select(&zero, &raw_commitment)?;
				out_commitments.push(commitment);
			}

			// Value conservation across 64-bit range-checked amounts.
			sum_in.enforce_equal(&sum_out)?;

			// Pack `(public_in, public_out, relayer_fee)` into one 192-bit scalar.
			let shift64 = Fr::from(1_u128 << 64);
			let shift128 = shift64 * shift64;
			let amounts_packed = public_in
				+ public_out * FpVar::constant(shift64)
				+ relayer_fee * FpVar::constant(shift128);

			let h_in = poseidon2_var(&params, &nullifiers[0], &nullifiers[1])?;
			let h_out = poseidon2_var(&params, &out_commitments[0], &out_commitments[1])?;
			let h_meta = poseidon2_var(&params, &ext_data_hash, &asset_id)?;
			let h_ext = poseidon2_var(&params, &h_meta, &amounts_packed)?;
			let left = poseidon2_var(&params, &root, &h_in)?;
			let right = poseidon2_var(&params, &h_out, &h_ext)?;
			let computed = poseidon2_var(&params, &left, &right)?;
			computed.enforce_equal(&statement_pub)?;

			Ok(())
		}
	}

	// -------------------------------------------------------------------
	// Circuit 2: Custom Ring Policy Circuit (`VK_SLOT_POLICY`)
	// -------------------------------------------------------------------

	/// Demonstrates a composable Custom Ring ZK policy proof: proves knowledge
	/// of a preimage `(identity_secret, policy_salt)` whose Poseidon commitment
	/// equals the public `policy_hash`, without revealing the identity.
	#[derive(Clone)]
	pub struct PolicyCircuit {
		pub policy_hash: Fr,
		pub identity_secret: Option<Fr>,
		pub policy_salt: Option<Fr>,
	}

	impl ConstraintSynthesizer<Fr> for PolicyCircuit {
		fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
			let params = width3_parameters();
			let policy_pub = FpVar::new_input(cs.clone(), || Ok(self.policy_hash))?;
			let id = FpVar::new_witness(cs.clone(), || {
				self.identity_secret
					.ok_or(SynthesisError::AssignmentMissing)
			})?;
			let salt = FpVar::new_witness(cs.clone(), || {
				self.policy_salt.ok_or(SynthesisError::AssignmentMissing)
			})?;
			poseidon2_var(&params, &id, &salt)?.enforce_equal(&policy_pub)?;
			Ok(())
		}
	}

	// -------------------------------------------------------------------
	// Setup, proving, and compressed wire serialization
	// -------------------------------------------------------------------

	pub fn seeded_setup_join_split(seed: u64) -> (ProvingKey<Bn254>, VerifyingKey<Bn254>) {
		let mut rng = StdRng::seed_from_u64(seed);
		let circuit = JoinSplitCircuit {
			statement_hash: Fr::zero(),
			witness: None,
		};
		Groth16::<Bn254>::circuit_specific_setup(circuit, &mut rng)
			.unwrap_or_else(|error| panic!("join-split setup: {error:?}"))
	}

	pub fn seeded_setup_policy(seed: u64) -> (ProvingKey<Bn254>, VerifyingKey<Bn254>) {
		let mut rng = StdRng::seed_from_u64(seed);
		let circuit = PolicyCircuit {
			policy_hash: Fr::zero(),
			identity_secret: None,
			policy_salt: None,
		};
		Groth16::<Bn254>::circuit_specific_setup(circuit, &mut rng)
			.unwrap_or_else(|error| panic!("policy setup: {error:?}"))
	}

	pub fn prove_join_split(
		pk: &ProvingKey<Bn254>,
		circuit: JoinSplitCircuit,
		seed: u64,
	) -> Proof<Bn254> {
		let mut rng = StdRng::seed_from_u64(seed);
		Groth16::<Bn254>::prove(pk, circuit, &mut rng)
			.unwrap_or_else(|error| panic!("prove join-split: {error:?}"))
	}

	pub fn prove_policy(pk: &ProvingKey<Bn254>, circuit: PolicyCircuit, seed: u64) -> Proof<Bn254> {
		let mut rng = StdRng::seed_from_u64(seed);
		Groth16::<Bn254>::prove(pk, circuit, &mut rng)
			.unwrap_or_else(|error| panic!("prove policy: {error:?}"))
	}

	pub struct WireVerifyingKey {
		pub alpha_g1: [u8; 64],
		pub beta_g2: [u8; 128],
		pub gamma_g2: [u8; 128],
		pub delta_g2: [u8; 128],
		pub ic0: [u8; 64],
		pub ic1: [u8; 64],
	}

	pub fn serialize_vk(vk: &VerifyingKey<Bn254>) -> WireVerifyingKey {
		assert_eq!(vk.gamma_abc_g1.len(), 2, "single-input VK expected");
		WireVerifyingKey {
			alpha_g1: g1_to_be(&vk.alpha_g1),
			beta_g2: g2_to_be(&vk.beta_g2),
			gamma_g2: g2_to_be(&vk.gamma_g2),
			delta_g2: g2_to_be(&vk.delta_g2),
			ic0: g1_to_be(&vk.gamma_abc_g1[0]),
			ic1: g1_to_be(&vk.gamma_abc_g1[1]),
		}
	}

	/// 192-byte compressed Groth16 wire proof (`A: 32B` pre-negated compressed G₁,
	/// `B: 128B` uncompressed G₂, `C: 32B` compressed G₁), matching Helius Privacy
	/// Rings' wire format.
	pub struct WireProof {
		pub a: [u8; 32],
		pub b: [u8; 128],
		pub c: [u8; 32],
	}

	pub fn serialize_proof_compressed(proof: &Proof<Bn254>) -> WireProof {
		let negated_a: G1Affine = -proof.a;
		WireProof {
			a: g1_compress_be(&negated_a),
			b: g2_to_be(&proof.b),
			c: g1_compress_be(&proof.c),
		}
	}

	#[cfg(test)]
	mod tests {
		use super::*;
		use crate::Address;

		#[test]
		fn compressed_join_split_proof_verifies_and_rejects_tampering() {
			let root = HostMerkleTree::new();
			let asset = Fr::zero();
			let view_pk = [0x5A; 32];
			let in0 = note_secrets(0x1001, 1_000_000_000, asset, view_pk);
			let out0 = note_secrets(0x2001, 600_000_000, asset, [0x6B; 32]);
			let out1 = note_secrets(0x2002, 390_000_000, asset, view_pk);

			let mut tree = root;
			tree.push(in0.leaf_bytes());
			let tree_root = tree.root();
			let (path_elements, path_indices) = tree.witness(0);

			let ext_hash = super::super::ext_data_digest(
				&Address::default(),
				&Address::new_from_array([0xAA; 32]),
				&Address::default(),
				&out0.view_pubkey,
				0,
				&[0_u8; super::super::MAX_ENVELOPE_BYTES],
				&[0_u8; super::super::SHARE_BLOB_BYTES],
				&out1.view_pubkey,
				0,
				&[0_u8; super::super::MAX_ENVELOPE_BYTES],
				&[0_u8; super::super::SHARE_BLOB_BYTES],
			)
			.unwrap();
			let amounts = super::super::pack_amounts(0, 0, 10_000_000);
			let statement = super::super::join_split_statement_hash(
				&tree_root,
				&in0.nullifier_bytes(),
				&[0_u8; 32],
				&out0.commitment_bytes(),
				&out1.commitment_bytes(),
				&ext_hash,
				&[0_u8; 32],
				&amounts,
			)
			.unwrap();

			let (pk, vk) = seeded_setup_join_split(0x7777);
			let circuit = JoinSplitCircuit {
				statement_hash: fr_from_le(&statement).unwrap(),
				witness: Some(JoinSplitWitness {
					root: fr_from_le(&tree_root).unwrap(),
					asset_id: asset,
					ext_data_hash: fr_from_le(&ext_hash).unwrap(),
					public_in: 0,
					public_out: 0,
					relayer_fee: 10_000_000,
					inputs: [
						InputWitness {
							note: in0,
							path_elements,
							path_indices,
						},
						InputWitness::dummy(asset),
					],
					outputs: [out0, out1],
				}),
			};

			let proof = prove_join_split(&pk, circuit, 0x8888);
			let wire_vk = serialize_vk(&vk);
			let wire_proof = serialize_proof_compressed(&proof);
			let key = super::super::Groth16Key {
				alpha_g1: wire_vk.alpha_g1,
				beta_g2: wire_vk.beta_g2,
				gamma_g2: wire_vk.gamma_g2,
				delta_g2: wire_vk.delta_g2,
				ic0: wire_vk.ic0,
				ic1: wire_vk.ic1,
			};
			assert!(
				super::super::verify_groth16_compressed(
					&key,
					&wire_proof.a,
					&wire_proof.b,
					&wire_proof.c,
					&statement,
				)
				.unwrap()
			);

			let mut bad = statement;
			bad[0] ^= 1;
			assert!(
				!super::super::verify_groth16_compressed(
					&key,
					&wire_proof.a,
					&wire_proof.b,
					&wire_proof.c,
					&bad,
				)
				.unwrap()
			);
		}
	}
}

#[cfg(test)]
mod tests {
	extern crate alloc;

	use super::*;

	#[test]
	fn zero_hashes_table_matches_runtime_poseidon2() {
		let computed = zero_hashes().unwrap();
		assert_eq!(ZERO_HASHES, computed);
	}

	#[test]
	fn frontier_tree_matches_host_merkle_tree() {
		let mut frontier = alloc::vec![0_u8; FRONTIER_BYTES];
		prefill_frontier(&mut frontier).unwrap();

		let mut leaves = alloc::vec::Vec::new();
		for i in 0_u8..5 {
			let leaf = [i + 1; 32];
			leaves.push(leaf);
			let root = insert_leaf(&mut frontier, u64::from(i), &leaf).unwrap();
			// Verify computed root against direct fold.
			let mut layer = leaves.clone();
			for &zero in &ZERO_HASHES[..TREE_DEPTH] {
				let mut next = alloc::vec::Vec::new();
				let mut idx = 0;
				while idx < layer.len() {
					let left = layer[idx];
					let right = if idx + 1 < layer.len() {
						layer[idx + 1]
					} else {
						zero
					};
					next.push(poseidon2(&left, &right).unwrap());
					idx += 2;
				}
				layer = next;
			}
			assert_eq!(root, layer[0]);
		}
	}

	#[test]
	fn root_ring_evicts_oldest_only_past_capacity() {
		let mut roots = alloc::vec![0_u8; ROOT_RING_BYTES];
		let mut len = 0;
		for round in 0..ROOT_RING_CAPACITY as u8 {
			len = push_root(&mut roots, len, &[round; 32]).unwrap();
		}
		assert_eq!(len, ROOT_RING_CAPACITY);
		assert!(root_known(&roots, len, &[0_u8; 32]));
		len = push_root(&mut roots, len, &[99_u8; 32]).unwrap();
		assert!(!root_known(&roots, len, &[0_u8; 32]));
		assert!(root_known(&roots, len, &[99_u8; 32]));
	}

	#[test]
	fn disclosure_scope_hash_is_salted_and_deterministic() {
		let commitment = [0xAA; 32];
		let requester = Address::new_from_array([0x11; 32]);
		let s0 = disclosure_scope_hash(&commitment, &requester, 0);
		let s1 = disclosure_scope_hash(&commitment, &requester, 1);
		assert_ne!(s0, commitment);
		assert_ne!(s0, s1);
		assert_eq!(s0, disclosure_scope_hash(&commitment, &requester, 0));
	}
}
