//! A tiered-disclosure privacy pool built with pina.
//!
//! This example is a shielded-pool payments program whose distinguishing
//! feature is its disclosure protocol: transfers are private by default, and
//! access to a note's history is a *governed, logged* act rather than a
//! standing backdoor. It exists to show the scale of program pina can carry —
//! Poseidon commitments through the runtime syscall, on-chain Groth16
//! verification through the BN254 pairing syscall, a full nullifier-set
//! discipline, and a multi-party disclosure state machine — inside one
//! `no_std` crate with no `unsafe` code and heap use confined to the
//! verifier's scratch buffers.
//!
//! # The pool
//!
//! [`PrivacyPoolInstruction::Deposit`] escrows a fixed denomination of SOL
//! and inserts a client-computed Poseidon commitment into an on-chain
//! incremental Merkle tree. [`PrivacyPoolInstruction::Withdraw`] and
//! [`PrivacyPoolInstruction::Transfer`] spend a note by verifying a Groth16
//! proof (public inputs: the tree root, the derived nullifier, and for
//! transfers the successor commitment) and recording the nullifier. The
//! pairing check runs entirely through the `sol_alt_bn128_group_op` syscall,
//! so a spend fits in one transaction at a fraction of the compute budget
//! instead of demanding the multi-transaction slicing a hand-written
//! verifier used to need.
//!
//! # The disclosure tiers
//!
//! Every note carries an *audit envelope*: the note plaintext encrypted by
//! the depositor's client, plus key shares escrowed to a committee of
//! custodians. No single custodian can open an envelope; a threshold can,
//! and the shares travel off-chain so no key material ever becomes public.
//! Disclosure requests flow through three tiers:
//!
//! - **Tier 0 — consent.** Anyone may request. The note's *view key* holder
//!   (a fresh keypair per note) must grant before custodians may execute.
//! - **Tier 1 — verified.** Registered requesters. The subject is notified
//!   through an encrypted notice blob and holds a challenge window; once the
//!   window lapses without a challenge, the committee may execute.
//! - **Tier 2 — compelled.** Registered requesters with standing legal
//!   process. No window; the committee may execute immediately.
//!
//! The invariant the program enforces — the reason this is a protocol and
//! not a policy document — is that reaching the custodian threshold *is* the
//! log entry: [`PrivacyPoolInstruction::ApproveDisclosure`] flips a request
//! to executed and appends to the append-only [`DisclosureLog`] in the same
//! instruction. Disclosure without a permanent public record is impossible
//! by construction.
//!
//! # What this example deliberately is not
//!
//! This is a teaching scaffold, not a production privacy system. The
//! readme enumerates the honest limits: custodians can collude off-chain,
//! fee-payer linkage is unaddressed (no relayer), deposits and withdrawals
//! are correlation points, the nullifier scan is linear, the share
//! encryption in tests is demo-grade, and the disclosure log names its
//! target commitment rather than a salted scope root.

#![allow(missing_docs)]
#![allow(clippy::inline_always)]
// `AccountView` is small and `Copy`, but every pina accessor and example
// passes it by reference; match the shared signature style.
#![expect(clippy::trivially_copy_pass_by_ref)]
#![no_std]

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
const SEED_NULLIFIER_SET: &[u8] = b"privacy-pool-nullifiers";
const SEED_CUSTODIAN_REGISTRY: &[u8] = b"privacy-pool-custodians";
const SEED_REQUESTER_REGISTRY: &[u8] = b"privacy-pool-requesters";
const SEED_NOTE: &[u8] = b"privacy-pool-note";
const SEED_REQUEST: &[u8] = b"privacy-pool-request";
const SEED_LOG: &[u8] = b"privacy-pool-log";
const SEED_VKEY: &[u8] = b"privacy-pool-vkey";

/// Merkle tree depth. 128 leaves keeps the full node store under the
/// 10,240-byte ceiling the runtime places on accounts created inside inner
/// instructions; production trees shard levels across ≤10 KB child accounts
/// (one per subtree), which changes storage layout but not this program's
/// per-level hashing logic.
pub const TREE_DEPTH: usize = 7;

/// Total nodes in a depth-7 binary tree: `2^(depth+1) − 1`.
pub const TREE_NODES: usize = (1 << (TREE_DEPTH + 1)) - 1;

/// Leaves the tree can hold before it is full: `2^depth`.
pub const TREE_CAPACITY: usize = 1 << TREE_DEPTH;

/// Flat byte length of the node store.
pub const TREE_NODES_BYTES: usize = TREE_NODES * 32;

/// Recent roots retained for proof freshness, so a spend whose witness was
/// built against a slightly old root does not fail just because another
/// deposit landed first.
pub const ROOT_RING_CAPACITY: usize = 8;

/// Flat byte length of the root ring.
pub const ROOT_RING_BYTES: usize = ROOT_RING_CAPACITY * 32;

/// Spends the nullifier set can record before it is full. The scan is
/// linear; production sets hash-index their entries.
pub const NULLIFIER_CAPACITY: usize = 128;

/// Flat byte length of the nullifier store.
pub const NULLIFIER_BYTES: usize = NULLIFIER_CAPACITY * 32;

/// SOL escrowed per deposit. A fixed denomination keeps every note fungible
/// and keeps amounts out of the instruction data; multi-denomination pools
/// run one tree per denomination.
pub const DEPOSIT_LAMPORTS: u64 = 1_000_000_000;

/// Custodians on the disclosure committee.
pub const CUSTODIAN_COUNT: usize = 3;

/// Default custodian approvals required to execute a disclosure.
pub const DEFAULT_CUSTODIAN_THRESHOLD: u16 = 2;

/// Default tier-1 challenge window in seconds.
pub const DEFAULT_CHALLENGE_WINDOW_SECS: u32 = 600;

/// Registered disclosure requesters the registry can hold.
pub const MAX_REQUESTERS: usize = 16;

/// Encrypted note plaintext the program stores per note, client-opaque.
pub const MAX_ENVELOPE_BYTES: usize = 128;

/// Per-custodian encrypted key shares, client-opaque: three 48-byte slots.
pub const SHARE_BLOB_BYTES: usize = 48 * CUSTODIAN_COUNT;

/// Encrypted subject-notice blob stored on a disclosure request.
pub const MAX_NOTICE_BYTES: usize = 96;

/// Disclosure log entries the append-only log can hold.
pub const LOG_CAPACITY: usize = 32;

/// One serialized log entry: requester, target commitment, tier, nonce,
/// timestamp, padding.
pub const LOG_ENTRY_BYTES: usize = 96;

/// Flat byte length of the log store.
pub const LOG_BYTES: usize = LOG_CAPACITY * LOG_ENTRY_BYTES;

/// Verifying-key slot for withdraw proofs (public inputs: root, nullifier).
pub const VK_SLOT_WITHDRAW: u8 = 0;

/// Verifying-key slot for transfer proofs (root, nullifier, commitment).
pub const VK_SLOT_TRANSFER: u8 = 1;

/// Largest IC array either circuit uses (transfer: four entries).
pub const MAX_PUBLIC_INPUTS: usize = 4;

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

/// Lamports the inline `Migrate` path may spend on reallocs in one
/// transaction. Every account here is fixed-layout, so migrations resize
/// only when a future layout actually grows an account; the bound covers
/// the 16 KB tree with headroom.
const MAX_INLINE_MIGRATION_LAMPORTS: u64 = 64 * 1024 * 1_000_000;

/// Schema array lengths must be integer literals for the ABI parser; these
/// assertions bind the literals above to the named constants the logic uses.
const _: () = assert!(TREE_NODES_BYTES == 8160);
const _: () = assert!(ROOT_RING_BYTES == 256);
const _: () = assert!(NULLIFIER_BYTES == 4096);
const _: () = assert!(MAX_ENVELOPE_BYTES == 128);
const _: () = assert!(SHARE_BLOB_BYTES == 144);
const _: () = assert!(MAX_NOTICE_BYTES == 96);
const _: () = assert!(LOG_BYTES == 3072);

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
	/// The note account already exists for this commitment.
	NoteAlreadyExists = 2,
	/// The Merkle tree is full.
	TreeFull = 3,
	/// The submitted root is not in the recent-root ring.
	UnknownRoot = 4,
	/// The nullifier has already been spent.
	NullifierAlreadySpent = 5,
	/// The nullifier set is full.
	NullifierSetFull = 6,
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
	/// The target note does not exist.
	NoteNotFound = 16,
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
}

// ---------------------------------------------------------------------------
// Discriminators
// ---------------------------------------------------------------------------

/// The `migrations(...)` list is the reserved `Migrate` instruction's slot
/// order: `[payer, systemProgram, poolConfig, merkleTree, nullifierSet,
/// custodianRegistry, requesterRegistry, disclosureLog, noteCommitment,
/// disclosureRequest, verifyingKeyAccount]`. Generated clients derive the
/// same order from the IDL.
#[discriminator(
	entrypoint,
	migrations(
		PoolConfig,
		MerkleTree,
		NullifierSet,
		CustodianRegistry,
		RequesterRegistry,
		DisclosureLog,
		NoteCommitment,
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
}

#[discriminator]
pub enum PrivacyPoolAccountType {
	PoolConfig = 1,
	PoolVault = 2,
	MerkleTree = 3,
	NullifierSet = 4,
	CustodianRegistry = 5,
	RequesterRegistry = 6,
	DisclosureLog = 7,
	NoteCommitment = 8,
	DisclosureRequest = 9,
	VerifyingKeyAccount = 10,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Global pool configuration and governance root.
#[account(discriminator = PrivacyPoolAccountType)]
#[pda(seeds = [SEED_POOL_CONFIG], bump = bump)]
pub struct PoolConfig {
	pub bump: u8,
	/// The key that may rotate custodians, requesters, and verifying keys.
	pub authority: Address,
	/// Custodian approvals required to execute a disclosure.
	pub custodian_threshold: u16,
	/// Tier-1 challenge window length in seconds.
	pub challenge_window_secs: u32,
	/// SOL escrowed per deposit.
	pub deposit_lamports: u64,
	/// Lifetime deposit count, for analysis only.
	pub total_deposits: u64,
}

/// The pool's SOL custody. Data-less by design: every lamport in it is
/// backed by an unspent note commitment in the tree.
#[account(discriminator = PrivacyPoolAccountType)]
#[pda(seeds = [SEED_VAULT], bump = bump)]
pub struct PoolVault {
	pub bump: u8,
}

/// Full-node incremental Merkle tree over Poseidon-2 hashes.
///
/// Level `ℓ` (0 = leaves) occupies `nodes` at offset
/// `(1 << (depth+1)) − (1 << (depth+1−ℓ))`, so the root sits in the final
/// slot. Empty subtrees hold the precomputed zero-hash chain, letting
/// witnesses cover unwritten leaves.
#[account(discriminator = PrivacyPoolAccountType)]
#[pda(seeds = [SEED_MERKLE_TREE], bump = bump)]
pub struct MerkleTree {
	pub bump: u8,
	/// Number of leaves written so far.
	pub next_leaf_index: u64,
	/// Flat 32-byte node store; unwritten slots hold the zero-hash chain.
	pub nodes: [u8; 8160],
	/// Roots currently in the ring; evictions hold it at capacity.
	pub roots_len: u16,
	/// Ring of recent roots, oldest first, zero-padded until full.
	pub roots: [u8; 256],
}

/// Append-only set of spent nullifiers.
#[account(discriminator = PrivacyPoolAccountType)]
#[pda(seeds = [SEED_NULLIFIER_SET], bump = bump)]
pub struct NullifierSet {
	pub bump: u8,
	/// Number of recorded nullifiers.
	pub count: u64,
	/// Flat 32-byte nullifier store.
	pub nullifiers: [u8; 4096],
}

/// The disclosure committee. A threshold of these keys must approve before
/// a disclosure may execute, and each approval is a signed transaction.
#[account(discriminator = PrivacyPoolAccountType)]
#[pda(seeds = [SEED_CUSTODIAN_REGISTRY], bump = bump)]
pub struct CustodianRegistry {
	pub bump: u8,
	/// Committee members as concatenated 32-byte addresses; raw bytes keep
	/// the generated CLI clients renderable. A zero address marks the slot
	/// unset.
	pub custodians: [u8; 96],
}

/// Entities allowed to file tier-1 and tier-2 disclosure requests.
#[account(discriminator = PrivacyPoolAccountType)]
#[pda(seeds = [SEED_REQUESTER_REGISTRY], bump = bump)]
pub struct RequesterRegistry {
	pub bump: u8,
	/// Active entries in the parallel arrays below.
	pub count: u8,
	/// Requester keys as concatenated 32-byte addresses, parallel to
	/// `max_tiers`.
	pub keys: [u8; 512],
	/// Highest tier each requester may file at.
	pub max_tiers: [u8; 16],
}

/// Per-note audit record. The envelope and shares are encrypted by the
/// depositor's client and opaque to the program; the view key is the
/// per-note consent key that may grant tier-0 requests and file challenges.
///
/// Spent notes are *not* marked here: linking a nullifier to its commitment
/// would break the privacy the pool exists to provide. The nullifier set is
/// the only spent record.
#[account(discriminator = PrivacyPoolAccountType)]
#[pda(seeds = [SEED_NOTE, commitment: Address], bump = bump)]
pub struct NoteCommitment {
	pub bump: u8,
	/// The commitment; also the PDA seed.
	pub commitment: [u8; 32],
	/// Per-note consent/notification public key.
	pub view_pubkey: [u8; 32],
	/// Active bytes of the encrypted note plaintext, client-opaque.
	pub envelope_len: u8,
	pub envelope: [u8; 128],
	/// Concatenated per-custodian encrypted key shares, client-opaque.
	pub shares: [u8; 144],
}

/// One disclosure request and its state machine.
#[account(discriminator = PrivacyPoolAccountType)]
#[pda(seeds = [SEED_REQUEST, requester: Address, nonce: u64], bump = bump)]
pub struct DisclosureRequest {
	pub bump: u8,
	/// Filing entity.
	pub requester: Address,
	/// Client-chosen disambiguator for repeat filings.
	pub nonce: u64,
	/// Tier this request was filed at; see the `TIER_*` constants.
	pub tier: u8,
	/// Target note commitment.
	pub commitment: [u8; 32],
	/// Active bytes of the encrypted notice to the subject, client-opaque.
	pub notice_len: u8,
	pub notice: [u8; 96],
	/// Hash of the legal basis document; required nonzero above tier 0.
	pub legal_basis_hash: [u8; 32],
	/// Filing unix timestamp.
	pub created_at: u64,
	/// Challenge deadline unix timestamp (tier 1).
	pub challenge_deadline: u64,
	/// Current status; see the `STATUS_*` constants.
	pub status: u8,
	/// Set when the note's view key has granted consent.
	pub granted: u8,
	/// Custodian approval bitmask.
	pub approvals: u8,
}

/// Append-only public record of executed disclosures. The entry is written
/// by the same instruction that marks the request executed, so execution
/// without a log record cannot happen.
#[account(discriminator = PrivacyPoolAccountType)]
#[pda(seeds = [SEED_LOG], bump = bump)]
pub struct DisclosureLog {
	pub bump: u8,
	/// Entries written so far.
	pub count: u64,
	/// Flat `LOG_ENTRY_BYTES` entries: requester, commitment, tier, nonce,
	/// timestamp, padding.
	pub entries: [u8; 3072],
}

/// One installed Groth16 verifying key. Slot 0 verifies withdraw proofs
/// (two public inputs), slot 1 transfer proofs (three).
#[account(discriminator = PrivacyPoolAccountType)]
#[pda(seeds = [SEED_VKEY, slot: u64], bump = bump)]
pub struct VerifyingKeyAccount {
	pub bump: u8,
	/// Circuit slot; see the `VK_SLOT_*` constants.
	pub slot: u8,
	/// αG₁, little-endian affine coordinates.
	pub alpha_g1: [u8; 64],
	/// βG₂, little-endian affine coordinates.
	pub beta_g2: [u8; 128],
	/// γG₂, little-endian affine coordinates.
	pub gamma_g2: [u8; 128],
	/// δG₂, little-endian affine coordinates.
	pub delta_g2: [u8; 128],
	/// IC entries in use, including the constant term.
	pub ic_len: u8,
	/// IC G₁ points, little-endian; entries beyond `ic_len` are zero.
	pub ic0: [u8; 64],
	pub ic1: [u8; 64],
	pub ic2: [u8; 64],
	pub ic3: [u8; 64],
}

// ---------------------------------------------------------------------------
// Poseidon and Groth16 verification
// ---------------------------------------------------------------------------

mod crypto;
mod syscalls;

pub use crypto::poseidon2;

/// View a 32-byte commitment as an address; the note PDA seeds off it.
/// A commitment is exactly 32 bytes, so the conversion always succeeds.
fn commitment_address(commitment: &[u8; 32]) -> Address {
	Address::new_from_array(*commitment)
}
pub use crypto::Groth16Key;
pub use crypto::verify_groth16;
pub use crypto::zero_hashes;

/// Node offset for tree level `level` (0 = leaves).
const fn level_offset(level: usize) -> usize {
	(1 << (TREE_DEPTH + 1)) - (1 << (TREE_DEPTH + 1 - level))
}

// ---------------------------------------------------------------------------
// Merkle tree, nullifier, and log helpers
// ---------------------------------------------------------------------------

/// Read the 32-byte node at `level`/`position` out of a flat node store.
pub fn read_node(nodes: &[u8], level: usize, position: usize) -> Result<[u8; 32], ProgramError> {
	let start = (level_offset(level) + position) * 32;
	let end = start + 32;
	let slice = nodes
		.get(start..end)
		.ok_or(PrivacyPoolError::BufferOverflow)?;
	let mut node = [0_u8; 32];
	node.copy_from_slice(slice);
	Ok(node)
}

/// Write a 32-byte node at `level`/`position` into a flat node store.
pub fn write_node(
	nodes: &mut [u8],
	level: usize,
	position: usize,
	value: &[u8; 32],
) -> Result<(), ProgramError> {
	let start = (level_offset(level) + position) * 32;
	let end = start + 32;
	let slice = nodes
		.get_mut(start..end)
		.ok_or(PrivacyPoolError::BufferOverflow)?;
	slice.copy_from_slice(value);
	Ok(())
}

/// Append a leaf and recompute its path to the root. Returns the new root.
pub fn insert_leaf(
	nodes: &mut [u8],
	next_leaf_index: u64,
	leaf: &[u8; 32],
) -> Result<[u8; 32], ProgramError> {
	let index = usize::try_from(next_leaf_index).map_err(|_| PrivacyPoolError::TreeFull)?;
	if index >= TREE_CAPACITY {
		return Err(PrivacyPoolError::TreeFull.into());
	}

	write_node(nodes, 0, index, leaf)?;
	let mut current = *leaf;
	let mut position = index;
	for level in 1..=TREE_DEPTH {
		let sibling = read_node(nodes, level - 1, position ^ 1)?;
		let (left, right) = if position & 1 == 0 {
			(current, sibling)
		} else {
			(sibling, current)
		};
		current = poseidon2(&left, &right)?;
		position >>= 1;
		write_node(nodes, level, position, &current)?;
	}
	Ok(current)
}

/// Prefill a node store with the zero-hash chain so witnesses against
/// unwritten subtrees are well formed.
pub fn prefill_zero_nodes(nodes: &mut [u8]) -> Result<(), ProgramError> {
	let zeros = zero_hashes()?;
	for (level, zero) in zeros.iter().enumerate() {
		let width = TREE_CAPACITY >> level;
		for position in 0..width {
			write_node(nodes, level, position, zero)?;
		}
	}
	Ok(())
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

/// Whether the nullifier is already recorded.
pub fn nullifier_spent(nullifiers: &[u8], count: u64, nullifier: &[u8; 32]) -> bool {
	for index in 0..(count as usize) {
		let start = index * 32;
		if nullifiers
			.get(start..start + 32)
			.is_some_and(|slot| slot == nullifier.as_slice())
		{
			return true;
		}
	}
	false
}

/// Append a nullifier, refusing duplicates.
pub fn push_nullifier(
	nullifiers: &mut [u8],
	count: u64,
	nullifier: &[u8; 32],
) -> Result<u64, ProgramError> {
	if nullifier_spent(nullifiers, count, nullifier) {
		return Err(PrivacyPoolError::NullifierAlreadySpent.into());
	}
	let index = count as usize;
	if index >= NULLIFIER_CAPACITY {
		return Err(PrivacyPoolError::NullifierSetFull.into());
	}
	nullifiers
		.get_mut(index * 32..(index + 1) * 32)
		.ok_or(PrivacyPoolError::BufferOverflow)?
		.copy_from_slice(nullifier);
	Ok(count
		.checked_add(1)
		.ok_or(PrivacyPoolError::ArithmeticOverflow)?)
}

/// Encode one disclosure-log entry into `out`.
pub fn encode_log_entry(
	out: &mut [u8],
	requester: &Address,
	commitment: &[u8; 32],
	tier: u8,
	nonce: u64,
	timestamp: u64,
) -> Result<(), ProgramError> {
	let entry = out
		.get_mut(..LOG_ENTRY_BYTES)
		.ok_or(PrivacyPoolError::BufferOverflow)?;
	entry.fill(0);
	entry[..32].copy_from_slice(requester.as_ref());
	entry[32..64].copy_from_slice(commitment);
	entry[64] = tier;
	entry[72..80].copy_from_slice(&nonce.to_le_bytes());
	entry[80..88].copy_from_slice(&timestamp.to_le_bytes());
	Ok(())
}

/// The unix timestamp from a clock sysvar account view.
fn clock_timestamp(clock: &AccountView) -> Result<u64, ProgramError> {
	Ok(
		u64::try_from(sysvars::clock::Clock::from_account_view(clock)?.unix_timestamp)
			.map_err(|_| PrivacyPoolError::ArithmeticOverflow)?,
	)
}

/// A custodian set is valid when every slot holds a real, distinct key.
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

// ---------------------------------------------------------------------------
// Instruction arguments
// ---------------------------------------------------------------------------

#[instruction(discriminator = PrivacyPoolInstruction::Initialize)]
pub struct InitializeIx {
	pub config_bump: u8,
	pub vault_bump: u8,
	pub tree_bump: u8,
	pub nullifiers_bump: u8,
	pub custodians_bump: u8,
	pub requesters_bump: u8,
	pub log_bump: u8,
	/// Initial three-member disclosure committee as concatenated 32-byte
	/// addresses.
	pub custodians: [u8; 96],
}

#[instruction(discriminator = PrivacyPoolInstruction::SetVerificationKey)]
pub struct SetVerificationKeyIx {
	pub bump: u8,
	/// Circuit slot: [`VK_SLOT_WITHDRAW`] or [`VK_SLOT_TRANSFER`].
	pub slot: u8,
	pub ic_len: u8,
	pub alpha_g1: [u8; 64],
	pub beta_g2: [u8; 128],
	pub gamma_g2: [u8; 128],
	pub delta_g2: [u8; 128],
	pub ic0: [u8; 64],
	pub ic1: [u8; 64],
	pub ic2: [u8; 64],
	pub ic3: [u8; 64],
}

#[instruction(discriminator = PrivacyPoolInstruction::SetCustodians)]
pub struct SetCustodiansIx {
	pub custodians: [u8; 96],
}

#[instruction(discriminator = PrivacyPoolInstruction::RegisterRequester)]
pub struct RegisterRequesterIx {
	pub requester: Address,
	/// Highest tier the entity may file at.
	pub max_tier: u8,
}

#[instruction(discriminator = PrivacyPoolInstruction::Deposit)]
pub struct DepositIx {
	pub bump: u8,
	/// Client-computed Poseidon commitment of the new note.
	pub commitment: [u8; 32],
	/// Per-note consent/notification public key.
	pub view_pubkey: [u8; 32],
	/// Active length of the encrypted note plaintext.
	pub envelope_len: u8,
	pub envelope: [u8; 128],
	/// Encrypted per-custodian key shares.
	pub shares: [u8; 144],
}

#[instruction(discriminator = PrivacyPoolInstruction::Withdraw)]
pub struct WithdrawIx {
	/// Nullifier derived by the circuit from the spent note's secrets.
	pub nullifier: [u8; 32],
	/// Tree root the proof's membership witness was built against.
	pub root: [u8; 32],
	pub proof_a: [u8; 64],
	pub proof_b: [u8; 128],
	pub proof_c: [u8; 64],
}

#[instruction(discriminator = PrivacyPoolInstruction::Transfer)]
pub struct TransferIx {
	pub bump: u8,
	pub nullifier: [u8; 32],
	pub root: [u8; 32],
	/// Commitment of the successor note.
	pub new_commitment: [u8; 32],
	/// Successor note's consent key.
	pub new_view_pubkey: [u8; 32],
	pub envelope_len: u8,
	pub envelope: [u8; 128],
	pub shares: [u8; 144],
	pub proof_a: [u8; 64],
	pub proof_b: [u8; 128],
	pub proof_c: [u8; 64],
}

#[instruction(discriminator = PrivacyPoolInstruction::RequestDisclosure)]
pub struct RequestDisclosureIx {
	pub bump: u8,
	pub nonce: u64,
	/// Tier being invoked; see the `TIER_*` constants.
	pub tier: u8,
	/// Target note commitment.
	pub commitment: [u8; 32],
	/// Active length of the encrypted notice.
	pub notice_len: u8,
	pub notice: [u8; 96],
	/// Hash of the legal basis; nonzero required above tier 0.
	pub legal_basis_hash: [u8; 32],
}

#[instruction(discriminator = PrivacyPoolInstruction::ResolveChallenge)]
pub struct ResolveChallengeIx {
	/// Nonzero resolves in the requester's favor (execution may proceed);
	/// zero rejects the request outright.
	pub approve: u8,
}

#[instruction(discriminator = PrivacyPoolInstruction::GrantDisclosure)]
pub struct GrantDisclosureIx {
	/// Reserved; must be zero.
	pub reserved: u8,
}

#[instruction(discriminator = PrivacyPoolInstruction::ChallengeDisclosure)]
pub struct ChallengeDisclosureIx {
	/// Reserved; must be zero.
	pub reserved: u8,
}

#[instruction(discriminator = PrivacyPoolInstruction::ApproveDisclosure)]
pub struct ApproveDisclosureIx {
	/// Reserved; must be zero.
	pub reserved: u8,
}

#[instruction(discriminator = PrivacyPoolInstruction::CancelDisclosure)]
pub struct CancelDisclosureIx {
	/// Reserved; must be zero.
	pub reserved: u8,
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
	pub nullifier_set: &'a mut AccountView,
	pub custodian_registry: &'a mut AccountView,
	pub requester_registry: &'a mut AccountView,
	pub disclosure_log: &'a mut AccountView,
	pub system_program: &'a AccountView,
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
	pub note_commitment: &'a mut AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts)]
pub struct WithdrawAccounts<'a> {
	pub pool_config: &'a AccountView,
	pub pool_vault: &'a mut AccountView,
	pub merkle_tree: &'a AccountView,
	pub nullifier_set: &'a mut AccountView,
	pub verifying_key_account: &'a AccountView,
	pub recipient: &'a mut AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts)]
pub struct TransferAccounts<'a> {
	pub pool_config: &'a AccountView,
	/// Funds the successor note's rent. Transfers are anonymous with respect
	/// to the spent note, not to fees: this example has no relayer, so the
	/// submitting wallet signs and pays. Production routes this through a
	/// relayer so even this linkage disappears.
	pub payer: &'a mut AccountView,
	pub merkle_tree: &'a mut AccountView,
	pub nullifier_set: &'a mut AccountView,
	pub verifying_key_account: &'a AccountView,
	pub note_commitment: &'a mut AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts)]
pub struct RequestDisclosureAccounts<'a> {
	pub requester: &'a mut AccountView,
	pub pool_config: &'a AccountView,
	pub requester_registry: &'a AccountView,
	pub note_commitment: &'a AccountView,
	pub disclosure_request: &'a mut AccountView,
	pub system_program: &'a AccountView,
	pub clock: &'a AccountView,
}

#[derive(Accounts)]
pub struct GrantDisclosureAccounts<'a> {
	pub disclosure_request: &'a mut AccountView,
	pub note_commitment: &'a AccountView,
	pub viewer: &'a AccountView,
}

#[derive(Accounts)]
pub struct ChallengeDisclosureAccounts<'a> {
	pub disclosure_request: &'a mut AccountView,
	pub note_commitment: &'a AccountView,
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

// ---------------------------------------------------------------------------
// Instruction: Initialize
// ---------------------------------------------------------------------------

impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = InitializeIx::try_from_bytes(data)?;

		self.authority.assert_signer()?.assert_writable()?;
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
			config.deposit_lamports = DEPOSIT_LAMPORTS.into();
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
			prefill_zero_nodes(tree.nodes.as_mut_slice())?;
			tree.roots.as_mut_slice().fill(0);
			let root = zero_hashes()?[TREE_DEPTH];
			let ring_len = usize::from(tree.roots_len.get());
			let next_len = push_root(tree.roots.as_mut_slice(), ring_len, &root)?;
			tree.roots_len.set(next_len as u16);
		}

		CreateProgramAccountWithBump {
			account: self.nullifier_set,
			payer: self.authority,
			owner: &ID,
			seeds: &NullifierSet::seeds().as_slices(),
			bump: args.nullifiers_bump,
		}
		.invoke_with::<NullifierSet>(|set| {
			set.bump = args.nullifiers_bump;
			set.count = 0.into();
			set.nullifiers.as_mut_slice().fill(0);
			Ok(())
		})?;

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
// Instruction: SetVerificationKey
// ---------------------------------------------------------------------------

impl<'a> ProcessAccountInfos<'a> for SetVerificationKeyAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = SetVerificationKeyIx::try_from_bytes(data)?;

		self.authority.assert_signer()?.assert_writable()?;
		if args.slot != VK_SLOT_WITHDRAW && args.slot != VK_SLOT_TRANSFER {
			return Err(PrivacyPoolError::InvalidVerifyingKeySlot.into());
		}
		let expected_ic = if args.slot == VK_SLOT_WITHDRAW { 3 } else { 4 };
		if usize::from(args.ic_len) != expected_ic {
			return Err(PrivacyPoolError::InvalidVerifyingKeySlot.into());
		}

		{
			let config = self.pool_config.as_account::<PoolConfig>(&ID)?;
			if config.authority != *self.authority.address() {
				drop(config);
				return Err(PrivacyPoolError::InvalidAuthority.into());
			}
		}

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
			key.ic_len = args.ic_len;
			key.ic0 = args.ic0;
			key.ic1 = args.ic1;
			key.ic2 = args.ic2;
			key.ic3 = args.ic3;
			Ok(())
		})?;

		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Instruction: SetCustodians
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

// ---------------------------------------------------------------------------
// Instruction: RegisterRequester
// ---------------------------------------------------------------------------

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
		if usize::from(registry.count) >= MAX_REQUESTERS {
			return Err(PrivacyPoolError::RequesterRegistryFull.into());
		}
		let requester_bytes = args.requester.as_ref();
		if let Some(index) = registry
			.keys
			.as_chunks::<32>()
			.0
			.iter()
			.position(|key| key == requester_bytes)
		{
			registry.max_tiers[index] = args.max_tier;
		} else {
			let index = usize::from(registry.count);
			registry.keys[index * 32..(index + 1) * 32].copy_from_slice(requester_bytes);
			registry.max_tiers[index] = args.max_tier;
			registry.count += 1;
		}

		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Instruction: Deposit
// ---------------------------------------------------------------------------

impl<'a> ProcessAccountInfos<'a> for DepositAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = DepositIx::try_from_bytes(data)?;

		self.depositor.assert_signer()?.assert_writable()?;
		self.system_program.assert_address(&system::ID)?;
		if args.commitment == [0_u8; 32] {
			return Err(PrivacyPoolError::ZeroCommitment.into());
		}
		let envelope_len = usize::from(args.envelope_len);
		if envelope_len > MAX_ENVELOPE_BYTES {
			return Err(PrivacyPoolError::BufferOverflow.into());
		}

		let deposit_lamports = {
			let config = self.pool_config.as_account::<PoolConfig>(&ID)?;
			config.deposit_lamports.get()
		};

		// Escrow the denomination first; everything after is bookkeeping.
		system::instructions::Transfer {
			from: self.depositor,
			to: self.pool_vault,
			lamports: deposit_lamports,
		}
		.invoke()?;

		{
			let mut tree = self.merkle_tree.as_account_mut::<MerkleTree>(&ID)?;
			let next_index = tree.next_leaf_index.get();
			if next_index >= TREE_CAPACITY as u64 {
				drop(tree);
				return Err(PrivacyPoolError::TreeFull.into());
			}
			let root = insert_leaf(tree.nodes.as_mut_slice(), next_index, &args.commitment)?;
			tree.next_leaf_index.set(next_index + 1);
			let ring_len = usize::from(tree.roots_len.get());
			let next_len = push_root(tree.roots.as_mut_slice(), ring_len, &root)?;
			tree.roots_len.set(next_len as u16);
		}

		CreateProgramAccountWithBump {
			account: self.note_commitment,
			payer: self.depositor,
			owner: &ID,
			seeds: &NoteCommitment::seeds(&commitment_address(&args.commitment)).as_slices(),
			bump: args.bump,
		}
		.invoke_with::<NoteCommitment>(|note| {
			note.bump = args.bump;
			note.commitment = args.commitment;
			note.view_pubkey = args.view_pubkey;
			note.envelope_len = args.envelope_len;
			note.envelope.as_mut_slice().fill(0);
			note.envelope.as_mut_slice()[..envelope_len]
				.copy_from_slice(&args.envelope[..envelope_len]);
			note.shares = args.shares;
			Ok(())
		})?;

		{
			let mut config = self.pool_config.as_account_mut::<PoolConfig>(&ID)?;
			let deposits = config.total_deposits.get();
			config.total_deposits.set(
				deposits
					.checked_add(1)
					.ok_or(PrivacyPoolError::ArithmeticOverflow)?,
			);
		}

		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Instructions: Withdraw / Transfer
// ---------------------------------------------------------------------------

/// A Groth16 proof in the big-endian wire layout.
pub struct WireProof {
	/// The (already negated) A point.
	pub a: [u8; 64],
	/// The B point.
	pub b: [u8; 128],
	/// The C point.
	pub c: [u8; 64],
}

/// Everything a spend needs at the verification boundary.
pub struct SpendRequest<'a> {
	/// Circuit slot the proof was produced for.
	pub slot: u8,
	/// Derived nullifier of the spent note.
	pub nullifier: [u8; 32],
	/// Tree root the witness proves membership against.
	pub root: [u8; 32],
	/// The wire proof.
	pub proof: WireProof,
	/// Public inputs in tree byte order.
	pub public_inputs: &'a [[u8; 32]],
}

/// Shared spend path: check the root ring, verify the Groth16 proof against
/// the slot's key, and record the nullifier.
fn spend_note(
	merkle_tree: &AccountView,
	nullifier_set: &mut AccountView,
	verifying_key: &AccountView,
	spend: &SpendRequest<'_>,
) -> Result<(), ProgramError> {
	let SpendRequest {
		slot,
		nullifier,
		root,
		proof,
		public_inputs,
	} = spend;
	VerifyingKeyAccount::assert_seeds(verifying_key, u64::from(*slot), &ID)?;

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

	{
		let key = verifying_key.as_account::<VerifyingKeyAccount>(&ID)?;
		if key.slot != *slot {
			drop(key);
			return Err(PrivacyPoolError::VerifyingKeyMissing.into());
		}
		let material = Groth16Key {
			alpha_g1: key.alpha_g1,
			beta_g2: key.beta_g2,
			gamma_g2: key.gamma_g2,
			delta_g2: key.delta_g2,
			ic: [key.ic0, key.ic1, key.ic2, key.ic3],
			ic_len: usize::from(key.ic_len),
		};
		drop(key);
		let ok = verify_groth16(&material, &proof.a, &proof.b, &proof.c, public_inputs)?;
		if !ok {
			return Err(PrivacyPoolError::ProofVerificationFailed.into());
		}
	}

	{
		let mut set = nullifier_set.as_account_mut::<NullifierSet>(&ID)?;
		let count = set.count.get();
		let next = push_nullifier(set.nullifiers.as_mut_slice(), count, nullifier)?;
		set.count.set(next);
	}

	Ok(())
}

impl<'a> ProcessAccountInfos<'a> for WithdrawAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = WithdrawIx::try_from_bytes(data)?;

		self.recipient.assert_writable()?;
		self.system_program.assert_address(&system::ID)?;

		let deposit_lamports = {
			let config = self.pool_config.as_account::<PoolConfig>(&ID)?;
			config.deposit_lamports.get()
		};

		let spend = SpendRequest {
			slot: VK_SLOT_WITHDRAW,
			nullifier: args.nullifier,
			root: args.root,
			proof: WireProof {
				a: args.proof_a,
				b: args.proof_b,
				c: args.proof_c,
			},
			public_inputs: &[args.root, args.nullifier],
		};
		spend_note(
			self.merkle_tree,
			self.nullifier_set,
			self.verifying_key_account,
			&spend,
		)?;

		// The vault carries data, so the system program refuses a CPI
		// transfer out of it; move the escrowed lamports directly between
		// the accounts instead, the standard custody-account payout.
		let vault_lamports = self.pool_vault.lamports();
		let remaining = vault_lamports
			.checked_sub(deposit_lamports)
			.ok_or(ProgramError::from(PrivacyPoolError::ArithmeticOverflow))?;
		self.pool_vault.set_lamports(remaining);
		let recipient_lamports = self.recipient.lamports();
		let payout = recipient_lamports
			.checked_add(deposit_lamports)
			.ok_or(ProgramError::from(PrivacyPoolError::ArithmeticOverflow))?;
		self.recipient.set_lamports(payout);

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for TransferAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = TransferIx::try_from_bytes(data)?;

		self.system_program.assert_address(&system::ID)?;
		if args.new_commitment == [0_u8; 32] {
			return Err(PrivacyPoolError::ZeroCommitment.into());
		}
		let envelope_len = usize::from(args.envelope_len);
		if envelope_len > MAX_ENVELOPE_BYTES {
			return Err(PrivacyPoolError::BufferOverflow.into());
		}

		let spend = SpendRequest {
			slot: VK_SLOT_TRANSFER,
			nullifier: args.nullifier,
			root: args.root,
			proof: WireProof {
				a: args.proof_a,
				b: args.proof_b,
				c: args.proof_c,
			},
			public_inputs: &[args.root, args.nullifier, args.new_commitment],
		};
		spend_note(
			self.merkle_tree,
			self.nullifier_set,
			self.verifying_key_account,
			&spend,
		)?;

		{
			let mut tree = self.merkle_tree.as_account_mut::<MerkleTree>(&ID)?;
			let next_index = tree.next_leaf_index.get();
			if next_index >= TREE_CAPACITY as u64 {
				drop(tree);
				return Err(PrivacyPoolError::TreeFull.into());
			}
			let root = insert_leaf(tree.nodes.as_mut_slice(), next_index, &args.new_commitment)?;
			tree.next_leaf_index.set(next_index + 1);
			let ring_len = usize::from(tree.roots_len.get());
			let next_len = push_root(tree.roots.as_mut_slice(), ring_len, &root)?;
			tree.roots_len.set(next_len as u16);
		}

		self.payer.assert_signer()?.assert_writable()?;
		CreateProgramAccountWithBump {
			account: self.note_commitment,
			payer: self.payer,
			owner: &ID,
			seeds: &NoteCommitment::seeds(&commitment_address(&args.new_commitment)).as_slices(),
			bump: args.bump,
		}
		.invoke_with::<NoteCommitment>(|note| {
			note.bump = args.bump;
			note.commitment = args.new_commitment;
			note.view_pubkey = args.new_view_pubkey;
			note.envelope_len = args.envelope_len;
			note.envelope.as_mut_slice().fill(0);
			note.envelope.as_mut_slice()[..envelope_len]
				.copy_from_slice(&args.envelope[..envelope_len]);
			note.shares = args.shares;
			Ok(())
		})?;

		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Instruction: RequestDisclosure
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

		// The target must be a live note account.
		NoteCommitment::assert_seeds(
			self.note_commitment,
			&commitment_address(&args.commitment),
			&ID,
		)?;

		let window = {
			let config = self.pool_config.as_account::<PoolConfig>(&ID)?;
			u64::from(config.challenge_window_secs.get())
		};
		let now = clock_timestamp(self.clock)?;
		let deadline = if args.tier == TIER_VERIFIED {
			now + window
		} else {
			now
		};

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
		})?;

		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Instructions: GrantDisclosure / ChallengeDisclosure
// ---------------------------------------------------------------------------

/// Require `viewer` to be the signer matching the target note's consent
/// key, returning the request's current status.
fn require_note_viewer(
	request: &AccountView,
	note: &AccountView,
	viewer: &AccountView,
) -> Result<(u8, u8, u64), ProgramError> {
	let commitment = {
		let request_view = request.as_account::<DisclosureRequest>(&ID)?;
		request_view.commitment
	};

	NoteCommitment::assert_seeds(note, &commitment_address(&commitment), &ID)?;
	let view_pubkey = {
		let note_view = note.as_account::<NoteCommitment>(&ID)?;
		note_view.view_pubkey
	};

	viewer.assert_signer()?;
	if viewer.address().as_ref() != view_pubkey.as_ref() {
		return Err(PrivacyPoolError::NotNoteViewer.into());
	}

	let request_view = request.as_account::<DisclosureRequest>(&ID)?;
	Ok((
		request_view.status,
		request_view.tier,
		request_view.challenge_deadline.get(),
	))
}

impl<'a> ProcessAccountInfos<'a> for GrantDisclosureAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = GrantDisclosureIx::try_from_bytes(data)?;

		let (status, ..) =
			require_note_viewer(self.disclosure_request, self.note_commitment, self.viewer)?;
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
			require_note_viewer(self.disclosure_request, self.note_commitment, self.viewer)?;
		if status != STATUS_PENDING {
			return Err(PrivacyPoolError::InvalidRequestStatus.into());
		}
		if tier != TIER_VERIFIED {
			return Err(PrivacyPoolError::InvalidRequestStatus.into());
		}
		let now = clock_timestamp(self.clock)?;
		// The subject may challenge only while the window is open; once it
		// lapses the request stands and the committee may execute.
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

// ---------------------------------------------------------------------------
// Instruction: ResolveChallenge
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Instruction: ApproveDisclosure
// ---------------------------------------------------------------------------

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

			// Threshold met: execute and log in this same instruction. The
			// log append cannot be separated from the status flip, which is
			// the protocol's no-silent-disclosure guarantee.
			request.status = STATUS_EXECUTED;
			requester = request.requester;
			commitment = request.commitment;
			nonce = request.nonce.get();
		}

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
		encode_log_entry(entry, &requester, &commitment, tier, nonce, now)?;
		log.count.set(count + 1);

		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Instruction: CancelDisclosure
// ---------------------------------------------------------------------------

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

/// Circuits, seeded trusted setup, proving, and the little-endian wire
/// encoding the on-chain verifier expects. Host-only: never compiled into
/// the SBF artifact. A wallet embeds this module; the tests use it to put
/// real Groth16 proofs through the program's syscall-backed verifier.
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
	use ark_snark::SNARK;
	use ark_std::UniformRand;
	use light_poseidon::PoseidonParameters;
	use light_poseidon::parameters::bn254_x5;
	use rand::SeedableRng;
	use rand::rngs::StdRng;

	use super::TREE_DEPTH;

	// -------------------------------------------------------------------
	// Wire encoding: field elements and curve points as little-endian bytes
	// -------------------------------------------------------------------

	/// 32-byte little-endian encoding of a field element.
	pub fn fr_to_le(value: &Fr) -> [u8; 32] {
		let mut out = [0_u8; 32];
		let bytes = value.into_bigint().to_bytes_le();
		out.copy_from_slice(&bytes);
		out
	}

	/// Field element from 32 canonical little-endian bytes. Inputs that
	/// exceed the scalar field modulus are rejected; every value this
	/// example handles round-trips through [`fr_to_le`].
	pub fn fr_from_le(bytes: &[u8; 32]) -> Option<Fr> {
		let mut limbs = [0_u64; 4];
		for (index, chunk) in bytes.as_chunks::<8>().0.iter().enumerate() {
			let mut limb = [0_u8; 8];
			limb.copy_from_slice(chunk);
			limbs[index] = u64::from_le_bytes(limb);
		}
		Fr::from_bigint(ark_ff::BigInteger256::new(limbs))
	}

	/// Serialize a G₁ point through arkworks' canonical serializer, then
	/// reverse each 32-byte coordinate into the big-endian (EIP-197) word
	/// order the runtime's group-op selectors consume.
	pub fn g1_to_be(affine: &G1Affine) -> [u8; 64] {
		use ark_serialize::CanonicalSerialize;
		use ark_serialize::Compress;
		let mut le = [0_u8; 64];
		affine
			.x
			.serialize_with_mode(&mut le[..32], Compress::No)
			.unwrap_or_else(|error| panic!("g1 x: {error:?}"));
		affine
			.y
			.serialize_with_mode(&mut le[32..], Compress::No)
			.unwrap_or_else(|error| panic!("g1 y: {error:?}"));
		reverse_chunks32(&le)
	}

	/// Serialize a G₂ point to big-endian EIP-197 bytes: serialize both
	/// 64-byte extension-field coordinates canonically, then reverse each
	/// 64-byte block, yielding `x.c1, x.c0, y.c1, y.c0` words.
	pub fn g2_to_be(affine: &G2Affine) -> [u8; 128] {
		use ark_serialize::CanonicalSerialize;
		use ark_serialize::Compress;
		let mut le = [0_u8; 128];
		affine
			.x
			.serialize_with_mode(&mut le[..64], Compress::No)
			.unwrap_or_else(|error| panic!("g2 x: {error:?}"));
		affine
			.y
			.serialize_with_mode(&mut le[64..], Compress::No)
			.unwrap_or_else(|error| panic!("g2 y: {error:?}"));
		let mut out = [0_u8; 128];
		out[..64].copy_from_slice(&le[..64].iter().rev().copied().collect::<Vec<_>>());
		out[64..].copy_from_slice(&le[64..].iter().rev().copied().collect::<Vec<_>>());
		out
	}

	/// Field element to big-endian bytes for scalar inputs.
	pub fn fr_to_be(value: &Fr) -> [u8; 32] {
		let mut out = [0_u8; 32];
		let bytes = value.into_bigint().to_bytes_be();
		out.copy_from_slice(&bytes);
		out
	}

	/// Reverse each 32-byte chunk of a 64-byte buffer.
	fn reverse_chunks32(input: &[u8; 64]) -> [u8; 64] {
		let mut out = [0_u8; 64];
		out[..32].copy_from_slice(&input[..32].iter().rev().copied().collect::<Vec<_>>());
		out[32..].copy_from_slice(&input[32..].iter().rev().copied().collect::<Vec<_>>());
		out
	}

	// -------------------------------------------------------------------
	// In-circuit Poseidon: a faithful mirror of the x5 permutation the
	// runtime syscall implements (state width 3, 8 full + 57 partial
	// rounds, x^5 S-box, output state[0]).
	// -------------------------------------------------------------------

	fn width3_parameters() -> PoseidonParameters<Fr> {
		bn254_x5::get_poseidon_parameters::<Fr>(3)
			.unwrap_or_else(|error| panic!("poseidon parameters: {error:?}"))
	}

	/// Poseidon-2 inside the constraint system. Round constants and the MDS
	/// matrix come from the same generated table the host hasher uses, so a
	/// proof about an in-circuit hash is a proof about the on-chain hash.
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
			// ark
			for (element, slot) in state.iter_mut().enumerate() {
				let constant = FpVar::constant(params.ark[round * params.width + element]);
				*slot = slot.clone() + constant;
			}
			// s-box
			let sbox = |value: &FpVar<Fr>| -> Result<FpVar<Fr>, SynthesisError> {
				// x^5 = ((x^2)^2) * x — two constraints per application.
				Ok(value.square()?.square()? * value)
			};
			if round < half || round >= half + params.partial_rounds {
				for slot in &mut state {
					*slot = sbox(slot)?;
				}
			} else {
				state[0] = sbox(&state[0])?;
			}
			// mds
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

	// -------------------------------------------------------------------
	// Circuits
	// -------------------------------------------------------------------

	/// Note secrets as the wallet holds them.
	#[derive(Clone, Copy, Debug)]
	pub struct NoteSecrets {
		/// First secret input to the commitment hash.
		pub secret: Fr,
		/// Second secret input; also derives the nullifier.
		pub nullifier_seed: Fr,
	}

	/// A spend proof's private material.
	#[derive(Clone)]
	pub struct SpendWitness {
		pub spent: NoteSecrets,
		/// Sibling hashes, leaf level first.
		pub path_elements: [Fr; TREE_DEPTH],
		/// One bit per level: 0 means the current node is the left input.
		pub path_indices: [bool; TREE_DEPTH],
		/// Successor note; present for transfers.
		pub successor: Option<NoteSecrets>,
	}

	/// Shared join-split shape: knowledge of a committed note in the tree
	/// that derives the public nullifier. Transfers additionally bind the
	/// public successor commitment to fresh secrets.
	#[derive(Clone)]
	pub struct SpendCircuit {
		/// Merkle root the witness proves membership against (public).
		pub root: Fr,
		/// Derived nullifier (public).
		pub nullifier: Fr,
		/// Successor commitment (public, transfers only).
		pub output_commitment: Option<Fr>,
		/// Fixed denomination as a field element (constant in-circuit).
		pub amount: Fr,
		/// Private witness material, provided as closures so both setup
		/// (without witnesses) and proving (with) reuse the circuit.
		pub witness: Option<SpendWitness>,
	}

	impl ConstraintSynthesizer<Fr> for SpendCircuit {
		fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
			let params = width3_parameters();

			let root = FpVar::new_input(cs.clone(), || Ok(self.root))?;
			let nullifier_pub = FpVar::new_input(cs.clone(), || Ok(self.nullifier))?;
			let output = match self.output_commitment {
				Some(value) => Some(FpVar::new_input(cs.clone(), || Ok(value))?),
				None => None,
			};

			let witness = self.witness;
			let allocate = |value: Fr| -> Option<Fr> { Some(value) };

			let secret = FpVar::new_witness(cs.clone(), || {
				witness
					.as_ref()
					.and_then(|w| allocate(w.spent.secret))
					.ok_or(SynthesisError::AssignmentMissing)
			})?;
			let seed = FpVar::new_witness(cs.clone(), || {
				witness
					.as_ref()
					.and_then(|w| allocate(w.spent.nullifier_seed))
					.ok_or(SynthesisError::AssignmentMissing)
			})?;
			let amount = FpVar::constant(self.amount);

			// commitment = poseidon(secret, seed, amount)
			let commitment =
				poseidon2_var(&params, &poseidon2_var(&params, &secret, &seed)?, &amount)?;

			// merkle fold
			let mut current = commitment;
			for level in 0..TREE_DEPTH {
				let sibling = FpVar::new_witness(cs.clone(), || {
					witness
						.as_ref()
						.map(|w| w.path_elements[level])
						.ok_or(SynthesisError::AssignmentMissing)
				})?;
				let index = FpVar::new_witness(cs.clone(), || {
					Ok(Fr::from(u64::from(
						witness.as_ref().is_some_and(|w| w.path_indices[level]),
					)))
				})?;
				// select left/right placement: one_of linear choice
				let chosen_left = select_left(&index, &current, &sibling);
				let chosen_right = select_left(&index, &sibling, &current);
				current = poseidon2_var(&params, &chosen_left, &chosen_right)?;
			}
			current.enforce_equal(&root)?;

			// nullifier = poseidon(seed, secret)
			let nullifier = poseidon2_var(&params, &seed, &secret)?;
			nullifier.enforce_equal(&nullifier_pub)?;

			// transfer: bind the public successor commitment to fresh secrets
			if let Some(output_var) = output {
				let successor = FpVar::new_witness(cs.clone(), || {
					witness
						.as_ref()
						.and_then(|w| w.successor.map(|s| s.secret))
						.ok_or(SynthesisError::AssignmentMissing)
				})?;
				let successor_seed = FpVar::new_witness(cs.clone(), || {
					witness
						.as_ref()
						.and_then(|w| w.successor.map(|s| s.nullifier_seed))
						.ok_or(SynthesisError::AssignmentMissing)
				})?;
				let next = poseidon2_var(
					&params,
					&poseidon2_var(&params, &successor, &successor_seed)?,
					&amount,
				)?;
				next.enforce_equal(&output_var)?;
			}

			Ok(())
		}
	}

	/// `index == 0` yields `on_zero`, `index == 1` yields `on_one`:
	/// `on_zero + index * (on_one - on_zero)`.
	fn select_left(index: &FpVar<Fr>, on_zero: &FpVar<Fr>, on_one: &FpVar<Fr>) -> FpVar<Fr> {
		on_zero + index * (on_one - on_zero)
	}

	// -------------------------------------------------------------------
	// Setup, proving, and witness assembly
	// -------------------------------------------------------------------

	/// A deterministic proving key pair for one circuit shape. The seed is
	/// fixed by the caller so test fixtures and recorded instructions stay
	/// reproducible; production deployments replace this with a ceremony.
	pub fn seeded_setup(with_output: bool, seed: u64) -> (ProvingKey<Bn254>, VerifyingKey<Bn254>) {
		let mut rng = StdRng::seed_from_u64(seed);
		let circuit = SpendCircuit {
			root: Fr::zero(),
			nullifier: Fr::zero(),
			output_commitment: with_output.then(Fr::zero),
			amount: Fr::from(super::DEPOSIT_LAMPORTS),
			witness: None,
		};
		Groth16::<Bn254>::circuit_specific_setup(circuit, &mut rng)
			.unwrap_or_else(|error| panic!("setup: {error:?}"))
	}

	/// Prove a spend with the deterministic rng seeded per note so recorded
	/// instruction paths are reproducible.
	pub fn prove_spend(pk: &ProvingKey<Bn254>, circuit: SpendCircuit, seed: u64) -> Proof<Bn254> {
		let mut rng = StdRng::seed_from_u64(seed);
		Groth16::<Bn254>::prove(pk, circuit, &mut rng)
			.unwrap_or_else(|error| panic!("prove: {error:?}"))
	}

	/// Verify a proof on the host, as a sanity mirror of the on-chain check.
	pub fn verify_host(
		vk: &VerifyingKey<Bn254>,
		public_inputs: &[Fr],
		proof: &Proof<Bn254>,
	) -> bool {
		Groth16::<Bn254>::verify(vk, public_inputs, proof).unwrap_or(false)
	}

	/// Serialized verifying key in the on-chain account layout.
	pub struct WireVerifyingKey {
		pub alpha_g1: [u8; 64],
		pub beta_g2: [u8; 128],
		pub gamma_g2: [u8; 128],
		pub delta_g2: [u8; 128],
		pub ic_len: u8,
		pub ic: [[u8; 64]; 4],
	}

	/// Serialize a verifying key into the little-endian wire layout.
	pub fn serialize_vk(vk: &VerifyingKey<Bn254>) -> WireVerifyingKey {
		let mut ic = [[0_u8; 64]; 4];
		for (slot, point) in vk.gamma_abc_g1.iter().take(4).enumerate() {
			ic[slot] = g1_to_be(point);
		}
		WireVerifyingKey {
			alpha_g1: g1_to_be(&vk.alpha_g1),
			beta_g2: g2_to_be(&vk.beta_g2),
			gamma_g2: g2_to_be(&vk.gamma_g2),
			delta_g2: g2_to_be(&vk.delta_g2),
			ic_len: vk.gamma_abc_g1.len() as u8,
			ic,
		}
	}

	/// Serialized proof in the on-chain instruction layout.
	pub struct WireProof {
		pub a: [u8; 64],
		pub b: [u8; 128],
		pub c: [u8; 64],
	}

	/// Serialize a proof into the little-endian wire layout.
	pub fn serialize_proof(proof: &Proof<Bn254>) -> WireProof {
		// A is negated here so the on-chain equation pairs four positive
		// points: e(−A, B) · e(IC(x), γ) · e(C, δ) · e(α, β) = 1.
		let negated_a: G1Affine = -proof.a;
		WireProof {
			a: g1_to_be(&negated_a),
			b: g2_to_be(&proof.b),
			c: g1_to_be(&proof.c),
		}
	}

	/// Build a membership witness against the tree's flat node store.
	/// `nodes` is the on-chain `[u8; TREE_NODES * 32]` field read back over
	/// RPC, `leaf_index` the spent note's position.
	pub fn build_witness(
		nodes: &[u8],
		leaf_index: usize,
	) -> ([Fr; TREE_DEPTH], [bool; TREE_DEPTH]) {
		let mut path_elements = [Fr::zero(); TREE_DEPTH];
		let mut path_indices = [false; TREE_DEPTH];
		let mut position = leaf_index;
		for level in 0..TREE_DEPTH {
			let sibling_position = position ^ 1;
			let level_offset = (1 << (TREE_DEPTH + 1)) - (1 << (TREE_DEPTH + 1 - level));
			let start = (level_offset + sibling_position) * 32;
			let mut bytes = [0_u8; 32];
			bytes.copy_from_slice(
				nodes
					.get(start..start + 32)
					.unwrap_or_else(|| panic!("witness level {level} out of bounds")),
			);
			path_elements[level] =
				fr_from_le(&bytes).unwrap_or_else(|| panic!("non-canonical sibling"));
			path_indices[level] = position & 1 == 1;
			position >>= 1;
		}
		(path_elements, path_indices)
	}

	/// The fixed denomination as a scalar-field element.
	pub fn amount_field() -> Fr {
		Fr::from(super::DEPOSIT_LAMPORTS)
	}

	/// Fresh random note secrets from a seeded rng.
	pub fn note_secrets(seed: u64) -> NoteSecrets {
		let mut rng = StdRng::seed_from_u64(seed);
		NoteSecrets {
			secret: Fr::rand(&mut rng),
			nullifier_seed: Fr::rand(&mut rng),
		}
	}

	#[cfg(test)]
	mod tests {
		use super::*;

		fn u256_from_be(bytes: &[u8]) -> ark_ff::BigInteger256 {
			let mut limbs = [0_u64; 4];
			for index in 0..4 {
				let mut chunk = [0_u8; 8];
				chunk.copy_from_slice(&bytes[index * 8..(index + 1) * 8]);
				limbs[3 - index] = u64::from_be_bytes(chunk);
			}
			ark_ff::BigInteger256::new(limbs)
		}

		#[test]
		fn wire_codec_round_trips_points() {
			use ark_ec::short_weierstrass::SWCurveConfig;
			let g1: ark_bn254::g1::G1Affine = ark_bn254::g1::Config::GENERATOR;
			let wire = g1_to_be(&g1);
			assert_eq!(ark_bn254::Fq::from(u256_from_be(&wire[..32])), g1.x, "g1 x");
			assert_eq!(ark_bn254::Fq::from(u256_from_be(&wire[32..])), g1.y, "g1 y");

			let g2: ark_bn254::g2::G2Affine = ark_bn254::g2::Config::GENERATOR;
			let wire2 = g2_to_be(&g2);
			assert_eq!(
				ark_bn254::Fq::from(u256_from_be(&wire2[..32])),
				g2.x.c1,
				"g2 x.c1"
			);
			assert_eq!(
				ark_bn254::Fq::from(u256_from_be(&wire2[32..64])),
				g2.x.c0,
				"g2 x.c0"
			);
		}

		#[test]
		fn host_verifier_accepts_a_real_proof_and_rejects_a_bad_one() {
			use super::super::Groth16Key;
			use super::super::verify_groth16;

			let amount = Fr::from(super::super::DEPOSIT_LAMPORTS);
			let secrets = note_secrets(0x4321);
			let inner = super::super::poseidon2(
				&fr_to_le(&secrets.secret),
				&fr_to_le(&secrets.nullifier_seed),
			)
			.unwrap_or_else(|e| panic!("inner: {e:?}"));
			let mut amount_le = [0_u8; 32];
			amount_le[..8].copy_from_slice(&super::super::DEPOSIT_LAMPORTS.to_le_bytes());
			let commitment = super::super::poseidon2(&inner, &amount_le)
				.unwrap_or_else(|e| panic!("commitment: {e:?}"));
			let mut nodes = vec![0_u8; super::super::TREE_NODES_BYTES];
			super::super::prefill_zero_nodes(&mut nodes).unwrap();
			let root = super::super::insert_leaf(&mut nodes, 0, &commitment).unwrap();
			let (path_elements, path_indices) = build_witness(&nodes, 0);
			let nullifier = super::super::poseidon2(
				&fr_to_le(&secrets.nullifier_seed),
				&fr_to_le(&secrets.secret),
			)
			.unwrap_or_else(|e| panic!("nullifier: {e:?}"));

			let (pk, vk) = seeded_setup(false, 0x7777);
			let circuit = SpendCircuit {
				root: fr_from_le(&root).unwrap(),
				nullifier: fr_from_le(&nullifier).unwrap(),
				output_commitment: None,
				amount,
				witness: Some(SpendWitness {
					spent: secrets,
					path_elements,
					path_indices,
					successor: None,
				}),
			};
			let proof = prove_spend(&pk, circuit, 0x8888);
			assert!(verify_host(
				&vk,
				&[fr_from_le(&root).unwrap(), fr_from_le(&nullifier).unwrap()],
				&proof
			));

			let wire_vk = serialize_vk(&vk);
			let wire_proof = serialize_proof(&proof);
			let key = Groth16Key {
				alpha_g1: wire_vk.alpha_g1,
				beta_g2: wire_vk.beta_g2,
				gamma_g2: wire_vk.gamma_g2,
				delta_g2: wire_vk.delta_g2,
				ic: wire_vk.ic,
				ic_len: usize::from(wire_vk.ic_len),
			};
			let inputs = [root, nullifier];
			assert!(
				verify_groth16(&key, &wire_proof.a, &wire_proof.b, &wire_proof.c, &inputs)
					.unwrap_or_else(|e| panic!("host verify: {e:?}"))
			);

			// A flipped public input must fail.
			let mut bad = root;
			bad[0] ^= 0xFF;
			assert!(
				!verify_groth16(
					&key,
					&wire_proof.a,
					&wire_proof.b,
					&wire_proof.c,
					&[bad, nullifier]
				)
				.unwrap()
			);
		}

		fn circuit_is_satisfied_by_a_consistent_witness() {
			let amount = Fr::from(super::super::DEPOSIT_LAMPORTS);
			let secrets = note_secrets(0x1234);

			// Host commitment, host tree, host witness — the same helpers
			// the e2e suite drives against the program.
			let inner = super::super::poseidon2(
				&fr_to_le(&secrets.secret),
				&fr_to_le(&secrets.nullifier_seed),
			)
			.unwrap_or_else(|e| panic!("inner: {e:?}"));
			let mut amount_le = [0_u8; 32];
			amount_le[..8].copy_from_slice(&super::super::DEPOSIT_LAMPORTS.to_le_bytes());
			let commitment_bytes = super::super::poseidon2(&inner, &amount_le)
				.unwrap_or_else(|e| panic!("commitment: {e:?}"));

			let mut nodes = vec![0_u8; super::super::TREE_NODES_BYTES];
			super::super::prefill_zero_nodes(&mut nodes).unwrap();
			let root_bytes = super::super::insert_leaf(&mut nodes, 0, &commitment_bytes).unwrap();

			let (path_elements, path_indices) = build_witness(&nodes, 0);
			let nullifier_bytes = super::super::poseidon2(
				&fr_to_le(&secrets.nullifier_seed),
				&fr_to_le(&secrets.secret),
			)
			.unwrap_or_else(|e| panic!("nullifier: {e:?}"));

			let circuit = SpendCircuit {
				root: fr_from_le(&root_bytes).unwrap(),
				nullifier: fr_from_le(&nullifier_bytes).unwrap(),
				output_commitment: None,
				amount,
				witness: Some(SpendWitness {
					spent: secrets,
					path_elements,
					path_indices,
					successor: None,
				}),
			};

			let cs = ark_relations::r1cs::ConstraintSystem::<Fr>::new_ref();
			circuit.clone().generate_constraints(cs.clone()).unwrap();
			assert!(
				cs.is_satisfied().unwrap(),
				"unsatisfied: {:?}",
				cs.which_is_unsatisfied().unwrap()
			);
		}
	}
}

#[cfg(test)]
mod tests {
	extern crate alloc;

	use super::*;

	fn hash_pair(left: [u8; 32], right: [u8; 32]) -> [u8; 32] {
		poseidon2(&left, &right).unwrap_or_else(|e| panic!("poseidon: {e:?}"))
	}

	fn prefilled_nodes() -> alloc::vec::Vec<u8> {
		let mut nodes = alloc::vec![0_u8; TREE_NODES_BYTES];
		prefill_zero_nodes(&mut nodes).unwrap_or_else(|e| panic!("prefill: {e:?}"));
		nodes
	}

	#[test]
	fn empty_tree_root_is_zero_hash_chain_tip() {
		let zeros = zero_hashes().unwrap_or_else(|e| panic!("zeros: {e:?}"));
		let nodes = prefilled_nodes();
		assert_eq!(read_node(&nodes, TREE_DEPTH, 0).unwrap(), zeros[TREE_DEPTH]);
	}

	/// Fold `node` (already a `start_level` node) up to the root.
	fn chain_from(
		mut node: [u8; 32],
		zeros: &[[u8; 32]; TREE_DEPTH + 1],
		start_level: usize,
	) -> [u8; 32] {
		for level in start_level..=TREE_DEPTH {
			node = hash_pair(node, zeros[level - 1]);
		}
		node
	}

	#[test]
	fn insert_two_leaves_produces_expected_root() {
		let zeros = zero_hashes().unwrap_or_else(|e| panic!("zeros: {e:?}"));
		let mut nodes = prefilled_nodes();

		let leaf0 = [1_u8; 32];
		let leaf1 = [2_u8; 32];
		let root = insert_leaf(&mut nodes, 0, &leaf0).unwrap();
		assert_eq!(read_node(&nodes, 0, 0).unwrap(), leaf0);
		// A single leaf pairs with the zero sibling at every level.
		assert_eq!(root, chain_from(hash_pair(leaf0, zeros[0]), &zeros, 2));

		let root = insert_leaf(&mut nodes, 1, &leaf1).unwrap();
		let pair = hash_pair(leaf0, leaf1);
		let mut expected = hash_pair(pair, zeros[1]);
		for level in 3..=TREE_DEPTH {
			expected = hash_pair(expected, zeros[level - 1]);
		}
		assert_eq!(root, expected);
	}

	#[test]
	fn tree_capacity_is_enforced() {
		let mut nodes = prefilled_nodes();
		for index in 0..TREE_CAPACITY {
			insert_leaf(&mut nodes, index as u64, &[7_u8; 32]).unwrap();
		}
		assert_eq!(
			insert_leaf(&mut nodes, TREE_CAPACITY as u64, &[7_u8; 32]),
			Err(PrivacyPoolError::TreeFull.into())
		);
	}

	#[test]
	fn nullifier_set_deduplicates_and_fills() {
		let mut nullifiers = alloc::vec![0_u8; NULLIFIER_BYTES];
		let mut count = 0_u64;
		for index in 0..NULLIFIER_CAPACITY as u64 {
			// `index + 1` keeps every entry distinct from the zero padding.
			count = push_nullifier(&mut nullifiers, count, &[index as u8 + 1; 32]).unwrap();
		}
		assert_eq!(count, NULLIFIER_CAPACITY as u64);
		// Duplicate detection precedes the capacity check.
		assert_eq!(
			push_nullifier(&mut nullifiers, count, &[1_u8; 32]),
			Err(PrivacyPoolError::NullifierAlreadySpent.into())
		);
		// A fresh value at capacity is rejected as full.
		assert_eq!(
			push_nullifier(&mut nullifiers, count, &[0xEE_u8; 32]),
			Err(PrivacyPoolError::NullifierSetFull.into())
		);
	}

	#[test]
	fn root_ring_evicts_oldest_only_past_capacity() {
		let mut roots = alloc::vec![0_u8; ROOT_RING_BYTES];
		let mut len = 0;
		// Up to capacity, roots land in order and nothing is evicted. The
		// all-zero root is a legal value: the ring tracks length explicitly.
		for round in 0..ROOT_RING_CAPACITY as u8 {
			len = push_root(&mut roots, len, &[round; 32]).unwrap();
		}
		assert_eq!(len, ROOT_RING_CAPACITY);
		assert!(root_known(&roots, len, &[0_u8; 32]));
		for round in 0..ROOT_RING_CAPACITY as u8 {
			assert_eq!(&roots[round as usize * 32..][..32], &[round; 32][..]);
		}
		// Two more pushes evict the two oldest roots; the survivors are
		// [2, 3, 4, 5, 6, 7] followed by the two newcomers.
		len = push_root(&mut roots, len, &[9_u8; 32]).unwrap();
		len = push_root(&mut roots, len, &[10_u8; 32]).unwrap();
		assert_eq!(len, ROOT_RING_CAPACITY);
		let expected_ring = [2_u8, 3, 4, 5, 6, 7, 9, 10];
		for (slot, expected) in expected_ring.iter().enumerate() {
			assert_eq!(&roots[slot * 32..(slot + 1) * 32], &[*expected; 32][..]);
		}
		assert!(!root_known(&roots, len, &[0_u8; 32]));
		assert!(root_known(&roots, len, &[10_u8; 32]));
	}

	#[test]
	fn root_ring_membership_follows_insertion() {
		let mut roots = alloc::vec![0_u8; ROOT_RING_BYTES];
		assert!(!root_known(&roots, 0, &[5_u8; 32]));
		let len = push_root(&mut roots, 0, &[5_u8; 32]).unwrap();
		assert!(root_known(&roots, len, &[5_u8; 32]));
	}

	#[test]
	fn log_entry_encoding_round_trips_fields() {
		let mut entry = [0_u8; LOG_ENTRY_BYTES];
		let requester = Address::new_from_array([9_u8; 32]);
		encode_log_entry(
			&mut entry,
			&requester,
			&[7_u8; 32],
			TIER_VERIFIED,
			42,
			1_700,
		)
		.unwrap();
		assert_eq!(&entry[..32], &[9_u8; 32]);
		assert_eq!(&entry[32..64], &[7_u8; 32]);
		assert_eq!(entry[64], TIER_VERIFIED);
		assert_eq!(&entry[72..80], &42_u64.to_le_bytes());
		assert_eq!(&entry[80..88], &1_700_u64.to_le_bytes());
		assert!(entry[88..].iter().all(|byte| *byte == 0));
	}

	#[test]
	fn custodian_set_validation_rejects_zeros_and_duplicates() {
		let mut set = [0_u8; 96];
		set[..32].fill(1);
		set[32..64].fill(1);
		set[64..].fill(3);
		assert!(validate_custodian_set(&set).is_err());

		let mut distinct = [0_u8; 96];
		distinct[..32].fill(1);
		distinct[32..64].fill(2);
		distinct[64..].fill(3);
		assert!(validate_custodian_set(&distinct).is_ok());

		// A zero slot is rejected.
		let mut zeroed = distinct;
		zeroed[32..64].fill(0);
		assert!(validate_custodian_set(&zeroed).is_err());
	}
}
