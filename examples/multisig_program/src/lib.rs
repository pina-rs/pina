//! An ambitious multisig wallet built with pina.
//!
//! This example implements a production-shaped multisig — permissioned
//! members, threshold approvals, timelocked execution, vault transactions,
//! governed configuration, and spending limits — on pina's `no_std`,
//! zero-copy runtime, informed by studying the deployed multisig programs
//! the ecosystem actually uses. Beyond a faithful consensus core it
//! deliberately improves on the common designs:
//!
//! - One unified [`Proposal`] account carries the vote state *and* the
//!   payload (a compiled vault message or a config action stream), where the
//!   usual split keeps a proposal account plus a separate transaction
//!   account per kind.
//! - Votes are `u32` bitmasks over the sorted member list instead of three
//!   sorted `Vec<Pubkey>` collections, so voting is constant-time word
//!   arithmetic, proposal accounts never grow while voting, and header-only
//!   updates write in place without a resize CPI.
//! - The multisig stores members in compact tails, paying rent only for
//!   active members, and every state read is a zero-copy view.
//! - Any approver may revoke their approval; when approvals drop below the
//!   threshold an approved proposal becomes active again, and a fresh
//!   timelock applies after re-approval.
//! - Proposals carry a configurable lifetime ([`MultisigInstruction`]
//!   creation records an expiry, and the multisig's `ttl` governs it), so
//!   stale consent cannot linger forever — a governance-program idea the
//!   classic multisigs lack.
//! - Config changes are one typed action stream executed by a single
//!   governed instruction (autonomous multisigs) or directly by the config
//!   authority (controlled multisigs), instead of a family of separate
//!   direct setters.
//! - [`MultisigInstruction::MultisigImport`] adopts an existing on-chain
//!   multisig account written in the classic Anchor layout (8-byte account
//!   discriminator, inline `Vec<Member>` roster), zero-copy parsing it and
//!   seeding this program's compact state from the roster and consensus
//!   parameters.
//! - Every account is enrolled in the pina migration envelope, so future
//!   layout changes ship checked-in transitions instead of forks.
//!
//! Consensus core: permission masks (initiate/vote/execute), draft →
//! active → approved/rejected/cancelled → executed lifecycle, timelock
//! between approval and execution, stale-transaction invalidation on
//! consensus changes, vault PDAs with per-proposal ephemeral signers,
//! spending limits with period resets, rent-collection closes, and a global
//! program config with a multisig creation fee.
//!
//! This is a teaching scaffold, not a production wallet: vault messages are
//! capped at [`MAX_MESSAGE_BYTES`] so a proposal always fits in one
//! transaction (production programs stage larger ones through a buffer
//! account), address table lookups and batches are out of scope, and funds
//! held by a legacy vault stay under the legacy program's control after an
//! import. See the production-readiness guide before shipping anything
//! derived from this.

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
use pina::pinocchio::cpi::invoke_signed_with_bounds;
use pina::*;

declare_id!("5BeQ7VMZHYdnUD6PyrMd29WQo2DLfo7N2NDXCDQZ5MQc");

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const SEED_PROGRAM_CONFIG: &[u8] = b"multisig-program-config";
const SEED_MULTISIG: &[u8] = b"multisig";
const SEED_PROPOSAL: &[u8] = b"proposal";
const SEED_SPENDING_LIMIT: &[u8] = b"spending-limit";
const SEED_VAULT: &[u8] = b"vault";
const SEED_EPHEMERAL_SIGNER: &[u8] = b"ephemeral-signer";

/// Maximum members per multisig. The vote masks are `u32`, so the cap keeps
/// one bit per member with room to spare; the fixed create-instruction
/// roster also has to share one transaction packet with the account list,
/// which caps it well below the mask width.
pub const MAX_MEMBERS: usize = 16;

/// Upper bound on the timelock so a fat-fingered config cannot brick the
/// multisig, matching the classic bound: three months.
pub const MAX_TIME_LOCK: u32 = 3 * 30 * 24 * 60 * 60;

/// Unique account keys a vault message may reference. The cap bounds the
/// stack-allocated CPI account table on BPF alongside the signer set.
pub const MAX_MESSAGE_ACCOUNTS: usize = 16;

/// Encoded vault message cap. A proposal creation instruction must fit in one
/// transaction, so the message shares the packet budget with the account
/// list. Production programs stage larger messages through a buffer account.
pub const MAX_MESSAGE_BYTES: usize = 640;

/// Inner instructions a vault message may contain.
pub const MAX_MESSAGE_INSTRUCTIONS: usize = 8;

/// Encoded config action stream cap.
pub const MAX_ACTIONS_BYTES: usize = 128;

/// Ephemeral signing PDAs a single proposal may derive. The cap also bounds
/// the vault-execute stack frame: every ephemeral signer owns a seed array
/// and a `Signer` on the BPF stack.
pub const MAX_EPHEMERAL_SIGNERS: usize = 4;

/// Members allowed to draw on one spending limit.
pub const MAX_SPENDING_LIMIT_MEMBERS: usize = MAX_MEMBERS;

/// Destinations one spending limit may be restricted to.
pub const MAX_SPENDING_LIMIT_DESTINATIONS: usize = 8;

/// Upper bound on a proposal's lifetime so a fat-fingered config cannot
/// brick the multisig or strand consent forever. Mirrors the timelock
/// bound: three months.
pub const MAX_PROPOSAL_TTL: u32 = 3 * 30 * 24 * 60 * 60;

/// Permission bits shared with the classic multisig layouts.
pub const PERMISSION_INITIATE: u8 = 1 << 0;
pub const PERMISSION_VOTE: u8 = 1 << 1;
pub const PERMISSION_EXECUTE: u8 = 1 << 2;
pub const PERMISSIONS_ALL: u8 = PERMISSION_INITIATE | PERMISSION_VOTE | PERMISSION_EXECUTE;

/// Proposal payload discriminators.
pub const KIND_VAULT: u8 = 0;
pub const KIND_CONFIG: u8 = 1;

/// Proposal lifecycle statuses.
pub const STATUS_DRAFT: u8 = 0;
pub const STATUS_ACTIVE: u8 = 1;
pub const STATUS_APPROVED: u8 = 2;
pub const STATUS_REJECTED: u8 = 3;
pub const STATUS_EXECUTED: u8 = 4;
pub const STATUS_CANCELLED: u8 = 5;

/// Spending limit reset periods.
pub const PERIOD_ONE_TIME: u8 = 0;
pub const PERIOD_DAY: u8 = 1;
pub const PERIOD_WEEK: u8 = 2;
pub const PERIOD_MONTH: u8 = 3;

const DAY_SECONDS: i64 = 24 * 60 * 60;
const WEEK_SECONDS: i64 = 7 * DAY_SECONDS;
const MONTH_SECONDS: i64 = 30 * DAY_SECONDS;

// Rent-exemption head room for the reserved inline `Migrate` route: layout
// transitions in this program move a few bytes, and roughly 6,960 lamports
// cover each grown byte at the two-year exemption threshold.
const MAX_INLINE_MIGRATION_LAMPORTS: u64 = 20_000;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultisigError {
	/// The signer is not a member of the multisig.
	NotAMember = 0,
	/// The member lacks the permission the instruction requires.
	Unauthorized = 1,
	/// The threshold is zero or exceeds the number of voting members.
	InvalidThreshold = 2,
	/// The member list exceeds [`MAX_MEMBERS`].
	TooManyMembers = 3,
	/// The member list contains a duplicate or unsorted key.
	DuplicateMember = 4,
	/// A member carries a permission bit outside the defined set.
	UnknownPermission = 5,
	/// The multisig needs at least one member with each core permission.
	InvalidConfiguration = 6,
	/// The timelock exceeds [`MAX_TIME_LOCK`].
	TimeLockExceedsMaxAllowed = 7,
	/// The proposal is not in the status this instruction requires.
	InvalidProposalStatus = 8,
	/// The proposal predates the last consensus change and is stale.
	StaleProposal = 9,
	/// The member has already cast this vote.
	AlreadyVoted = 10,
	/// The member has no approval to revoke.
	HasNotApproved = 11,
	/// The timelock has not elapsed since approval.
	TimeLockNotReleased = 12,
	/// The encoded vault message is malformed or exceeds a capacity.
	InvalidMessage = 13,
	/// The remaining accounts do not match the message account keys.
	InvalidNumberOfAccounts = 14,
	/// An account does not match the key, signer, or writability the message
	/// declares.
	InvalidAccount = 15,
	/// A message instruction wants to write a program-owned account the
	/// multisig must protect.
	ProtectedAccount = 16,
	/// The encoded config action stream is malformed or exceeds a capacity.
	InvalidActions = 17,
	/// A config action references a spending limit account that is missing.
	MissingAccount = 18,
	/// Governed config transactions require an autonomous multisig.
	NotSupportedForControlled = 19,
	/// The config-authority path requires a controlled multisig.
	NotSupportedForAutonomous = 20,
	/// The configured creation fee is positive but the treasury account is
	/// missing.
	MissingTreasury = 21,
	/// The spending limit is exhausted for this period.
	SpendingLimitExceeded = 22,
	/// The destination is not on the spending limit's allow-list.
	InvalidDestination = 23,
	/// The mint does not match the spending limit.
	InvalidMint = 24,
	/// The decimals do not match the mint (SOL always has nine).
	DecimalsMismatch = 25,
	/// The reset period is not one of the defined variants.
	InvalidPeriod = 26,
	/// The legacy account is not a multisig this program can import.
	InvalidLegacyMultisig = 27,
	/// The program config authority does not match the signer.
	InvalidConfigAuthority = 28,
	/// The proposal kind does not match the instruction.
	InvalidProposalKind = 29,
	/// A spending limit action needs its rent payer and system program.
	MissingRentPayer = 30,
	/// The proposal's recorded lifetime has elapsed.
	ProposalExpired = 31,
}

// ---------------------------------------------------------------------------
// Discriminators
// ---------------------------------------------------------------------------

/// The `migrations(...)` list is the reserved `Migrate` instruction's slot
/// order: `[payer, systemProgram, multisig, proposal, spendingLimit,
/// programConfig]`. Generated clients derive the same order from the IDL.
#[discriminator(
	entrypoint,
	migrations(Multisig, Proposal, SpendingLimit, ProgramConfig),
	migrations_max_lamports = MAX_INLINE_MIGRATION_LAMPORTS,
	inline = "hint"
)]
pub enum MultisigInstruction {
	ConfigInitialize = 0,
	ConfigUpdate = 1,
	MultisigCreate = 2,
	MultisigImport = 3,
	ProposalCreate = 4,
	ProposalActivate = 5,
	ProposalApprove = 6,
	ProposalReject = 7,
	ProposalRevoke = 8,
	ProposalCancel = 9,
	VaultExecute = 10,
	ConfigExecute = 11,
	ConfigAuthorityExecute = 12,
	SpendingLimitUse = 13,
	ProposalClose = 14,
}

#[discriminator]
pub enum MultisigAccountType {
	ProgramConfig = 1,
	Multisig = 2,
	Proposal = 3,
	SpendingLimit = 4,
}

#[discriminator]
pub enum MultisigEventType {
	ProposalStatus = 1,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Global program configuration: the authority that may update it, the
/// treasury that collects multisig creation fees, and the fee itself.
#[account(discriminator = MultisigAccountType)]
#[pda(seeds = [SEED_PROGRAM_CONFIG], bump = bump)]
pub struct ProgramConfig {
	pub bump: u8,
	pub authority: Address,
	pub treasury: Address,
	pub creation_fee: u64,
}

/// A multisig: consensus parameters plus the sorted member roster.
///
/// Members live in two parallel compact tails (`member_keys` sorted ascending,
/// `member_permissions` aligned by index) so the account only pays rent for
/// active members. A member's bit index in a proposal's vote masks is its
/// position in `member_keys`.
#[account(discriminator = MultisigAccountType, compact)]
#[pda(seeds = [SEED_MULTISIG, create_key: Address], bump = bump)]
pub struct Multisig {
	pub bump: u8,
	/// Key that seeds the multisig PDA and namespaces proposals.
	pub create_key: Address,
	/// An autonomous multisig (default key) changes config through proposals;
	/// a controlled multisig defers to this key via
	/// [`MultisigInstruction::ConfigAuthorityExecute`].
	pub config_authority: Address,
	/// Where terminal proposal rent is reclaimed; the default address
	/// disables collection.
	pub rent_collector: Address,
	/// Signatures required to approve a proposal.
	pub threshold: u16,
	/// Seconds between approval and permissible execution.
	pub timelock: u32,
	/// Lifetime stamped onto new proposals; zero disables expiry.
	pub ttl: u32,
	/// Last allocated proposal index; counts both kinds.
	pub transaction_index: u64,
	/// Proposals at or below this index predate the latest consensus change.
	pub stale_transaction_index: u64,
	/// Sorted member roster as concatenated 32-byte addresses, parallel to
	/// `member_permissions`. Raw bytes keep the generated CLI clients
	/// renderable while the compact tail still charges rent per member.
	pub member_roster: PodVec<u8, 512, 2>,
	/// Permission masks, parallel to the roster.
	pub member_permissions: PodVec<u8, 16, 1>,
}

/// A proposal: vote state and payload in one account.
///
/// The payload is stored exactly as the wire format the program re-parses at
/// execution, validated once at creation, so there is a single encoding of a
/// vault message or config action stream between creation and execution.
#[account(discriminator = MultisigAccountType, compact)]
#[pda(seeds = [SEED_PROPOSAL, multisig: Address, index: u64], bump = bump)]
pub struct Proposal {
	pub bump: u8,
	pub multisig: Address,
	pub creator: Address,
	/// The index this proposal occupies in the multisig's namespace.
	pub index: u64,
	/// [`KIND_VAULT`] or [`KIND_CONFIG`].
	pub kind: u8,
	/// Vault index for vault proposals; ignored for config proposals.
	pub vault_index: u8,
	/// Canonical bump of the vault PDA for vault proposals.
	pub vault_bump: u8,
	/// Lifecycle status; see the `STATUS_*` constants.
	pub status: u8,
	/// When the current status was set. For approved proposals this anchors
	/// the timelock.
	pub status_at: i64,
	/// When the proposal dies; zero means it never expires.
	pub expires_at: i64,
	/// Approval bitmask over the multisig's sorted member list.
	pub approved_mask: u32,
	/// Rejection bitmask over the multisig's sorted member list.
	pub rejected_mask: u32,
	/// Ephemeral signer bumps, one per ephemeral PDA, for vault proposals.
	pub ephemeral_bumps: PodVec<u8, 4, 1>,
	/// Encoded vault message ([`MAX_MESSAGE_BYTES`] cap).
	pub message: PodVec<u8, 640, 2>,
	/// Encoded config action stream ([`MAX_ACTIONS_BYTES`] cap).
	pub actions: PodVec<u8, 128, 2>,
}

/// A member-scoped allowance to spend from a vault without a vote.
#[account(discriminator = MultisigAccountType, compact)]
#[pda(seeds = [SEED_SPENDING_LIMIT, multisig: Address, create_key: Address], bump = bump)]
pub struct SpendingLimit {
	pub bump: u8,
	pub multisig: Address,
	pub create_key: Address,
	/// Vault the allowance draws from.
	pub vault_index: u8,
	/// The default address means SOL; anything else is an SPL mint.
	pub mint: Address,
	/// Allowance per period, in native mint decimals.
	pub amount: u64,
	/// Remaining allowance in the current period.
	pub remaining_amount: u64,
	/// When the current period started.
	pub last_reset: i64,
	/// Reset period; see the `PERIOD_*` constants.
	pub period: u8,
	/// Members allowed to draw on this limit, as concatenated addresses.
	pub members: PodVec<u8, 512, 2>,
	/// Allowed destinations as concatenated addresses; empty means
	/// unrestricted.
	pub destinations: PodVec<u8, 256, 2>,
}

// ---------------------------------------------------------------------------
// Instruction arguments
// ---------------------------------------------------------------------------

#[instruction(discriminator = MultisigInstruction::ConfigInitialize)]
pub struct ConfigInitializeIx {
	pub bump: u8,
	pub treasury: Address,
	pub creation_fee: u64,
}

#[instruction(discriminator = MultisigInstruction::ConfigUpdate)]
pub struct ConfigUpdateIx {
	/// When set, `treasury` replaces the configured treasury.
	pub set_treasury: bool,
	pub treasury: Address,
	/// When set, `creation_fee` replaces the configured fee.
	pub set_creation_fee: bool,
	pub creation_fee: u64,
}

#[instruction(discriminator = MultisigInstruction::MultisigCreate)]
pub struct MultisigCreateIx {
	pub bump: u8,
	pub threshold: u16,
	pub timelock: u32,
	/// Lifetime for proposals created by this multisig; zero disables.
	pub ttl: u32,
	/// Permission masks aligned with the trailing member accounts.
	pub member_permissions: [u8; 16],
	/// The default address makes the multisig autonomous.
	pub config_authority: Address,
	/// The default address disables rent collection.
	pub rent_collector: Address,
}

#[instruction(discriminator = MultisigInstruction::MultisigImport)]
pub struct MultisigImportIx {
	pub bump: u8,
	/// Program expected to own the legacy account.
	pub legacy_program: Address,
	/// Anchor account discriminator the legacy data must start with.
	pub legacy_discriminator: [u8; 8],
	/// When set, `config_authority` overrides the legacy value.
	pub set_config_authority: bool,
	pub config_authority: Address,
	/// When set, `rent_collector` overrides the legacy value.
	pub set_rent_collector: bool,
	pub rent_collector: Address,
}

#[instruction(discriminator = MultisigInstruction::ProposalCreate)]
pub struct ProposalCreateIx {
	pub bump: u8,
	/// [`KIND_VAULT`] or [`KIND_CONFIG`].
	pub kind: u8,
	pub vault_index: u8,
	/// Ephemeral signing PDAs the vault message requires.
	pub ephemeral_signers: u8,
	/// Active length of `message`.
	pub message_len: u16,
	pub message: [u8; 640],
	/// Active length of `actions`.
	pub actions_len: u16,
	pub actions: [u8; 128],
}

#[instruction(discriminator = MultisigInstruction::ProposalActivate)]
pub struct ProposalActivateIx {}

#[instruction(discriminator = MultisigInstruction::ProposalApprove)]
pub struct ProposalApproveIx {}

#[instruction(discriminator = MultisigInstruction::ProposalReject)]
pub struct ProposalRejectIx {}

#[instruction(discriminator = MultisigInstruction::ProposalRevoke)]
pub struct ProposalRevokeIx {}

#[instruction(discriminator = MultisigInstruction::ProposalCancel)]
pub struct ProposalCancelIx {}

#[instruction(discriminator = MultisigInstruction::VaultExecute)]
pub struct VaultExecuteIx {}

#[instruction(discriminator = MultisigInstruction::ConfigExecute)]
pub struct ConfigExecuteIx {}

#[instruction(discriminator = MultisigInstruction::ConfigAuthorityExecute)]
pub struct ConfigAuthorityExecuteIx {
	pub actions_len: u16,
	pub actions: [u8; 128],
}

#[instruction(discriminator = MultisigInstruction::SpendingLimitUse)]
pub struct SpendingLimitUseIx {
	pub amount: u64,
	pub decimals: u8,
}

#[instruction(discriminator = MultisigInstruction::ProposalClose)]
pub struct ProposalCloseIx {}

#[event(discriminator = MultisigEventType::ProposalStatus)]
pub struct ProposalStatusEvent {
	pub multisig: Address,
	pub index: u64,
	pub status: u8,
	pub timestamp: i64,
}

// ---------------------------------------------------------------------------
// Membership helpers
// ---------------------------------------------------------------------------

/// Validate a member roster: sorted strictly ascending (hence unique),
/// permissions limited to the defined bits, and at least one member holding
/// each core permission so the multisig cannot strand itself.
pub fn validate_members(
	member_keys: &[Address],
	member_permissions: &[u8],
	threshold: u16,
) -> Result<(), ProgramError> {
	if member_keys.len() != member_permissions.len() || member_keys.is_empty() {
		return Err(MultisigError::InvalidConfiguration.into());
	}
	if member_keys.len() > MAX_MEMBERS {
		return Err(MultisigError::TooManyMembers.into());
	}

	let mut initiators = 0;
	let mut voters = 0;
	let mut executors = 0;
	for (position, key) in member_keys.iter().enumerate() {
		if position > 0 && key <= &member_keys[position - 1] {
			return Err(MultisigError::DuplicateMember.into());
		}
		let permissions = member_permissions[position];
		if permissions & !PERMISSIONS_ALL != 0 {
			return Err(MultisigError::UnknownPermission.into());
		}
		initiators += usize::from(permissions & PERMISSION_INITIATE != 0);
		voters += usize::from(permissions & PERMISSION_VOTE != 0);
		executors += usize::from(permissions & PERMISSION_EXECUTE != 0);
	}

	if initiators == 0 || voters == 0 || executors == 0 {
		return Err(MultisigError::InvalidConfiguration.into());
	}
	if threshold == 0 || usize::from(threshold) > voters {
		return Err(MultisigError::InvalidThreshold.into());
	}

	Ok(())
}

/// Binary-search the sorted member list for `key`.
fn member_index(member_keys: &[Address], key: &Address) -> Option<usize> {
	member_keys.binary_search(key).ok()
}

/// Number of members holding the vote permission.
fn num_voters(member_permissions: &[u8]) -> usize {
	member_permissions
		.iter()
		.filter(|permissions| *permissions & PERMISSION_VOTE != 0)
		.count()
}

/// Rejections needed to settle a proposal as rejected: once this many voters
/// reject, the remaining voters cannot reach the threshold. For seven voters
/// and a threshold of three, five rejections settle it.
fn rejection_cutoff(member_permissions: &[u8], threshold: u16) -> Result<usize, ProgramError> {
	num_voters(member_permissions)
		.checked_sub(usize::from(threshold))
		.and_then(|remaining| remaining.checked_add(1))
		.ok_or(ProgramError::ArithmeticOverflow)
}

fn member_bit(index: usize) -> Result<u32, ProgramError> {
	if index >= 32 {
		return Err(MultisigError::InvalidConfiguration.into());
	}
	Ok(1_u32 << index)
}

fn mask_count(mask: u32) -> usize {
	mask.count_ones() as usize
}

/// Seconds in one reset period; one-time limits never reset.
fn period_seconds(period: u8) -> Option<i64> {
	match period {
		PERIOD_DAY => Some(DAY_SECONDS),
		PERIOD_WEEK => Some(WEEK_SECONDS),
		PERIOD_MONTH => Some(MONTH_SECONDS),
		_ => None,
	}
}

/// Advance a spending limit's period anchor. Returns the new anchor and
/// whether the allowance reset. The anchor moves by whole elapsed periods, so
/// slow spenders do not drift, and a partial period never resets.
fn roll_period(now: i64, last_reset: i64, period: u8) -> Result<(i64, bool), ProgramError> {
	let Some(period_length) = period_seconds(period) else {
		return Ok((last_reset, false));
	};
	let passed = now
		.checked_sub(last_reset)
		.ok_or(ProgramError::ArithmeticOverflow)?;
	if passed <= period_length {
		return Ok((last_reset, false));
	}
	let periods_passed = passed / period_length;
	let advanced = periods_passed
		.checked_mul(period_length)
		.and_then(|seconds| last_reset.checked_add(seconds))
		.ok_or(ProgramError::ArithmeticOverflow)?;
	Ok((advanced, true))
}

/// Flatten a roster of addresses into its wire form.
fn flatten_roster<const N: usize>(keys: &[Address]) -> [u8; N] {
	let mut bytes = [0_u8; N];
	for (position, key) in keys.iter().enumerate() {
		bytes[position * 32..position * 32 + 32].copy_from_slice(key.as_ref());
	}
	bytes
}

/// Decode a roster's active length; the tail must hold whole addresses.
fn roster_count(bytes: &[u8]) -> Result<usize, ProgramError> {
	if !bytes.len().is_multiple_of(32) {
		return Err(MultisigError::InvalidConfiguration.into());
	}
	Ok(bytes.len() / 32)
}

/// Copy a wire roster into owned addresses.
fn decode_roster(bytes: &[u8], output: &mut [Address]) -> Result<usize, ProgramError> {
	let count = roster_count(bytes)?;
	for (position, slot) in output.iter_mut().take(count).enumerate() {
		*slot = Address::try_from(&bytes[position * 32..position * 32 + 32])
			.map_err(|_| MultisigError::InvalidConfiguration)?;
	}
	Ok(count)
}

fn read_timestamp(clock: &AccountView) -> Result<i64, ProgramError> {
	Ok(sysvars::clock::Clock::from_account_view(clock)?.unix_timestamp)
}

/// Whether a proposal's recorded lifetime has elapsed. Zero never expires.
fn is_expired(expires_at: i64, now: i64) -> bool {
	expires_at != 0 && now > expires_at
}

/// Whether the multisig is controlled by an external config authority.
fn is_controlled(config_authority: &Address) -> bool {
	*config_authority != Address::default()
}

fn emit_proposal_status(
	multisig: &Address,
	index: u64,
	status: u8,
	timestamp: i64,
) -> ProgramResult {
	ProposalStatusEvent::emit(|event| {
		event.multisig = *multisig;
		event.index.set(index);
		event.status = status;
		event.timestamp.set(timestamp);
		Ok(())
	})
}

// ---------------------------------------------------------------------------
// Zero-copy state snapshots
// ---------------------------------------------------------------------------

/// An owned copy of a [`Multisig`] header plus its member roster, used to
/// validate and compute before writing anything back.
#[derive(Debug, Clone, Copy)]
pub struct MultisigSnapshot {
	pub create_key: Address,
	pub config_authority: Address,
	pub rent_collector: Address,
	pub threshold: u16,
	pub timelock: u32,
	pub ttl: u32,
	pub transaction_index: u64,
	pub stale_transaction_index: u64,
	pub bump: u8,
	pub member_keys: [Address; 16],
	pub member_permissions: [u8; 16],
	pub member_count: usize,
}

impl MultisigSnapshot {
	/// Load a multisig, asserting it is the stored-bump PDA for `create_key`.
	pub fn load(account: &AccountView, create_key: &Address) -> Result<Self, ProgramError> {
		let mut snapshot = MultisigSnapshot {
			create_key: *create_key,
			config_authority: Address::default(),
			rent_collector: Address::default(),
			threshold: 0,
			timelock: 0,
			ttl: 0,
			transaction_index: 0,
			stale_transaction_index: 0,
			bump: 0,
			member_keys: [Address::default(); MAX_MEMBERS],
			member_permissions: [0; MAX_MEMBERS],
			member_count: 0,
		};

		account.with_compact_account::<Multisig, _>(&ID, |state| {
			snapshot.config_authority = state.config_authority;
			snapshot.rent_collector = state.rent_collector;
			snapshot.threshold = state.threshold.get();
			snapshot.timelock = state.timelock.get();
			snapshot.ttl = state.ttl.get();
			snapshot.transaction_index = state.transaction_index.get();
			snapshot.stale_transaction_index = state.stale_transaction_index.get();
			snapshot.bump = state.bump;
			snapshot.member_count =
				decode_roster(state.member_roster(), &mut snapshot.member_keys)?;
			snapshot.member_permissions[..snapshot.member_count]
				.copy_from_slice(state.member_permissions());
			Ok(())
		})?;

		Multisig::assert_seeds(account, create_key, &ID)?;
		Ok(snapshot)
	}

	pub fn member_keys(&self) -> &[Address] {
		&self.member_keys[..self.member_count]
	}

	pub fn member_permissions(&self) -> &[u8] {
		&self.member_permissions[..self.member_count]
	}

	/// Index of `key` in the member roster, if present.
	pub fn member_index_of(&self, key: &Address) -> Option<usize> {
		member_index(self.member_keys(), key)
	}

	/// The member's permission mask, or zero for non-members.
	pub fn permissions_of(&self, key: &Address) -> u8 {
		self.member_index_of(key)
			.map_or(0, |index| self.member_permissions()[index])
	}

	pub fn is_member(&self, key: &Address) -> bool {
		self.member_index_of(key).is_some()
	}

	pub fn cutoff(&self) -> Result<usize, ProgramError> {
		rejection_cutoff(self.member_permissions(), self.threshold)
	}
}

/// An owned copy of a [`Proposal`] header. Payload tails stay in the account
/// and are re-parsed zero-copy when an instruction needs them.
#[derive(Debug, Clone, Copy)]
pub struct ProposalSnapshot {
	pub multisig: Address,
	pub creator: Address,
	pub index: u64,
	pub kind: u8,
	pub vault_index: u8,
	pub vault_bump: u8,
	pub status: u8,
	pub status_at: i64,
	pub expires_at: i64,
	pub approved_mask: u32,
	pub rejected_mask: u32,
	pub bump: u8,
}

impl ProposalSnapshot {
	/// Load a proposal, asserting it is the stored-bump PDA owned by the
	/// multisig at `multisig` for proposal `index`.
	pub fn load(
		account: &AccountView,
		multisig: &Address,
		index: u64,
	) -> Result<Self, ProgramError> {
		let mut snapshot = ProposalSnapshot {
			multisig: *multisig,
			creator: Address::default(),
			index,
			kind: KIND_VAULT,
			vault_index: 0,
			vault_bump: 0,
			status: STATUS_DRAFT,
			status_at: 0,
			expires_at: 0,
			approved_mask: 0,
			rejected_mask: 0,
			bump: 0,
		};

		account.with_compact_account::<Proposal, _>(&ID, |state| {
			if state.multisig != *multisig || state.index.get() != index {
				return Err(ProgramError::InvalidSeeds);
			}
			snapshot.creator = state.creator;
			snapshot.kind = state.kind;
			snapshot.vault_index = state.vault_index;
			snapshot.vault_bump = state.vault_bump;
			snapshot.status = state.status;
			snapshot.status_at = state.status_at.get();
			snapshot.expires_at = state.expires_at.get();
			snapshot.approved_mask = state.approved_mask.get();
			snapshot.rejected_mask = state.rejected_mask.get();
			snapshot.bump = state.bump;
			Ok(())
		})?;

		Proposal::assert_seeds(account, multisig, index, &ID)?;
		Ok(snapshot)
	}
}

/// Read a multisig's `create_key` straight from its header. Proposal loaders
/// need it to seed the multisig PDA check, and the caller only knows the
/// multisig address at that point.
fn stored_create_key(account: &AccountView) -> Result<Address, ProgramError> {
	let mut create_key = Address::default();
	account.with_compact_account::<Multisig, _>(&ID, |state| {
		create_key = state.create_key;
		Ok(())
	})?;
	Ok(create_key)
}

/// Read a proposal's `index` straight from its header so the caller can seed
/// the proposal PDA check.
fn stored_proposal_index(account: &AccountView) -> Result<u64, ProgramError> {
	let mut index = 0;
	account.with_compact_account::<Proposal, _>(&ID, |state| {
		index = state.index.get();
		Ok(())
	})?;
	Ok(index)
}

// ---------------------------------------------------------------------------
// Vault message format
// ---------------------------------------------------------------------------

/// Zero-copy view over a stored vault message.
///
/// Wire format (all integers little-endian):
///
/// ```text
/// message     := num_signers u8, num_writable_signers u8,
///                num_writable_non_signers u8,
///                account_keys_len u16, account_keys * Address,
///                instructions_len u8, instruction*
/// instruction  := program_id_index u8, accounts_len u8, account_index * u8,
///                data_len u16, data * u8
/// ```
///
/// Account ordering matches Solana's message layout: writable signers, then
/// readonly signers, then writable non-signers, then readonly non-signers.
/// The layout follows the classic compiled-message idea with tighter
/// `u16`/`u8` length prefixes and no address table lookups.
#[derive(Debug, Clone, Copy)]
pub struct MessageView<'a> {
	bytes: &'a [u8],
	num_signers: usize,
	num_writable_signers: usize,
	num_writable_non_signers: usize,
	num_accounts: usize,
	accounts_offset: usize,
	num_instructions: usize,
	instructions_offset: usize,
}

/// One decoded inner instruction of a message.
#[derive(Debug, Clone, Copy)]
pub struct MessageInstruction<'a> {
	pub program_id_index: usize,
	pub account_indexes: &'a [u8],
	pub data: &'a [u8],
}

impl<'a> MessageView<'a> {
	/// Parse and fully validate a message: counters must be consistent,
	/// indexes must stay in range, and every length prefix must be backed by
	/// exactly the bytes present.
	pub fn parse(bytes: &[u8]) -> Result<MessageView<'_>, ProgramError> {
		let invalid = || ProgramError::Custom(MultisigError::InvalidMessage as u32);

		if bytes.len() < 5 {
			return Err(invalid());
		}
		let num_signers = usize::from(bytes[0]);
		let num_writable_signers = usize::from(bytes[1]);
		let num_writable_non_signers = usize::from(bytes[2]);
		if num_writable_signers > num_signers {
			return Err(invalid());
		}

		let num_accounts = usize::from(read_u16(bytes, 3).ok_or_else(invalid)?);
		if num_accounts > MAX_MESSAGE_ACCOUNTS {
			return Err(invalid());
		}
		if num_signers > num_accounts || num_signers + num_writable_non_signers > num_accounts {
			return Err(invalid());
		}
		let accounts_offset: usize = 5;
		let instructions_offset = accounts_offset
			.checked_add(num_accounts.checked_mul(32).ok_or_else(invalid)?)
			.ok_or_else(invalid)?;

		let mut cursor = instructions_offset;
		let num_instructions = usize::from(*bytes.get(cursor).ok_or_else(invalid)?);
		cursor += 1;
		if num_instructions > MAX_MESSAGE_INSTRUCTIONS {
			return Err(invalid());
		}

		for _ in 0..num_instructions {
			let program_id_index = usize::from(*bytes.get(cursor).ok_or_else(invalid)?);
			cursor += 1;
			if program_id_index >= num_accounts {
				return Err(invalid());
			}

			let accounts_len = usize::from(*bytes.get(cursor).ok_or_else(invalid)?);
			cursor += 1;
			if accounts_len > MAX_MESSAGE_ACCOUNTS {
				return Err(invalid());
			}
			let indexes_end = cursor.checked_add(accounts_len).ok_or_else(invalid)?;
			for index in bytes.get(cursor..indexes_end).ok_or_else(invalid)? {
				if usize::from(*index) >= num_accounts {
					return Err(invalid());
				}
			}
			cursor = indexes_end;

			let data_len = usize::from(read_u16(bytes, cursor).ok_or_else(invalid)?);
			cursor = cursor
				.checked_add(2)
				.and_then(|cursor| cursor.checked_add(data_len))
				.ok_or_else(invalid)?;
			if cursor > bytes.len() {
				return Err(invalid());
			}
		}

		if cursor != bytes.len() {
			return Err(invalid());
		}

		Ok(MessageView {
			bytes,
			num_signers,
			num_writable_signers,
			num_writable_non_signers,
			num_accounts,
			accounts_offset,
			num_instructions,
			instructions_offset,
		})
	}

	/// Number of unique account keys the message references.
	pub fn num_accounts(&self) -> usize {
		self.num_accounts
	}

	/// Number of inner instructions.
	pub fn num_instructions(&self) -> usize {
		self.num_instructions
	}

	/// The account key at `index`.
	pub fn account_key(&self, index: usize) -> Result<Address, ProgramError> {
		let start = self
			.accounts_offset
			.checked_add(
				index
					.checked_mul(32)
					.ok_or(ProgramError::ArithmeticOverflow)?,
			)
			.ok_or(ProgramError::ArithmeticOverflow)?;
		let end = start
			.checked_add(32)
			.ok_or(ProgramError::ArithmeticOverflow)?;
		let bytes = self
			.bytes
			.get(start..end)
			.ok_or(ProgramError::Custom(MultisigError::InvalidMessage as u32))?;
		Address::try_from(bytes)
			.map_err(|_| ProgramError::Custom(MultisigError::InvalidMessage as u32))
	}

	/// Whether the message marks `index` as a signer.
	pub fn is_signer_index(&self, index: usize) -> bool {
		index < self.num_signers
	}

	/// Whether the message marks `index` as writable.
	pub fn is_writable_index(&self, index: usize) -> bool {
		if index < self.num_writable_signers {
			return true;
		}
		index >= self.num_signers && index < self.num_signers + self.num_writable_non_signers
	}

	/// The inner instruction at `position`.
	pub fn instruction(&self, position: usize) -> Result<MessageInstruction<'a>, ProgramError> {
		let invalid = || ProgramError::Custom(MultisigError::InvalidMessage as u32);

		let mut cursor = self.instructions_offset + 1;
		for scanned in 0..self.num_instructions {
			let program_id_index = usize::from(self.bytes[cursor]);
			cursor += 1;
			let accounts_len = usize::from(self.bytes[cursor]);
			cursor += 1;
			let indexes_end = cursor + accounts_len;
			let account_indexes = &self.bytes[cursor..indexes_end];
			cursor = indexes_end;
			let data_len = usize::from(read_u16(self.bytes, cursor).ok_or_else(invalid)?);
			cursor += 2;
			let data = self
				.bytes
				.get(cursor..cursor + data_len)
				.ok_or_else(invalid)?;
			cursor += data_len;

			if scanned == position {
				return Ok(MessageInstruction {
					program_id_index,
					account_indexes,
					data,
				});
			}
		}

		Err(invalid())
	}
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
	let slice = bytes.get(offset..offset.checked_add(2)?)?;
	Some(u16::from_le_bytes([slice[0], slice[1]]))
}

/// Encode a vault message into `output` and return its length. Semantic
/// inputs keep tests and clients from hand-assembling bytes: the account list
/// must already be in signer-first/writable-first order, and
/// `writable_non_signers` counts the writable run that starts at
/// `num_signers`.
pub fn encode_message(
	num_signers: usize,
	num_writable_signers: usize,
	writable_non_signers: usize,
	account_keys: &[Address],
	instructions: &[(usize, &[u8], &[u8])],
	output: &mut [u8],
) -> Result<usize, ProgramError> {
	let invalid = || ProgramError::Custom(MultisigError::InvalidMessage as u32);
	if account_keys.len() > MAX_MESSAGE_ACCOUNTS
		|| instructions.len() > MAX_MESSAGE_INSTRUCTIONS
		|| num_writable_signers > num_signers
		|| num_signers + writable_non_signers > account_keys.len()
	{
		return Err(invalid());
	}

	output[0] = num_signers as u8;
	output[1] = num_writable_signers as u8;
	output[2] = writable_non_signers as u8;
	output[3..5].copy_from_slice(&(account_keys.len() as u16).to_le_bytes());
	let mut cursor = 5;
	for key in account_keys {
		output
			.get_mut(cursor..cursor + 32)
			.ok_or_else(invalid)?
			.copy_from_slice(key.as_ref());
		cursor += 32;
	}
	output[cursor] = instructions.len() as u8;
	cursor += 1;
	for (program_id_index, account_indexes, data) in instructions {
		output[cursor] = *program_id_index as u8;
		cursor += 1;
		output[cursor] = account_indexes.len() as u8;
		cursor += 1;
		output
			.get_mut(cursor..cursor + account_indexes.len())
			.ok_or_else(invalid)?
			.copy_from_slice(account_indexes);
		cursor += account_indexes.len();
		output[cursor..cursor + 2].copy_from_slice(&(data.len() as u16).to_le_bytes());
		cursor += 2;
		output
			.get_mut(cursor..cursor + data.len())
			.ok_or_else(invalid)?
			.copy_from_slice(data);
		cursor += data.len();
	}

	MessageView::parse(&output[..cursor])?;
	Ok(cursor)
}

// ---------------------------------------------------------------------------
// Config action stream
// ---------------------------------------------------------------------------

/// Action discriminators inside the encoded action stream.
pub const ACTION_ADD_MEMBER: u8 = 0;
pub const ACTION_REMOVE_MEMBER: u8 = 1;
pub const ACTION_CHANGE_THRESHOLD: u8 = 2;
pub const ACTION_SET_TIME_LOCK: u8 = 3;
pub const ACTION_SET_RENT_COLLECTOR: u8 = 4;
pub const ACTION_SET_CONFIG_AUTHORITY: u8 = 5;
pub const ACTION_ADD_SPENDING_LIMIT: u8 = 6;
pub const ACTION_REMOVE_SPENDING_LIMIT: u8 = 7;
pub const ACTION_SET_PROPOSAL_TTL: u8 = 8;

/// Zero-copy view over one action of the encoded stream.
///
/// `AddSpendingLimit` carries its member and destination rosters in fixed
/// buffers so the view stays allocation-free; that width is intentional.
#[derive(Debug, Clone, Copy)]
#[expect(
	variant_size_differences,
	reason = "the spending-limit variant carries its fixed rosters by design"
)]
pub enum ConfigActionView {
	AddMember {
		key: Address,
		permissions: u8,
	},
	RemoveMember {
		key: Address,
	},
	ChangeThreshold {
		threshold: u16,
	},
	SetTimeLock {
		seconds: u32,
	},
	SetProposalTtl {
		seconds: u32,
	},
	SetRentCollector {
		collector: Option<Address>,
	},
	SetConfigAuthority {
		authority: Option<Address>,
	},
	AddSpendingLimit {
		create_key: Address,
		vault_index: u8,
		mint: Address,
		amount: u64,
		period: u8,
		members: [Address; MAX_SPENDING_LIMIT_MEMBERS],
		members_len: usize,
		destinations: [Address; MAX_SPENDING_LIMIT_DESTINATIONS],
		destinations_len: usize,
	},
	RemoveSpendingLimit {
		key: Address,
	},
}

/// Walk every action in an encoded stream, validating structure as it goes.
/// The stream grammar is a fixed-width encoding: `u8 count`, then
/// per-variant tagged payloads.
pub fn for_each_action(
	bytes: &[u8],
	mut visit: impl FnMut(ConfigActionView) -> Result<(), ProgramError>,
) -> Result<(), ProgramError> {
	let invalid = || ProgramError::Custom(MultisigError::InvalidActions as u32);

	let mut cursor = 0;
	let count = usize::from(*bytes.first().ok_or_else(invalid)?);
	cursor += 1;

	for _ in 0..count {
		let variant = *bytes.get(cursor).ok_or_else(invalid)?;
		cursor += 1;
		let action = match variant {
			ACTION_ADD_MEMBER => {
				let key = read_address(bytes, cursor)?;
				cursor += 32;
				let permissions = *bytes.get(cursor).ok_or_else(invalid)?;
				cursor += 1;
				ConfigActionView::AddMember { key, permissions }
			}
			ACTION_REMOVE_MEMBER => {
				let key = read_address(bytes, cursor)?;
				cursor += 32;
				ConfigActionView::RemoveMember { key }
			}
			ACTION_CHANGE_THRESHOLD => {
				let threshold = read_u16(bytes, cursor).ok_or_else(invalid)?;
				cursor += 2;
				ConfigActionView::ChangeThreshold { threshold }
			}
			ACTION_SET_TIME_LOCK => {
				let seconds = read_action_u32(bytes, &mut cursor)?;
				ConfigActionView::SetTimeLock { seconds }
			}
			ACTION_SET_PROPOSAL_TTL => {
				let seconds = read_action_u32(bytes, &mut cursor)?;
				ConfigActionView::SetProposalTtl { seconds }
			}
			ACTION_SET_RENT_COLLECTOR => {
				let collector = read_option_address(bytes, cursor)?;
				cursor += 33;
				ConfigActionView::SetRentCollector { collector }
			}
			ACTION_SET_CONFIG_AUTHORITY => {
				let authority = read_option_address(bytes, cursor)?;
				cursor += 33;
				ConfigActionView::SetConfigAuthority { authority }
			}
			ACTION_ADD_SPENDING_LIMIT => {
				let create_key = read_address(bytes, cursor)?;
				cursor += 32;
				let vault_index = *bytes.get(cursor).ok_or_else(invalid)?;
				cursor += 1;
				let mint = read_address(bytes, cursor)?;
				cursor += 32;
				let end = cursor.checked_add(8).ok_or_else(invalid)?;
				let slice = bytes.get(cursor..end).ok_or_else(invalid)?;
				let amount = u64::from_le_bytes(slice.try_into().map_err(|_| invalid())?);
				cursor = end;
				let period = *bytes.get(cursor).ok_or_else(invalid)?;
				cursor += 1;
				let members_len = usize::from(*bytes.get(cursor).ok_or_else(invalid)?);
				cursor += 1;
				if members_len > MAX_SPENDING_LIMIT_MEMBERS || members_len == 0 {
					return Err(invalid());
				}
				let mut members = [Address::default(); MAX_SPENDING_LIMIT_MEMBERS];
				read_addresses_into(bytes, cursor, members_len, &mut members)?;
				cursor = cursor
					.checked_add(members_len.checked_mul(32).ok_or_else(invalid)?)
					.ok_or_else(invalid)?;
				let destinations_len = usize::from(*bytes.get(cursor).ok_or_else(invalid)?);
				cursor += 1;
				if destinations_len > MAX_SPENDING_LIMIT_DESTINATIONS {
					return Err(invalid());
				}
				let mut destinations = [Address::default(); MAX_SPENDING_LIMIT_DESTINATIONS];
				read_addresses_into(bytes, cursor, destinations_len, &mut destinations)?;
				cursor = cursor
					.checked_add(destinations_len.checked_mul(32).ok_or_else(invalid)?)
					.ok_or_else(invalid)?;
				ConfigActionView::AddSpendingLimit {
					create_key,
					vault_index,
					mint,
					amount,
					period,
					members,
					members_len,
					destinations,
					destinations_len,
				}
			}
			ACTION_REMOVE_SPENDING_LIMIT => {
				let key = read_address(bytes, cursor)?;
				cursor += 32;
				ConfigActionView::RemoveSpendingLimit { key }
			}
			_ => return Err(invalid()),
		};

		visit(action)?;
	}

	if cursor != bytes.len() {
		return Err(invalid());
	}

	Ok(())
}

/// Structural validation at proposal creation: every action parses, and the
/// state-independent semantic guards hold.
pub fn validate_actions(bytes: &[u8]) -> Result<(), ProgramError> {
	if bytes.is_empty() {
		return Err(MultisigError::InvalidActions.into());
	}
	for_each_action(bytes, |action| {
		match action {
			ConfigActionView::SetTimeLock { seconds } => {
				if seconds > MAX_TIME_LOCK {
					return Err(MultisigError::TimeLockExceedsMaxAllowed.into());
				}
			}
			ConfigActionView::SetProposalTtl { seconds } => {
				if seconds > MAX_PROPOSAL_TTL {
					return Err(MultisigError::ProposalExpired.into());
				}
			}
			ConfigActionView::AddSpendingLimit { period, .. }
				if period != PERIOD_ONE_TIME && period_seconds(period).is_none() =>
			{
				return Err(MultisigError::InvalidPeriod.into());
			}
			_ => {}
		}
		Ok(())
	})
}

fn read_action_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, ProgramError> {
	let invalid = || ProgramError::Custom(MultisigError::InvalidActions as u32);
	let end = cursor.checked_add(4).ok_or_else(invalid)?;
	let slice = bytes.get(*cursor..end).ok_or_else(invalid)?;
	*cursor = end;
	Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_address(bytes: &[u8], offset: usize) -> Result<Address, ProgramError> {
	let end = offset
		.checked_add(32)
		.ok_or(ProgramError::ArithmeticOverflow)?;
	let slice = bytes
		.get(offset..end)
		.ok_or(ProgramError::Custom(MultisigError::InvalidActions as u32))?;
	Address::try_from(slice).map_err(|_| ProgramError::Custom(MultisigError::InvalidActions as u32))
}

/// Copy `count` addresses starting at `start` into a fixed buffer.
fn read_addresses_into(
	bytes: &[u8],
	start: usize,
	count: usize,
	output: &mut [Address],
) -> Result<(), ProgramError> {
	for (position, slot) in output.iter_mut().take(count).enumerate() {
		*slot = read_address(bytes, start + position * 32)?;
	}
	Ok(())
}

fn read_option_address(bytes: &[u8], offset: usize) -> Result<Option<Address>, ProgramError> {
	let tag = *bytes
		.get(offset)
		.ok_or(ProgramError::Custom(MultisigError::InvalidActions as u32))?;
	match tag {
		0 => Ok(None),
		1 => Ok(Some(read_address(bytes, offset + 1)?)),
		_ => Err(ProgramError::Custom(MultisigError::InvalidActions as u32)),
	}
}

// ---------------------------------------------------------------------------
// Legacy multisig import
// ---------------------------------------------------------------------------

/// Zero-copy view over a legacy on-chain multisig account written in the
/// classic Anchor layout.
///
/// Layout (Anchor/Borsh, little-endian) shared by the deployed multisig
/// programs that store their roster inline:
/// 8..40    `create_key`
/// 40..72   `config_authority`
/// 72..74   threshold u16
/// 74..78   `time_lock` u32
/// 78..86   `transaction_index` u64
/// 86..94   `stale_transaction_index` u64
/// 94..95   `rent_collector` Option tag
/// 95..127  `rent_collector` key (present even when None)
/// 127..128 bump u8
/// 128..132 members length u32
/// 132..    members: 33 bytes each (key + permission mask)
/// ```
#[derive(Debug, Clone, Copy)]
pub struct LegacyMultisig {
	pub create_key: Address,
	pub config_authority: Address,
	pub threshold: u16,
	pub time_lock: u32,
	pub rent_collector: Option<Address>,
	pub member_keys: [Address; MAX_MEMBERS],
	pub member_permissions: [u8; MAX_MEMBERS],
	pub member_count: usize,
}

impl LegacyMultisig {
	/// Number of members carried over from the legacy account.
	pub fn members(&self) -> &[Address] {
		&self.member_keys[..self.member_count]
	}

	/// Parse and validate a legacy multisig account owned by
	/// `legacy_program` and discriminated by `discriminator`. The roster must
	/// satisfy the same invariants this program enforces at creation, so an
	/// import can never smuggle in a broken consensus.
	pub fn parse(bytes: &[u8], discriminator: &[u8; 8]) -> Result<LegacyMultisig, ProgramError> {
		let invalid = || ProgramError::Custom(MultisigError::InvalidLegacyMultisig as u32);

		if bytes.len() < 132 || bytes[..8] != *discriminator {
			return Err(invalid());
		}

		let member_count =
			u32::from_le_bytes([bytes[128], bytes[129], bytes[130], bytes[131]]) as usize;
		if member_count == 0 || member_count > MAX_MEMBERS {
			return Err(invalid());
		}
		let members_end = 132_usize
			.checked_add(member_count.checked_mul(33).ok_or_else(invalid)?)
			.ok_or_else(invalid)?;
		if members_end > bytes.len() {
			return Err(invalid());
		}

		let mut member_keys = [Address::default(); MAX_MEMBERS];
		for (position, slot) in member_keys.iter_mut().take(member_count).enumerate() {
			*slot = read_legacy_address(bytes, 132 + position * 33)?;
		}
		let mut member_permissions = [0_u8; MAX_MEMBERS];
		for (position, slot) in member_permissions.iter_mut().enumerate().take(member_count) {
			*slot = *bytes.get(132 + position * 33 + 32).ok_or_else(invalid)?;
		}

		let rent_collector_tag = bytes[94];
		if rent_collector_tag > 1 {
			return Err(invalid());
		}
		let rent_collector = (rent_collector_tag == 1)
			.then(|| read_legacy_address(bytes, 95))
			.transpose()?;

		let multisig = LegacyMultisig {
			create_key: read_legacy_address(bytes, 8)?,
			config_authority: read_legacy_address(bytes, 40)?,
			threshold: u16::from_le_bytes([bytes[72], bytes[73]]),
			time_lock: u32::from_le_bytes([bytes[74], bytes[75], bytes[76], bytes[77]]),
			rent_collector,
			member_keys,
			member_permissions,
			member_count,
		};

		validate_members(
			multisig.members(),
			&multisig.member_permissions[..multisig.member_count],
			multisig.threshold,
		)?;
		if multisig.time_lock > MAX_TIME_LOCK {
			return Err(MultisigError::TimeLockExceedsMaxAllowed.into());
		}

		Ok(multisig)
	}
}

fn read_legacy_address(bytes: &[u8], offset: usize) -> Result<Address, ProgramError> {
	let end = offset
		.checked_add(32)
		.ok_or(ProgramError::ArithmeticOverflow)?;
	let slice = bytes.get(offset..end).ok_or(ProgramError::Custom(
		MultisigError::InvalidLegacyMultisig as u32,
	))?;
	Address::try_from(slice)
		.map_err(|_| ProgramError::Custom(MultisigError::InvalidLegacyMultisig as u32))
}

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct ConfigInitializeAccounts<'a> {
	pub authority: &'a mut AccountView,
	pub program_config: &'a mut AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts)]
pub struct ConfigUpdateAccounts<'a> {
	pub authority: &'a AccountView,
	pub program_config: &'a mut AccountView,
}

#[derive(Accounts)]
pub struct MultisigCreateAccounts<'a> {
	pub program_config: &'a AccountView,
	pub create_key: &'a AccountView,
	pub multisig: &'a mut AccountView,
	pub rent_payer: &'a mut AccountView,
	pub system_program: &'a AccountView,
	/// Treasury that collects the creation fee; pass the program's own
	/// address as a filler when the fee is zero.
	pub treasury: Option<&'a mut AccountView>,
	/// The founding members, sorted by address; `member_permissions` indexes
	/// into this list positionally.
	#[pina(remaining)]
	pub member_accounts: &'a [AccountView],
}

#[derive(Accounts)]
pub struct MultisigImportAccounts<'a> {
	pub legacy_multisig: &'a AccountView,
	pub program_config: &'a AccountView,
	pub create_key: &'a AccountView,
	pub multisig: &'a mut AccountView,
	pub rent_payer: &'a mut AccountView,
	pub system_program: &'a AccountView,
	/// Treasury that collects the creation fee; absent when the fee is zero.
	pub treasury: Option<&'a mut AccountView>,
}

#[derive(Accounts)]
pub struct ProposalCreateAccounts<'a> {
	pub multisig: &'a mut AccountView,
	pub proposal: &'a mut AccountView,
	pub creator: &'a AccountView,
	pub rent_payer: &'a mut AccountView,
	pub system_program: &'a AccountView,
	pub clock: &'a AccountView,
}

#[derive(Accounts)]
pub struct ProposalActivateAccounts<'a> {
	pub multisig: &'a AccountView,
	pub proposal: &'a mut AccountView,
	pub member: &'a AccountView,
	pub clock: &'a AccountView,
}

#[derive(Accounts)]
pub struct ProposalApproveAccounts<'a> {
	pub multisig: &'a AccountView,
	pub proposal: &'a mut AccountView,
	pub member: &'a AccountView,
	pub clock: &'a AccountView,
}

#[derive(Accounts)]
pub struct ProposalRejectAccounts<'a> {
	pub multisig: &'a AccountView,
	pub proposal: &'a mut AccountView,
	pub member: &'a AccountView,
	pub clock: &'a AccountView,
}

#[derive(Accounts)]
pub struct ProposalRevokeAccounts<'a> {
	pub multisig: &'a AccountView,
	pub proposal: &'a mut AccountView,
	pub member: &'a AccountView,
	pub clock: &'a AccountView,
}

#[derive(Accounts)]
pub struct ProposalCancelAccounts<'a> {
	pub multisig: &'a AccountView,
	pub proposal: &'a mut AccountView,
	pub member: &'a AccountView,
	pub clock: &'a AccountView,
}

#[derive(Accounts)]
pub struct VaultExecuteAccounts<'a> {
	pub multisig: &'a AccountView,
	pub proposal: &'a mut AccountView,
	pub member: &'a AccountView,
	pub clock: &'a AccountView,
	/// The message's account keys, in message order. Accounts the message
	/// marks writable must also be writable in this instruction.
	#[pina(remaining)]
	pub message_accounts: &'a [AccountView],
}

#[derive(Accounts)]
pub struct ConfigExecuteAccounts<'a> {
	pub multisig: &'a mut AccountView,
	pub proposal: &'a mut AccountView,
	pub member: &'a AccountView,
	pub rent_payer: &'a mut AccountView,
	pub system_program: &'a AccountView,
	pub clock: &'a AccountView,
	/// Spending limit accounts referenced by add/remove spending-limit
	/// actions, in any order.
	#[pina(remaining)]
	pub spending_limit_accounts: &'a mut [AccountView],
}

#[derive(Accounts)]
pub struct ConfigAuthorityExecuteAccounts<'a> {
	pub multisig: &'a mut AccountView,
	pub authority: &'a AccountView,
	pub rent_payer: &'a mut AccountView,
	pub system_program: &'a AccountView,
	pub clock: &'a AccountView,
	/// Spending limit accounts referenced by add/remove spending-limit
	/// actions, in any order.
	#[pina(remaining)]
	pub spending_limit_accounts: &'a mut [AccountView],
}

#[derive(Accounts)]
pub struct SpendingLimitUseAccounts<'a> {
	pub multisig: &'a AccountView,
	pub spending_limit: &'a mut AccountView,
	pub member: &'a AccountView,
	/// The vault PDA: SOL source or SPL transfer authority.
	pub vault: &'a mut AccountView,
	/// SOL destination, or the destination token account for SPL.
	pub destination: &'a mut AccountView,
	pub clock: &'a AccountView,
	/// SPL source token account; absent for SOL.
	pub vault_token_account: Option<&'a mut AccountView>,
	/// SPL mint; absent for SOL.
	pub mint: Option<&'a AccountView>,
	/// SPL token program; absent for SOL.
	pub token_program: Option<&'a AccountView>,
	/// System program; absent for SPL.
	pub system_program: Option<&'a AccountView>,
}

#[derive(Accounts)]
pub struct ProposalCloseAccounts<'a> {
	pub multisig: &'a AccountView,
	pub proposal: &'a mut AccountView,
	pub rent_collector: &'a mut AccountView,
}

// ---------------------------------------------------------------------------
// Config action application
// ---------------------------------------------------------------------------

/// Apply a validated action stream to a multisig working copy, executing
/// spending-limit account creation and closure CPIs along the way.
///
/// `changed` in the working state marks that a consensus parameter moved and
/// prior proposals must be invalidated.
fn apply_config_actions(
	multisig_key: &Address,
	working: &mut MultisigWorkingState,
	actions: &[u8],
	spending_limit_accounts: &mut [AccountView],
	rent_payer: &mut AccountView,
	system_program: &AccountView,
	now: i64,
) -> Result<(), ProgramError> {
	for_each_action(actions, |action| {
		match action {
			ConfigActionView::AddMember { key, permissions } => {
				if working.member_count >= MAX_MEMBERS {
					return Err(MultisigError::TooManyMembers.into());
				}
				if working.member_keys[..working.member_count].contains(&key) {
					return Err(MultisigError::DuplicateMember.into());
				}
				// Sorted insert keeps the mask indexes stable for proposals
				// created after this point; prior proposals become stale.
				let position = working.member_keys[..working.member_count]
					.binary_search(&key)
					.expect_err("duplicate members are rejected above");
				working
					.member_keys
					.copy_within(position..working.member_count, position + 1);
				working.member_keys[position] = key;
				working
					.member_permissions
					.copy_within(position..working.member_count, position + 1);
				working.member_permissions[position] = permissions;
				working.member_count += 1;
				working.changed = true;
			}
			ConfigActionView::RemoveMember { key } => {
				let position = member_index(&working.member_keys[..working.member_count], &key)
					.ok_or(MultisigError::NotAMember)?;
				working
					.member_keys
					.copy_within(position + 1..working.member_count, position);
				working
					.member_permissions
					.copy_within(position + 1..working.member_count, position);
				working.member_count -= 1;
				working.changed = true;
			}
			ConfigActionView::ChangeThreshold { threshold } => {
				working.threshold = threshold;
				working.changed = true;
			}
			ConfigActionView::SetTimeLock { seconds } => {
				working.timelock = seconds;
				working.changed = true;
			}
			// A TTL change does not invalidate prior proposals: each proposal
			// recorded its own expiry at creation.
			ConfigActionView::SetProposalTtl { seconds } => {
				working.ttl = seconds;
			}
			ConfigActionView::SetRentCollector { collector } => {
				working.rent_collector = collector.unwrap_or_default();
			}
			ConfigActionView::SetConfigAuthority { authority } => {
				working.config_authority = authority.unwrap_or_default();
				working.changed = true;
			}
			ConfigActionView::AddSpendingLimit {
				create_key,
				vault_index,
				mint,
				amount,
				period,
				members,
				members_len,
				destinations,
				destinations_len,
			} => {
				system_program.assert_address(&system::ID)?;

				let seeds = SpendingLimit::seeds(multisig_key, &create_key);
				let Some((limit_key, bump)) = try_find_program_address(&seeds.as_slices(), &ID)
				else {
					return Err(ProgramError::InvalidSeeds);
				};
				let account = find_mut_by_address(spending_limit_accounts, &limit_key)
					.ok_or(MultisigError::MissingAccount)?;

				let mut sorted_members = members;
				sorted_members[..members_len].sort_unstable();

				let space =
					SpendingLimit::projected_bytes(members_len * 32, destinations_len * 32)?;
				CreateCompactProgramAccountWithBump {
					account,
					payer: &mut *rent_payer,
					owner: &ID,
					seeds: &seeds.as_slices(),
					bump,
					patch: SpendingLimitPatch::new()
						.bump(bump)
						.multisig(*multisig_key)
						.create_key(create_key)
						.vault_index(vault_index)
						.mint(mint)
						.amount(amount)
						.remaining_amount(amount)
						.last_reset(now)
						.period(period)
						.replace_members(
							&flatten_roster::<512>(&sorted_members[..members_len])
								[..members_len * 32],
						)
						.replace_destinations(
							&flatten_roster::<256>(&destinations[..destinations_len])
								[..destinations_len * 32],
						),
					space,
				}
				.invoke::<SpendingLimit>()?;
			}
			ConfigActionView::RemoveSpendingLimit { key } => {
				let account = find_mut_by_address(spending_limit_accounts, &key)
					.ok_or(MultisigError::MissingAccount)?;

				let mut create_key = Address::default();
				account.with_compact_account::<SpendingLimit, _>(&ID, |limit| {
					if limit.multisig != *multisig_key {
						return Err(ProgramError::InvalidSeeds);
					}
					create_key = limit.create_key;
					Ok(())
				})?;
				SpendingLimit::assert_seeds(account, multisig_key, &create_key, &ID)?;

				account.close_account_zeroed(&ID, &mut *rent_payer)?;
			}
		}
		Ok(())
	})
}

/// Find a mutable account view by address.
fn find_mut_by_address<'a>(
	accounts: &'a mut [AccountView],
	address: &Address,
) -> Option<&'a mut AccountView> {
	accounts
		.iter_mut()
		.find(|account| account.address() == address)
}

/// A working copy of a multisig's mutable state, accumulated while applying
/// config actions and committed once at the end.
struct MultisigWorkingState {
	member_keys: [Address; MAX_MEMBERS],
	member_permissions: [u8; MAX_MEMBERS],
	member_count: usize,
	threshold: u16,
	timelock: u32,
	ttl: u32,
	rent_collector: Address,
	config_authority: Address,
	transaction_index: u64,
	changed: bool,
}

impl MultisigWorkingState {
	fn capture(snapshot: &MultisigSnapshot) -> Self {
		MultisigWorkingState {
			member_keys: snapshot.member_keys,
			member_permissions: snapshot.member_permissions,
			member_count: snapshot.member_count,
			threshold: snapshot.threshold,
			timelock: snapshot.timelock,
			ttl: snapshot.ttl,
			rent_collector: snapshot.rent_collector,
			config_authority: snapshot.config_authority,
			transaction_index: snapshot.transaction_index,
			changed: false,
		}
	}

	fn validate(&self) -> Result<(), ProgramError> {
		validate_members(
			&self.member_keys[..self.member_count],
			&self.member_permissions[..self.member_count],
			self.threshold,
		)?;
		if self.timelock > MAX_TIME_LOCK {
			return Err(MultisigError::TimeLockExceedsMaxAllowed.into());
		}
		if self.ttl > MAX_PROPOSAL_TTL {
			return Err(MultisigError::ProposalExpired.into());
		}
		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Instruction: ConfigInitialize
// ---------------------------------------------------------------------------

impl<'a> ProcessAccountInfos<'a> for ConfigInitializeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = ConfigInitializeIx::try_from_bytes(data)?;

		self.authority.assert_signer()?.assert_writable()?;
		self.system_program.assert_address(&system::ID)?;
		let seeds = ProgramConfig::seeds();

		CreateProgramAccountWithBump {
			account: self.program_config,
			payer: self.authority,
			owner: &ID,
			seeds: &seeds.as_slices(),
			bump: args.bump,
		}
		.invoke_with::<ProgramConfig>(|config| {
			config.bump = args.bump;
			config.authority = *self.authority.address();
			config.treasury = args.treasury;
			config.creation_fee.set(args.creation_fee.get());
			Ok(())
		})?;

		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Instruction: ConfigUpdate
// ---------------------------------------------------------------------------

impl<'a> ProcessAccountInfos<'a> for ConfigUpdateAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = ConfigUpdateIx::try_from_bytes(data)?;

		self.authority.assert_signer()?;
		ProgramConfig::assert_seeds(self.program_config, &ID)?;

		let mut config = self.program_config.as_account_mut::<ProgramConfig>(&ID)?;

		if config.authority != *self.authority.address() {
			return Err(MultisigError::InvalidConfigAuthority.into());
		}
		if args.set_treasury.get() {
			config.treasury = args.treasury;
		}
		if args.set_creation_fee.get() {
			config.creation_fee.set(args.creation_fee.get());
		}

		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Instructions: MultisigCreate / MultisigImport
// ---------------------------------------------------------------------------

/// Charge the configured creation fee into the treasury, if any.
fn pay_creation_fee(
	program_config: &AccountView,
	treasury: Option<&mut AccountView>,
	rent_payer: &mut AccountView,
) -> Result<(), ProgramError> {
	let config = program_config.as_account::<ProgramConfig>(&ID)?;
	let creation_fee = config.creation_fee.get();
	let treasury_key = config.treasury;
	drop(config);

	if creation_fee == 0 {
		return Ok(());
	}
	let treasury = treasury.ok_or(MultisigError::MissingTreasury)?;
	treasury.assert_address(&treasury_key)?.assert_writable()?;
	system::instructions::Transfer {
		from: rent_payer,
		to: treasury,
		lamports: creation_fee,
	}
	.invoke()
}

impl<'a> ProcessAccountInfos<'a> for MultisigCreateAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = MultisigCreateIx::try_from_bytes(data)?;
		let member_count = self.member_accounts.len();
		if member_count == 0 || member_count > MAX_MEMBERS {
			return Err(MultisigError::TooManyMembers.into());
		}
		let member_permissions = &args.member_permissions[..member_count];
		let threshold = args.threshold.get();
		let timelock = args.timelock.get();
		let ttl = args.ttl.get();

		// Trailing accounts carry the roster; collect their addresses for
		// validation and the compact tail in one stack copy.
		let mut member_keys = [Address::default(); MAX_MEMBERS];
		for (position, account) in self.member_accounts.iter().take(member_count).enumerate() {
			member_keys[position] = *account.address();
		}
		validate_members(&member_keys[..member_count], member_permissions, threshold)?;
		if timelock > MAX_TIME_LOCK {
			return Err(MultisigError::TimeLockExceedsMaxAllowed.into());
		}
		if ttl > MAX_PROPOSAL_TTL {
			return Err(MultisigError::ProposalExpired.into());
		}

		self.create_key.assert_signer()?;
		self.rent_payer.assert_signer()?.assert_writable()?;
		self.system_program.assert_address(&system::ID)?;
		pay_creation_fee(self.program_config, self.treasury, self.rent_payer)?;

		let create_key = *self.create_key.address();
		let space = Multisig::projected_bytes(member_count * 32, member_count)?;
		let seeds = Multisig::seeds(&create_key);

		CreateCompactProgramAccountWithBump {
			account: self.multisig,
			payer: self.rent_payer,
			owner: &ID,
			seeds: &seeds.as_slices(),
			bump: args.bump,
			patch: MultisigPatch::new()
				.bump(args.bump)
				.create_key(create_key)
				.config_authority(args.config_authority)
				.rent_collector(args.rent_collector)
				.threshold(threshold)
				.timelock(timelock)
				.ttl(ttl)
				.transaction_index(0)
				.stale_transaction_index(0)
				.replace_member_roster(
					&flatten_roster::<512>(&member_keys[..member_count])[..member_count * 32],
				)
				.replace_member_permissions(member_permissions),
			space,
		}
		.invoke::<Multisig>()?;

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for MultisigImportAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = MultisigImportIx::try_from_bytes(data)?;

		self.legacy_multisig.assert_owner(&args.legacy_program)?;
		let legacy_data = self.legacy_multisig.try_borrow()?;
		let legacy = LegacyMultisig::parse(&legacy_data, &args.legacy_discriminator)?;

		self.create_key.assert_signer()?;
		self.rent_payer.assert_signer()?.assert_writable()?;
		self.system_program.assert_address(&system::ID)?;
		pay_creation_fee(self.program_config, self.treasury, self.rent_payer)?;

		let create_key = *self.create_key.address();
		let member_count = legacy.member_count;
		let space = Multisig::projected_bytes(member_count * 32, member_count)?;
		let seeds = Multisig::seeds(&create_key);

		CreateCompactProgramAccountWithBump {
			account: self.multisig,
			payer: self.rent_payer,
			owner: &ID,
			seeds: &seeds.as_slices(),
			bump: args.bump,
			patch: MultisigPatch::new()
				.bump(args.bump)
				.create_key(create_key)
				.config_authority(if args.set_config_authority.get() {
					args.config_authority
				} else {
					legacy.config_authority
				})
				.rent_collector(if args.set_rent_collector.get() {
					args.rent_collector
				} else {
					legacy.rent_collector.unwrap_or_default()
				})
				.threshold(legacy.threshold)
				.timelock(legacy.time_lock)
				.ttl(0)
				.transaction_index(0)
				.stale_transaction_index(0)
				.replace_member_roster(
					&flatten_roster::<512>(legacy.members())[..legacy.member_count * 32],
				)
				.replace_member_permissions(&legacy.member_permissions[..member_count]),
			space,
		}
		.invoke::<Multisig>()?;

		log!("imported a legacy multisig with {member_count} members");

		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Instruction: ProposalCreate
// ---------------------------------------------------------------------------

impl<'a> ProcessAccountInfos<'a> for ProposalCreateAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = ProposalCreateIx::try_from_bytes(data)?;
		let kind = args.kind;
		if kind != KIND_VAULT && kind != KIND_CONFIG {
			return Err(MultisigError::InvalidProposalKind.into());
		}
		let message_len = usize::from(args.message_len.get());
		let actions_len = usize::from(args.actions_len.get());
		if message_len > MAX_MESSAGE_BYTES || actions_len > MAX_ACTIONS_BYTES {
			return Err(MultisigError::InvalidMessage.into());
		}
		let ephemeral_count = usize::from(args.ephemeral_signers);
		if ephemeral_count > MAX_EPHEMERAL_SIGNERS || (kind == KIND_CONFIG && ephemeral_count > 0) {
			return Err(MultisigError::InvalidMessage.into());
		}

		let message = &args.message[..message_len];
		let actions = &args.actions[..actions_len];
		if kind == KIND_VAULT {
			if message_len == 0 {
				return Err(MultisigError::InvalidMessage.into());
			}
			MessageView::parse(message)?;
		} else if actions_len == 0 {
			return Err(MultisigError::InvalidActions.into());
		} else {
			validate_actions(actions)?;
		}

		self.creator.assert_signer()?;
		self.rent_payer.assert_signer()?.assert_writable()?;
		self.system_program.assert_address(&system::ID)?;

		let create_key = stored_create_key(self.multisig)?;
		let snapshot = MultisigSnapshot::load(self.multisig, &create_key)?;
		let creator_index = snapshot
			.member_index_of(self.creator.address())
			.ok_or(MultisigError::NotAMember)?;
		if snapshot.member_permissions()[creator_index] & PERMISSION_INITIATE == 0 {
			return Err(MultisigError::Unauthorized.into());
		}
		let multisig_ttl = snapshot.ttl;

		let multisig_key = *self.multisig.address();
		let transaction_index = snapshot
			.transaction_index
			.checked_add(1)
			.ok_or(ProgramError::ArithmeticOverflow)?;
		let now = read_timestamp(self.clock)?;
		let expires_at = if multisig_ttl == 0 {
			0
		} else {
			now.checked_add(i64::from(multisig_ttl))
				.ok_or(ProgramError::ArithmeticOverflow)?
		};

		// Derive the proposal address once so ephemeral signers and the vault
		// bump seed from the exact address the account will land at.
		let seeds = Proposal::seeds(&multisig_key, transaction_index);
		let proposal_seeds = seeds.with_bump(args.bump);
		let proposal_key = create_program_address(&proposal_seeds.as_slices(), &ID)?;

		let mut ephemeral_bumps = [0_u8; MAX_EPHEMERAL_SIGNERS];
		for (position, slot) in ephemeral_bumps.iter_mut().enumerate().take(ephemeral_count) {
			let position_bytes = [position as u8];
			let Some((_, bump)) = try_find_program_address(
				&[
					SEED_EPHEMERAL_SIGNER,
					proposal_key.as_ref(),
					&position_bytes,
				],
				&ID,
			) else {
				return Err(ProgramError::InvalidSeeds);
			};
			*slot = bump;
		}

		let vault_bump = if kind == KIND_VAULT {
			let vault_index_bytes = [args.vault_index];
			let Some((_, vault_bump)) = try_find_program_address(
				&[SEED_VAULT, multisig_key.as_ref(), &vault_index_bytes],
				&ID,
			) else {
				return Err(ProgramError::InvalidSeeds);
			};
			vault_bump
		} else {
			0
		};

		let space = Proposal::projected_bytes(
			if kind == KIND_VAULT {
				ephemeral_count
			} else {
				0
			},
			if kind == KIND_VAULT { message_len } else { 0 },
			if kind == KIND_CONFIG { actions_len } else { 0 },
		)?;

		let mut patch = ProposalPatch::new()
			.bump(args.bump)
			.multisig(multisig_key)
			.creator(*self.creator.address())
			.index(transaction_index)
			.kind(kind)
			.vault_index(args.vault_index)
			.vault_bump(vault_bump)
			.status(STATUS_DRAFT)
			.status_at(now)
			.expires_at(expires_at)
			.approved_mask(0)
			.rejected_mask(0);
		if kind == KIND_VAULT {
			patch = patch
				.replace_ephemeral_bumps(&ephemeral_bumps[..ephemeral_count])
				.replace_message(message);
		} else {
			patch = patch.replace_actions(actions);
		}

		CreateCompactProgramAccountWithBump {
			account: self.proposal,
			payer: self.rent_payer,
			owner: &ID,
			// The builder appends the bump itself, so hand it the seeds
			// without one.
			seeds: &seeds.as_slices(),
			bump: args.bump,
			patch,
			space,
		}
		.invoke::<Proposal>()?;

		// The transaction index lives in the multisig header, so the bump is
		// an in-place write with no resize CPI.
		let mut multisig_data = self.multisig.try_borrow_mut()?;
		Multisig::update(
			&mut multisig_data,
			&MultisigPatch::new().transaction_index(transaction_index),
		)?;
		drop(multisig_data);

		emit_proposal_status(&multisig_key, transaction_index, STATUS_DRAFT, now)?;

		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Instructions: ProposalActivate / Approve / Reject / Revoke / Cancel
// ---------------------------------------------------------------------------

/// The consensus facts a member-driven proposal instruction needs, without
/// copying the member roster onto the stack.
#[derive(Debug, Clone, Copy)]
pub struct MemberContext {
	pub multisig_key: Address,
	pub threshold: u16,
	pub timelock: u32,
	pub stale_transaction_index: u64,
	/// The member's position in the sorted roster; also its vote-mask bit.
	pub member_index: usize,
}

/// Load both accounts for a member-driven proposal instruction: the multisig
/// (seeded by its stored create key) and the proposal (seeded by the multisig
/// and its stored index), checking membership and `permission`. The proposal
/// snapshot travels separately.
fn load_member_proposal(
	multisig: &AccountView,
	proposal: &AccountView,
	member: &Address,
	permission: u8,
) -> Result<(MemberContext, ProposalSnapshot), ProgramError> {
	let create_key = stored_create_key(multisig)?;

	let mut context = MemberContext {
		multisig_key: *multisig.address(),
		threshold: 0,
		timelock: 0,
		stale_transaction_index: 0,
		member_index: 0,
	};
	multisig.with_compact_account::<Multisig, _>(&ID, |state| {
		let mut roster = [Address::default(); MAX_MEMBERS];
		let count = decode_roster(state.member_roster(), &mut roster)?;
		let position = member_index(&roster[..count], member).ok_or(MultisigError::NotAMember)?;
		if state.member_permissions()[position] & permission == 0 {
			return Err(MultisigError::Unauthorized.into());
		}
		context.threshold = state.threshold.get();
		context.timelock = state.timelock.get();
		context.stale_transaction_index = state.stale_transaction_index.get();
		context.member_index = position;
		Ok(())
	})?;
	Multisig::assert_seeds(multisig, &create_key, &ID)?;

	let index = stored_proposal_index(proposal)?;
	let proposal_snapshot = ProposalSnapshot::load(proposal, multisig.address(), index)?;
	Ok((context, proposal_snapshot))
}

/// Write a header-only proposal patch in place. Vote and status transitions
/// never change the encoded length, so this avoids the resize CPI entirely.
fn update_proposal_header(
	proposal: &mut AccountView,
	patch: &ProposalPatch<'_>,
) -> Result<(), ProgramError> {
	let mut data = proposal.try_borrow_mut()?;
	Proposal::update(&mut data, patch)?;
	drop(data);
	Ok(())
}

impl<'a> ProcessAccountInfos<'a> for ProposalActivateAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = ProposalActivateIx::try_from_bytes(data)?;

		self.member.assert_signer()?;
		let (multisig, proposal) = load_member_proposal(
			self.multisig,
			self.proposal,
			self.member.address(),
			PERMISSION_INITIATE,
		)?;
		if proposal.status != STATUS_DRAFT {
			return Err(MultisigError::InvalidProposalStatus.into());
		}
		if proposal.index <= multisig.stale_transaction_index {
			return Err(MultisigError::StaleProposal.into());
		}
		let now = read_timestamp(self.clock)?;
		if is_expired(proposal.expires_at, now) {
			return Err(MultisigError::ProposalExpired.into());
		}

		update_proposal_header(
			self.proposal,
			&ProposalPatch::new().status(STATUS_ACTIVE).status_at(now),
		)?;
		emit_proposal_status(&multisig.multisig_key, proposal.index, STATUS_ACTIVE, now)?;

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for ProposalApproveAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = ProposalApproveIx::try_from_bytes(data)?;

		self.member.assert_signer()?;
		let (multisig, proposal) = load_member_proposal(
			self.multisig,
			self.proposal,
			self.member.address(),
			PERMISSION_VOTE,
		)?;
		if proposal.status != STATUS_ACTIVE {
			return Err(MultisigError::InvalidProposalStatus.into());
		}
		if proposal.index <= multisig.stale_transaction_index {
			return Err(MultisigError::StaleProposal.into());
		}

		let now = read_timestamp(self.clock)?;
		if is_expired(proposal.expires_at, now) {
			return Err(MultisigError::ProposalExpired.into());
		}

		let bit = member_bit(multisig.member_index)?;
		if proposal.approved_mask & bit != 0 {
			return Err(MultisigError::AlreadyVoted.into());
		}

		// Approving clears any prior rejection: vote switching with one
		// word of state.
		let approved_mask = proposal.approved_mask | bit;
		let rejected_mask = proposal.rejected_mask & !bit;
		let status = if mask_count(approved_mask) >= usize::from(multisig.threshold) {
			STATUS_APPROVED
		} else {
			STATUS_ACTIVE
		};

		update_proposal_header(
			self.proposal,
			&ProposalPatch::new()
				.approved_mask(approved_mask)
				.rejected_mask(rejected_mask)
				.status(status)
				.status_at(now),
		)?;
		emit_proposal_status(&multisig.multisig_key, proposal.index, status, now)?;

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for ProposalRejectAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = ProposalRejectIx::try_from_bytes(data)?;

		self.member.assert_signer()?;
		let (multisig, proposal) = load_member_proposal(
			self.multisig,
			self.proposal,
			self.member.address(),
			PERMISSION_VOTE,
		)?;
		if proposal.status != STATUS_ACTIVE {
			return Err(MultisigError::InvalidProposalStatus.into());
		}
		if proposal.index <= multisig.stale_transaction_index {
			return Err(MultisigError::StaleProposal.into());
		}

		let now = read_timestamp(self.clock)?;
		if is_expired(proposal.expires_at, now) {
			return Err(MultisigError::ProposalExpired.into());
		}

		let bit = member_bit(multisig.member_index)?;
		if proposal.rejected_mask & bit != 0 {
			return Err(MultisigError::AlreadyVoted.into());
		}

		// The cutoff counts voters on the current roster: once this many
		// reject, approval is arithmetically impossible.
		let cutoff = {
			let mut cutoff = 0;
			self.multisig
				.with_compact_account::<Multisig, _>(&ID, |state| {
					cutoff = rejection_cutoff(state.member_permissions(), state.threshold.get())?;
					Ok(())
				})?;
			cutoff
		};
		let rejected_mask = proposal.rejected_mask | bit;
		let approved_mask = proposal.approved_mask & !bit;
		let status = if mask_count(rejected_mask) >= cutoff {
			STATUS_REJECTED
		} else {
			STATUS_ACTIVE
		};

		update_proposal_header(
			self.proposal,
			&ProposalPatch::new()
				.approved_mask(approved_mask)
				.rejected_mask(rejected_mask)
				.status(status)
				.status_at(now),
		)?;
		emit_proposal_status(&multisig.multisig_key, proposal.index, status, now)?;

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for ProposalRevokeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = ProposalRevokeIx::try_from_bytes(data)?;

		self.member.assert_signer()?;
		let (multisig, proposal) = load_member_proposal(
			self.multisig,
			self.proposal,
			self.member.address(),
			PERMISSION_VOTE,
		)?;
		// Revocation reads the member bit for the *current* roster, so it is
		// refused on stale proposals: after a membership change the bit no
		// longer names the approver who set it.
		if proposal.status != STATUS_APPROVED {
			return Err(MultisigError::InvalidProposalStatus.into());
		}
		if proposal.index <= multisig.stale_transaction_index {
			return Err(MultisigError::StaleProposal.into());
		}

		let bit = member_bit(multisig.member_index)?;
		if proposal.approved_mask & bit == 0 {
			return Err(MultisigError::HasNotApproved.into());
		}

		let approved_mask = proposal.approved_mask & !bit;
		// Dropping below the threshold returns the proposal to active, so the
		// remaining approvers must refresh consent — including waiting out a
		// fresh timelock after re-approval.
		let status = if mask_count(approved_mask) < usize::from(multisig.threshold) {
			STATUS_ACTIVE
		} else {
			STATUS_APPROVED
		};
		let now = read_timestamp(self.clock)?;

		update_proposal_header(
			self.proposal,
			&ProposalPatch::new()
				.approved_mask(approved_mask)
				.status(status)
				.status_at(now),
		)?;
		emit_proposal_status(&multisig.multisig_key, proposal.index, status, now)?;

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for ProposalCancelAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = ProposalCancelIx::try_from_bytes(data)?;

		self.member.assert_signer()?;
		let (_multisig, proposal) = load_member_proposal(
			self.multisig,
			self.proposal,
			self.member.address(),
			PERMISSION_INITIATE,
		)?;
		// Only the creator may cancel, and only before activation: once votes
		// exist the rejection and revocation paths govern.
		if proposal.status != STATUS_DRAFT {
			return Err(MultisigError::InvalidProposalStatus.into());
		}
		if proposal.creator != *self.member.address() {
			return Err(MultisigError::Unauthorized.into());
		}
		let now = read_timestamp(self.clock)?;

		update_proposal_header(
			self.proposal,
			&ProposalPatch::new().status(STATUS_CANCELLED).status_at(now),
		)?;
		emit_proposal_status(
			self.multisig.address(),
			proposal.index,
			STATUS_CANCELLED,
			now,
		)?;

		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Instruction: VaultExecute
// ---------------------------------------------------------------------------

impl<'a> ProcessAccountInfos<'a> for VaultExecuteAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = VaultExecuteIx::try_from_bytes(data)?;

		self.member.assert_signer()?;
		let (multisig, proposal) = load_member_proposal(
			self.multisig,
			self.proposal,
			self.member.address(),
			PERMISSION_EXECUTE,
		)?;
		if proposal.kind != KIND_VAULT {
			return Err(MultisigError::InvalidProposalKind.into());
		}
		// A proposal approved before the roster changed stays executable:
		// the recorded consent already met the old threshold. The timelock
		// still anchors at approval time.
		if proposal.status != STATUS_APPROVED {
			return Err(MultisigError::InvalidProposalStatus.into());
		}
		let now = read_timestamp(self.clock)?;
		let elapsed = now
			.checked_sub(proposal.status_at)
			.ok_or(ProgramError::ArithmeticOverflow)?;
		if elapsed < i64::from(multisig.timelock) {
			return Err(MultisigError::TimeLockNotReleased.into());
		}
		// Expired consent dies with the proposal: an approved-then-expired
		// transaction can no longer be executed.
		if is_expired(proposal.expires_at, now) {
			return Err(MultisigError::ProposalExpired.into());
		}

		let multisig_key = multisig.multisig_key;
		let proposal_key = *self.proposal.address();

		// Copy the ephemeral bumps out of the account; the message itself is
		// parsed directly from the borrowed data below.
		let mut bumps = [0_u8; MAX_EPHEMERAL_SIGNERS];
		let mut ephemeral_count = 0;
		self.proposal
			.with_compact_account::<Proposal, _>(&ID, |state| {
				ephemeral_count = state.ephemeral_bumps().len();
				bumps[..ephemeral_count].copy_from_slice(state.ephemeral_bumps());
				if state.message().is_empty() {
					return Err(MultisigError::InvalidMessage.into());
				}
				Ok(())
			})?;

		// Derive the vault and ephemeral signer PDAs: the accounts the
		// message may declare as signers even though they cannot sign the
		// outer transaction. Seed byte buffers live in this frame so the
		// signers can borrow them through the invokes below.
		let vault_index_bytes = [proposal.vault_index];
		let vault_bump_bytes = [proposal.vault_bump];
		let position_bytes =
			core::array::from_fn::<u8, MAX_EPHEMERAL_SIGNERS, _>(|position| position as u8);
		let vault_key = create_program_address(
			&[
				SEED_VAULT,
				multisig_key.as_ref(),
				&vault_index_bytes,
				&vault_bump_bytes,
			],
			&ID,
		)?;
		let mut ephemeral_keys = [Address::default(); MAX_EPHEMERAL_SIGNERS];
		for position in 0..ephemeral_count {
			ephemeral_keys[position] = create_program_address(
				&[
					SEED_EPHEMERAL_SIGNER,
					proposal_key.as_ref(),
					&position_bytes[position..=position],
					&bumps[position..=position],
				],
				&ID,
			)?;
		}

		// The protected set: message instructions may read the multisig and
		// this proposal but never write them.
		let protected = [multisig_key, proposal_key];

		// Parse the message straight from the borrowed account data: the
		// BPF stack cannot afford a second copy alongside the signer set.
		let proposal_data = self.proposal.try_borrow()?;
		let mut message_offset = 0;
		let mut message_len = 0;
		self.proposal
			.with_compact_account::<Proposal, _>(&ID, |state| {
				let bytes = state.message();
				let offset = bytes.as_ptr() as usize - proposal_data.as_ptr() as usize;
				message_offset = offset;
				message_len = bytes.len();
				Ok(())
			})?;
		let message = MessageView::parse(
			proposal_data
				.get(message_offset..message_offset + message_len)
				.ok_or(MultisigError::InvalidMessage)?,
		)?;

		if self.message_accounts.len() != message.num_accounts() {
			return Err(MultisigError::InvalidNumberOfAccounts.into());
		}
		for account_index in 0..message.num_accounts() {
			let expected = message.account_key(account_index)?;
			let view = self
				.message_accounts
				.get(account_index)
				.ok_or(MultisigError::InvalidNumberOfAccounts)?;
			if view.address() != &expected {
				return Err(MultisigError::InvalidAccount.into());
			}
			if message.is_writable_index(account_index) {
				if !view.is_writable() {
					return Err(MultisigError::InvalidAccount.into());
				}
				if protected.contains(view.address()) {
					return Err(MultisigError::ProtectedAccount.into());
				}
			}
			if message.is_signer_index(account_index)
				&& view.address() != &vault_key
				&& !ephemeral_keys[..ephemeral_count].contains(view.address())
				&& !view.is_signer()
			{
				return Err(MultisigError::InvalidAccount.into());
			}
		}
		// PDA signers: the vault plus one per ephemeral signer, all owned in
		// this frame so the borrowed `Signer` values live through the invokes.
		let vault_signer_storage = PdaSigner::from_slices([
			SEED_VAULT,
			multisig_key.as_ref(),
			&vault_index_bytes,
			&vault_bump_bytes,
		]);
		let zero_bytes = [0_u8; 1];
		let mut ephemeral_signer_storage: [PdaSigner<'_, 4>; MAX_EPHEMERAL_SIGNERS] =
			core::array::from_fn(|_| {
				PdaSigner::from_slices([
					SEED_EPHEMERAL_SIGNER,
					proposal_key.as_ref(),
					&zero_bytes,
					&zero_bytes,
				])
			});
		for position in 0..ephemeral_count {
			ephemeral_signer_storage[position] = PdaSigner::from_slices([
				SEED_EPHEMERAL_SIGNER,
				proposal_key.as_ref(),
				&position_bytes[position..=position],
				&bumps[position..=position],
			]);
		}
		let mut signers: [Signer<'_, '_>; MAX_EPHEMERAL_SIGNERS + 1] =
			core::array::from_fn(|_| vault_signer_storage.as_signer());
		for position in 0..ephemeral_count {
			signers[position + 1] = ephemeral_signer_storage[position].as_signer();
		}
		let signers = &signers[..=ephemeral_count];

		for instruction_index in 0..message.num_instructions() {
			let instruction = message.instruction(instruction_index)?;
			let program_view = self
				.message_accounts
				.get(instruction.program_id_index)
				.ok_or(MultisigError::InvalidAccount)?;
			if !program_view.executable() {
				return Err(MultisigError::InvalidAccount.into());
			}

			let filler = Address::default();
			let mut instruction_accounts: [InstructionAccount<'_>; MAX_MESSAGE_ACCOUNTS] =
				core::array::from_fn(|_| InstructionAccount::readonly(&filler));
			let mut account_views: [&AccountView; MAX_MESSAGE_ACCOUNTS] =
				[self.message_accounts
					.first()
					.ok_or(MultisigError::InvalidNumberOfAccounts)?; MAX_MESSAGE_ACCOUNTS];
			for (slot, message_index) in instruction.account_indexes.iter().enumerate() {
				let index = usize::from(*message_index);
				let view = self
					.message_accounts
					.get(index)
					.ok_or(MultisigError::InvalidAccount)?;
				instruction_accounts[slot] = InstructionAccount::new(
					view.address(),
					message.is_writable_index(index),
					message.is_signer_index(index),
				);
				account_views[slot] = view;
			}

			let view = InstructionView {
				program_id: program_view.address(),
				data: instruction.data,
				accounts: &instruction_accounts[..instruction.account_indexes.len()],
			};
			invoke_signed_with_bounds::<MAX_MESSAGE_ACCOUNTS, &AccountView>(
				&view,
				&account_views[..instruction.account_indexes.len()],
				signers,
			)?;
		}
		drop(proposal_data);

		update_proposal_header(
			self.proposal,
			&ProposalPatch::new().status(STATUS_EXECUTED).status_at(now),
		)?;
		emit_proposal_status(&multisig_key, proposal.index, STATUS_EXECUTED, now)?;

		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Instructions: ConfigExecute / ConfigAuthorityExecute
// ---------------------------------------------------------------------------

/// Commit the working multisig state after a config action stream: validate
/// the invariants, invalidate prior proposals when consensus moved, and
/// write through the resize builder (member tails may have grown).
fn commit_multisig(
	multisig_account: &mut AccountView,
	working: &mut MultisigWorkingState,
	rent_payer: &mut AccountView,
) -> Result<(), ProgramError> {
	working.validate()?;

	let stale_transaction_index = if working.changed {
		working.transaction_index
	} else {
		current_stale_index(multisig_account)?
	};

	UpdateResizableAccount {
		account: multisig_account,
		rent_account: rent_payer,
		program_id: &ID,
		patch: MultisigPatch::new()
			.threshold(working.threshold)
			.timelock(working.timelock)
			.ttl(working.ttl)
			.rent_collector(working.rent_collector)
			.config_authority(working.config_authority)
			.stale_transaction_index(stale_transaction_index)
			.replace_member_roster(
				&flatten_roster::<512>(&working.member_keys[..working.member_count])
					[..working.member_count * 32],
			)
			.replace_member_permissions(&working.member_permissions[..working.member_count]),
	}
	.invoke::<Multisig>()?;

	Ok(())
}

fn current_stale_index(multisig_account: &AccountView) -> Result<u64, ProgramError> {
	let mut stale = 0;
	multisig_account.with_compact_account::<Multisig, _>(&ID, |state| {
		stale = state.stale_transaction_index.get();
		Ok(())
	})?;
	Ok(stale)
}

impl<'a> ProcessAccountInfos<'a> for ConfigExecuteAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = ConfigExecuteIx::try_from_bytes(data)?;

		self.member.assert_signer()?;
		self.rent_payer.assert_signer()?.assert_writable()?;
		self.system_program.assert_address(&system::ID)?;
		let create_key = stored_create_key(self.multisig)?;
		let multisig = MultisigSnapshot::load(self.multisig, &create_key)?;
		if is_controlled(&multisig.config_authority) {
			return Err(MultisigError::NotSupportedForControlled.into());
		}
		if !multisig.is_member(self.member.address()) {
			return Err(MultisigError::NotAMember.into());
		}
		if multisig.permissions_of(self.member.address()) & PERMISSION_EXECUTE == 0 {
			return Err(MultisigError::Unauthorized.into());
		}

		let index = stored_proposal_index(self.proposal)?;
		let proposal = ProposalSnapshot::load(self.proposal, self.multisig.address(), index)?;
		if proposal.kind != KIND_CONFIG {
			return Err(MultisigError::InvalidProposalKind.into());
		}
		// Unlike vault proposals, a config proposal must still be fresh at
		// execution: it rewrites the consensus that approved it.
		if proposal.status != STATUS_APPROVED {
			return Err(MultisigError::InvalidProposalStatus.into());
		}
		if proposal.index <= multisig.stale_transaction_index {
			return Err(MultisigError::StaleProposal.into());
		}
		let now = read_timestamp(self.clock)?;
		let elapsed = now
			.checked_sub(proposal.status_at)
			.ok_or(ProgramError::ArithmeticOverflow)?;
		if elapsed < i64::from(multisig.timelock) {
			return Err(MultisigError::TimeLockNotReleased.into());
		}

		let mut actions = [0_u8; MAX_ACTIONS_BYTES];
		let mut actions_len = 0;
		self.proposal
			.with_compact_account::<Proposal, _>(&ID, |state| {
				actions_len = state.actions().len();
				actions[..actions_len].copy_from_slice(state.actions());
				Ok(())
			})?;
		if actions_len == 0 {
			return Err(MultisigError::InvalidActions.into());
		}

		let multisig_key = *self.multisig.address();
		let mut working = MultisigWorkingState::capture(&multisig);
		apply_config_actions(
			&multisig_key,
			&mut working,
			&actions[..actions_len],
			self.spending_limit_accounts,
			self.rent_payer,
			self.system_program,
			now,
		)?;
		commit_multisig(self.multisig, &mut working, self.rent_payer)?;

		update_proposal_header(
			self.proposal,
			&ProposalPatch::new().status(STATUS_EXECUTED).status_at(now),
		)?;
		emit_proposal_status(&multisig_key, proposal.index, STATUS_EXECUTED, now)?;

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for ConfigAuthorityExecuteAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = ConfigAuthorityExecuteIx::try_from_bytes(data)?;
		let actions_len = usize::from(args.actions_len.get());
		if actions_len == 0 || actions_len > MAX_ACTIONS_BYTES {
			return Err(MultisigError::InvalidActions.into());
		}
		let actions_bytes = &args.actions[..actions_len];
		validate_actions(actions_bytes)?;

		self.authority.assert_signer()?;
		self.rent_payer.assert_signer()?.assert_writable()?;
		self.system_program.assert_address(&system::ID)?;
		let create_key = stored_create_key(self.multisig)?;
		let multisig = MultisigSnapshot::load(self.multisig, &create_key)?;
		if !is_controlled(&multisig.config_authority) {
			return Err(MultisigError::NotSupportedForAutonomous.into());
		}
		if multisig.config_authority != *self.authority.address() {
			return Err(MultisigError::InvalidConfigAuthority.into());
		}

		let multisig_key = *self.multisig.address();
		let now = read_timestamp(self.clock)?;
		let mut working = MultisigWorkingState::capture(&multisig);
		apply_config_actions(
			&multisig_key,
			&mut working,
			actions_bytes,
			self.spending_limit_accounts,
			self.rent_payer,
			self.system_program,
			now,
		)?;
		commit_multisig(self.multisig, &mut working, self.rent_payer)?;

		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Instruction: SpendingLimitUse
// ---------------------------------------------------------------------------

impl<'a> ProcessAccountInfos<'a> for SpendingLimitUseAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = SpendingLimitUseIx::try_from_bytes(data)?;
		let amount = args.amount.get();
		if amount == 0 {
			return Err(MultisigError::SpendingLimitExceeded.into());
		}

		self.member.assert_signer()?;
		let member_key = *self.member.address();
		let multisig_key = *self.multisig.address();

		// Validate the multisig account is the real stored-bump PDA before
		// trusting anything the spending limit says about it.
		let create_key = stored_create_key(self.multisig)?;
		MultisigSnapshot::load(self.multisig, &create_key)?;

		let mut limit_create_key = Address::default();
		let mut mint = Address::default();
		let mut vault_index = 0_u8;
		let mut period = PERIOD_ONE_TIME;
		let mut allowance = 0_u64;
		let mut remaining_amount = 0_u64;
		let mut last_reset = 0_i64;
		let mut destinations = [Address::default(); MAX_SPENDING_LIMIT_DESTINATIONS];
		let mut destination_count = 0;
		self.spending_limit
			.with_compact_account::<SpendingLimit, _>(&ID, |limit| {
				if limit.multisig != multisig_key {
					return Err(ProgramError::InvalidSeeds);
				}
				let mut members = [Address::default(); MAX_SPENDING_LIMIT_MEMBERS];
				let members_count = decode_roster(limit.members(), &mut members)?;
				if !members[..members_count].contains(&member_key) {
					return Err(MultisigError::Unauthorized.into());
				}
				limit_create_key = limit.create_key;
				mint = limit.mint;
				vault_index = limit.vault_index;
				period = limit.period;
				allowance = limit.amount.get();
				remaining_amount = limit.remaining_amount.get();
				last_reset = limit.last_reset.get();
				destination_count = decode_roster(limit.destinations(), &mut destinations)?;
				Ok(())
			})?;
		SpendingLimit::assert_seeds(self.spending_limit, &multisig_key, &limit_create_key, &ID)?;

		let now = read_timestamp(self.clock)?;

		// Roll the period forward when it has fully elapsed, keeping the
		// anchor aligned to whole periods.
		let (rolled_anchor, reset) = roll_period(now, last_reset, period)?;
		last_reset = rolled_anchor;
		if reset {
			remaining_amount = allowance;
		}

		// Destination allow-list: empty means unrestricted.
		if destination_count != 0
			&& !destinations[..destination_count].contains(self.destination.address())
		{
			return Err(MultisigError::InvalidDestination.into());
		}

		remaining_amount = remaining_amount
			.checked_sub(amount)
			.ok_or(MultisigError::SpendingLimitExceeded)?;

		// The vault PDA signs either flavor of transfer.
		let vault_index_bytes = [vault_index];
		let vault_seeds = [SEED_VAULT, multisig_key.as_ref(), &vault_index_bytes[..]];
		let Some((vault_key, vault_bump)) = try_find_program_address(&vault_seeds, &ID) else {
			return Err(ProgramError::InvalidSeeds);
		};
		self.vault.assert_address(&vault_key)?;
		let vault_bump_bytes = [vault_bump];
		let vault_signer_storage = PdaSigner::from_slices([
			SEED_VAULT,
			multisig_key.as_ref(),
			&vault_index_bytes,
			&vault_bump_bytes,
		]);
		let signers = [vault_signer_storage.as_signer()];

		if mint == Address::default() {
			// SOL: the vault account is the source and the destination is a
			// plain account.
			let system_program = self.system_program.ok_or(MultisigError::MissingAccount)?;
			system_program.assert_address(&system::ID)?;
			if args.decimals != 9 {
				return Err(MultisigError::DecimalsMismatch.into());
			}

			system::instructions::Transfer {
				from: self.vault,
				to: self.destination,
				lamports: amount,
			}
			.invoke_signed(&signers)?;
		} else {
			// SPL: transfer-checked from the vault token account.
			let mint_account = self.mint.ok_or(MultisigError::InvalidMint)?;
			mint_account.assert_address(&mint)?;
			let vault_token_account = self
				.vault_token_account
				.ok_or(MultisigError::MissingAccount)?;
			let token_program = self.token_program.ok_or(MultisigError::MissingAccount)?;
			token_program.assert_address(&token::ID)?;

			token::instructions::TransferChecked::new(
				vault_token_account,
				mint_account,
				self.destination,
				self.vault,
				amount,
				args.decimals,
			)
			.invoke_signed_with_program(&signers, token_program.address())?;
		}

		// Persist the allowance draw in place; the encoded size cannot change.
		{
			let mut limit_data = self.spending_limit.try_borrow_mut()?;
			SpendingLimit::update(
				&mut limit_data,
				&SpendingLimitPatch::new()
					.remaining_amount(remaining_amount)
					.last_reset(last_reset),
			)?;
		}

		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Instruction: ProposalClose
// ---------------------------------------------------------------------------

impl<'a> ProcessAccountInfos<'a> for ProposalCloseAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = ProposalCloseIx::try_from_bytes(data)?;

		let create_key = stored_create_key(self.multisig)?;
		let multisig = MultisigSnapshot::load(self.multisig, &create_key)?;
		if multisig.rent_collector == Address::default() {
			return Err(MultisigError::InvalidConfiguration.into());
		}
		self.rent_collector
			.assert_address(&multisig.rent_collector)?;

		let index = stored_proposal_index(self.proposal)?;
		let proposal = ProposalSnapshot::load(self.proposal, self.multisig.address(), index)?;
		if !matches!(
			proposal.status,
			STATUS_EXECUTED | STATUS_REJECTED | STATUS_CANCELLED
		) {
			return Err(MultisigError::InvalidProposalStatus.into());
		}

		self.proposal
			.close_account_zeroed(&ID, self.rent_collector)?;

		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Entrypoint
// ---------------------------------------------------------------------------

#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use super::*;

	nostd_entrypoint!(MultisigInstruction::process_instruction);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	extern crate std;

	use std::vec::Vec;

	use super::*;

	fn key(seed: u8) -> Address {
		let mut bytes = [0_u8; 32];
		bytes[0] = seed;
		Address::new_from_array(bytes)
	}

	fn sorted_members(count: usize) -> ([Address; MAX_MEMBERS], [u8; MAX_MEMBERS]) {
		let mut keys = [Address::default(); MAX_MEMBERS];
		let mut permissions = [0_u8; MAX_MEMBERS];
		// Descending seeds then reversed: distinct, ascending addresses.
		for position in 0..count {
			keys[position] = key(255 - position as u8);
			permissions[position] = PERMISSIONS_ALL;
		}
		keys[..count].reverse();
		(keys, permissions)
	}

	#[test]
	fn instruction_and_account_discriminators_are_stable() {
		assert_eq!(MultisigInstruction::ConfigInitialize as u8, 0);
		assert_eq!(MultisigInstruction::MultisigCreate as u8, 2);
		assert_eq!(MultisigInstruction::MultisigImport as u8, 3);
		assert_eq!(MultisigInstruction::ProposalCreate as u8, 4);
		assert_eq!(MultisigInstruction::VaultExecute as u8, 10);
		assert_eq!(MultisigInstruction::SpendingLimitUse as u8, 13);
		assert_eq!(MultisigInstruction::ProposalClose as u8, 14);

		assert_eq!(MultisigAccountType::ProgramConfig as u8, 1);
		assert_eq!(MultisigAccountType::Multisig as u8, 2);
		assert_eq!(MultisigAccountType::Proposal as u8, 3);
		assert_eq!(MultisigAccountType::SpendingLimit as u8, 4);
		assert_eq!(MultisigEventType::ProposalStatus as u8, 1);
	}

	#[test]
	fn membership_validation_accepts_a_healthy_roster() {
		let (keys, permissions) = sorted_members(3);
		assert_eq!(validate_members(&keys[..3], &permissions[..3], 2), Ok(()));
	}

	#[test]
	fn membership_validation_rejects_broken_rosters() {
		let (keys, permissions) = sorted_members(3);

		// Duplicate (and therefore unsorted) member.
		let mut duplicated = keys;
		duplicated[1] = duplicated[0];
		assert_eq!(
			validate_members(&duplicated[..3], &permissions[..3], 2),
			Err(MultisigError::DuplicateMember.into())
		);

		// Unknown permission bit.
		let mut alien = permissions;
		alien[0] |= 0b1000_0000;
		assert_eq!(
			validate_members(&keys[..3], &alien[..3], 2),
			Err(MultisigError::UnknownPermission.into())
		);

		// Threshold of zero, and above the voter count.
		assert_eq!(
			validate_members(&keys[..3], &permissions[..3], 0),
			Err(MultisigError::InvalidThreshold.into())
		);
		assert_eq!(
			validate_members(&keys[..3], &permissions[..3], 4),
			Err(MultisigError::InvalidThreshold.into())
		);

		// Missing a core permission holder strands the multisig: nobody can
		// vote when every member is initiate-only.
		let initiate_only = [PERMISSION_INITIATE; MAX_MEMBERS];
		assert_eq!(
			validate_members(&keys[..3], &initiate_only[..3], 1),
			Err(MultisigError::InvalidConfiguration.into())
		);

		// Empty rosters and length disagreement.
		assert_eq!(
			validate_members(&[], &[], 1),
			Err(MultisigError::InvalidConfiguration.into())
		);
		assert_eq!(
			validate_members(&keys[..3], &permissions[..2], 1),
			Err(MultisigError::InvalidConfiguration.into())
		);

		// A full roster at the cap is healthy.
		let (full_keys, full_permissions) = sorted_members(MAX_MEMBERS);
		assert_eq!(validate_members(&full_keys, &full_permissions, 2), Ok(()));
	}

	#[test]
	fn rejection_cutoff_prevents_impossible_approvals() {
		let (_, permissions) = sorted_members(7);
		// Seven voters, threshold three: five rejections make approval
		// impossible.
		assert_eq!(rejection_cutoff(&permissions[..7], 3), Ok(5));
		assert_eq!(rejection_cutoff(&permissions[..7], 7), Ok(1));
		assert_eq!(rejection_cutoff(&permissions[..1], 1), Ok(1));

		let mut mixed = [0_u8; MAX_MEMBERS];
		mixed[0] = PERMISSION_VOTE | PERMISSION_INITIATE;
		mixed[1] = PERMISSION_VOTE | PERMISSION_EXECUTE;
		mixed[2] = PERMISSION_INITIATE | PERMISSION_EXECUTE;
		// Only two voters; a threshold of two leaves no rejection slack.
		assert_eq!(rejection_cutoff(&mixed[..3], 2), Ok(1));
	}

	#[test]
	fn vote_masks_count_and_bound_their_bits() {
		assert_eq!(member_bit(0), Ok(1));
		assert_eq!(member_bit(31), Ok(1 << 31));
		assert!(member_bit(32).is_err());
		assert_eq!(mask_count(0b1011), 3);
		assert_eq!(mask_count(1 << 31), 1);
	}

	#[test]
	fn message_codec_roundtrips_counters_keys_and_instructions() {
		let account_keys = [key(1), key(2), key(3), key(4)];
		let mut buffer = [0_u8; MAX_MESSAGE_BYTES];
		let length = encode_message(
			1,
			1,
			1,
			&account_keys,
			&[(3, &[0, 2], &[9, 9]), (3, &[], &[])],
			&mut buffer,
		)
		.unwrap_or_else(|error| panic!("encode message: {error:?}"));

		let message = MessageView::parse(&buffer[..length])
			.unwrap_or_else(|error| panic!("parse message: {error:?}"));
		assert_eq!(message.num_accounts(), 4);
		assert_eq!(message.num_instructions(), 2);
		assert_eq!(message.account_key(2), Ok(key(3)));
		assert!(message.is_signer_index(0));
		assert!(!message.is_signer_index(1));
		assert!(message.is_writable_index(0));
		// Index 1 is the writable non-signer; index 2 is readonly.
		assert!(message.is_writable_index(1));
		assert!(!message.is_writable_index(2));

		let first = message
			.instruction(0)
			.unwrap_or_else(|error| panic!("first instruction: {error:?}"));
		assert_eq!(first.program_id_index, 3);
		assert_eq!(first.account_indexes, &[0, 2]);
		assert_eq!(first.data, &[9, 9]);
		let second = message
			.instruction(1)
			.unwrap_or_else(|error| panic!("second instruction: {error:?}"));
		assert_eq!(second.account_indexes, &[] as &[u8]);
		assert_eq!(second.data, &[] as &[u8]);
		assert!(message.instruction(2).is_err());
	}

	#[test]
	fn message_parser_rejects_every_truncation_and_bad_counter() {
		let account_keys = [key(1), key(2)];
		let mut buffer = [0_u8; MAX_MESSAGE_BYTES];
		let length = encode_message(
			1,
			1,
			0,
			&account_keys,
			&[(1, &[0], &[1, 2, 3])],
			&mut buffer,
		)
		.unwrap_or_else(|error| panic!("encode message: {error:?}"));

		for truncated in 0..length {
			assert!(
				MessageView::parse(&buffer[..truncated]).is_err(),
				"a message truncated to {truncated} bytes must not parse"
			);
		}
		assert!(MessageView::parse(&buffer[..=length]).is_err());

		// Writable signers may not exceed signers.
		let mut writable_overflow = buffer[..length].to_vec();
		writable_overflow[1] = 2;
		assert!(MessageView::parse(&writable_overflow).is_err());

		// Signers and writable non-signers may not exceed the key count.
		let mut signer_overflow = buffer[..length].to_vec();
		signer_overflow[0] = 3;
		assert!(MessageView::parse(&signer_overflow).is_err());

		// Account and instruction indexes must reference real keys.
		let mut bad_index = buffer[..length].to_vec();
		bad_index[72] = 2; // account index inside the first instruction
		assert!(MessageView::parse(&bad_index).is_err());
		let mut bad_program = buffer[..length].to_vec();
		bad_program[70] = 9; // program id index past the key list
		assert!(MessageView::parse(&bad_program).is_err());
	}

	#[test]
	fn message_parser_enforces_capacity_caps() {
		let mut many_keys = [Address::default(); MAX_MESSAGE_ACCOUNTS + 1];
		for (position, slot) in many_keys.iter_mut().enumerate() {
			*slot = key(position as u8);
		}
		let mut buffer = [0_u8; MAX_MESSAGE_BYTES * 2];
		assert!(encode_message(0, 0, 0, &many_keys, &[], &mut buffer).is_err());
		assert!(
			encode_message(
				0,
				0,
				0,
				&many_keys[..MAX_MESSAGE_ACCOUNTS],
				&[],
				&mut buffer
			)
			.is_ok()
		);

		let mut instruction_overflow = [(0_usize, &[][..], &[][..]); MAX_MESSAGE_INSTRUCTIONS + 1];
		for slot in instruction_overflow
			.iter_mut()
			.take(MAX_MESSAGE_INSTRUCTIONS + 1)
		{
			*slot = (1, &[0], &[]);
		}
		let mut wide = [0_u8; MAX_MESSAGE_BYTES * 2];
		assert!(
			encode_message(
				0,
				0,
				0,
				&many_keys[..MAX_MESSAGE_ACCOUNTS],
				&instruction_overflow,
				&mut wide
			)
			.is_err()
		);
	}

	fn action_buffer() -> [u8; MAX_ACTIONS_BYTES] {
		[0; MAX_ACTIONS_BYTES]
	}

	fn push_address(buffer: &mut Vec<u8>, address: &Address) {
		buffer.extend_from_slice(address.as_ref());
	}

	#[test]
	fn action_stream_roundtrips_every_variant() {
		let member = key(7);
		let authority = key(9);
		let collector = key(11);
		let limit_key = key(13);
		let destination = key(15);

		let mut stream = Vec::new();
		stream.push(9);
		stream.push(ACTION_ADD_MEMBER);
		push_address(&mut stream, &member);
		stream.push(PERMISSIONS_ALL);
		stream.push(ACTION_REMOVE_MEMBER);
		push_address(&mut stream, &key(21));
		stream.push(ACTION_CHANGE_THRESHOLD);
		stream.extend_from_slice(&2_u16.to_le_bytes());
		stream.push(ACTION_SET_TIME_LOCK);
		stream.extend_from_slice(&60_u32.to_le_bytes());
		stream.push(ACTION_SET_PROPOSAL_TTL);
		stream.extend_from_slice(&7200_u32.to_le_bytes());
		stream.push(ACTION_SET_RENT_COLLECTOR);
		stream.push(1);
		push_address(&mut stream, &collector);
		stream.push(ACTION_SET_CONFIG_AUTHORITY);
		stream.push(1);
		push_address(&mut stream, &authority);
		stream.push(ACTION_ADD_SPENDING_LIMIT);
		push_address(&mut stream, &limit_key);
		stream.push(2); // vault index
		push_address(&mut stream, &Address::default()); // SOL
		stream.extend_from_slice(&1_000_u64.to_le_bytes());
		stream.push(PERIOD_WEEK);
		stream.push(1);
		push_address(&mut stream, &member);
		stream.push(1);
		push_address(&mut stream, &destination);
		stream.push(ACTION_REMOVE_SPENDING_LIMIT);
		push_address(&mut stream, &limit_key);

		assert_eq!(validate_actions(&stream), Ok(()));

		let mut seen = Vec::new();
		for_each_action(&stream, |action| {
			match action {
				ConfigActionView::AddMember { key, permissions } => {
					assert_eq!((key, permissions), (member, PERMISSIONS_ALL));
					seen.push("add-member");
				}
				ConfigActionView::RemoveMember { .. } => seen.push("remove-member"),
				ConfigActionView::ChangeThreshold { threshold } => {
					assert_eq!(threshold, 2);
					seen.push("threshold");
				}
				ConfigActionView::SetTimeLock { seconds } => {
					assert_eq!(seconds, 60);
					seen.push("timelock");
				}
				ConfigActionView::SetProposalTtl { seconds } => {
					assert_eq!(seconds, 7200);
					seen.push("ttl");
				}
				ConfigActionView::SetRentCollector { collector } => {
					assert_eq!(collector, Some(collector_or(collector)));
					seen.push("rent-collector");
				}
				ConfigActionView::SetConfigAuthority { .. } => seen.push("authority"),
				ConfigActionView::AddSpendingLimit {
					vault_index,
					amount,
					period,
					members_len,
					destinations_len,
					..
				} => {
					assert_eq!(vault_index, 2);
					assert_eq!(amount, 1_000);
					assert_eq!(period, PERIOD_WEEK);
					assert_eq!((members_len, destinations_len), (1, 1));
					seen.push("spending-limit");
				}
				ConfigActionView::RemoveSpendingLimit { .. } => seen.push("remove-limit"),
			}
			Ok(())
		})
		.unwrap_or_else(|error| panic!("walk actions: {error:?}"));

		assert_eq!(
			seen,
			[
				"add-member",
				"remove-member",
				"threshold",
				"timelock",
				"ttl",
				"rent-collector",
				"authority",
				"spending-limit",
				"remove-limit"
			]
			.to_vec()
		);
		let _ = action_buffer();
	}

	fn collector_or(option: Option<Address>) -> Address {
		option.expect("collector present")
	}

	#[test]
	fn action_validation_rejects_malformed_streams() {
		// Empty stream.
		assert!(validate_actions(&[]).is_err());

		// Unknown variant tag.
		assert!(validate_actions(&[1, 255]).is_err());

		// Truncated payload for each fixed-width variant.
		assert!(validate_actions(&[1, ACTION_REMOVE_MEMBER, 7]).is_err());
		assert!(validate_actions(&[1, ACTION_CHANGE_THRESHOLD, 1]).is_err());
		assert!(validate_actions(&[1, ACTION_SET_TIME_LOCK, 1, 2, 3]).is_err());
		assert!(validate_actions(&[1, ACTION_SET_RENT_COLLECTOR]).is_err());

		// Semantic guards: timelock past the cap, unknown period, empty roster.
		let mut over_timelock = Vec::new();
		over_timelock.push(1);
		over_timelock.push(ACTION_SET_TIME_LOCK);
		over_timelock.extend_from_slice(&(MAX_TIME_LOCK + 1).to_le_bytes());
		assert_eq!(
			validate_actions(&over_timelock),
			Err(MultisigError::TimeLockExceedsMaxAllowed.into())
		);

		let mut bad_period = Vec::new();
		bad_period.push(1);
		bad_period.push(ACTION_ADD_SPENDING_LIMIT);
		push_address(&mut bad_period, &key(1));
		bad_period.push(0);
		push_address(&mut bad_period, &Address::default());
		bad_period.extend_from_slice(&1_u64.to_le_bytes());
		bad_period.push(9); // not a period
		bad_period.push(0);
		bad_period.push(0);
		assert!(validate_actions(&bad_period).is_err());

		let mut empty_roster = bad_period;
		empty_roster.truncate(empty_roster.len() - 2);
		// Rebuild with members_len = 0 but keep destinations_len = 0.
		let mut zero_members = Vec::new();
		zero_members.push(1);
		zero_members.push(ACTION_ADD_SPENDING_LIMIT);
		push_address(&mut zero_members, &key(1));
		zero_members.push(0);
		push_address(&mut zero_members, &Address::default());
		zero_members.extend_from_slice(&1_u64.to_le_bytes());
		zero_members.push(PERIOD_ONE_TIME);
		zero_members.push(0);
		zero_members.push(0);
		assert!(validate_actions(&zero_members).is_err());
		let _ = empty_roster;

		// Trailing bytes past the declared count.
		assert!(validate_actions(&[0, ACTION_ADD_MEMBER]).is_err());
	}

	const LEGACY_DISCRIMINATOR: [u8; 8] = [0xe0, 0x74, 0x79, 0xba, 0x44, 0xa1, 0x4f, 0xec];

	fn legacy_multisig_bytes() -> Vec<u8> {
		let (keys, permissions) = sorted_members(3);
		let mut bytes = Vec::new();
		bytes.extend_from_slice(&LEGACY_DISCRIMINATOR);
		bytes.extend_from_slice(key(50).as_ref()); // create_key
		bytes.extend_from_slice(Address::default().as_ref()); // autonomous
		bytes.extend_from_slice(&2_u16.to_le_bytes()); // threshold
		bytes.extend_from_slice(&4_320_u32.to_le_bytes()); // timelock
		bytes.extend_from_slice(&9_u64.to_le_bytes()); // transaction_index
		bytes.extend_from_slice(&9_u64.to_le_bytes()); // stale_transaction_index
		bytes.push(0); // rent_collector: None
		bytes.extend_from_slice(Address::default().as_ref()); // padded key
		bytes.push(254); // bump
		bytes.extend_from_slice(&3_u32.to_le_bytes()); // members len
		for position in 0..3 {
			bytes.extend_from_slice(keys[position].as_ref());
			bytes.push(permissions[position]);
		}
		bytes
	}

	#[test]
	fn legacy_parser_reads_a_classic_anchor_layout() {
		let bytes = legacy_multisig_bytes();
		let legacy = LegacyMultisig::parse(&bytes, &LEGACY_DISCRIMINATOR)
			.unwrap_or_else(|error| panic!("parse legacy multisig: {error:?}"));

		assert_eq!(legacy.create_key, key(50));
		assert_eq!(legacy.config_authority, Address::default());
		assert_eq!(legacy.threshold, 2);
		assert_eq!(legacy.time_lock, 4_320);
		assert_eq!(legacy.rent_collector, None);
		assert_eq!(legacy.member_count, 3);
		assert_eq!(legacy.members().len(), 3);
		assert_eq!(legacy.member_permissions[..3], [PERMISSIONS_ALL; 3]);

		// A rent collector carries over too.
		let mut with_collector = bytes.clone();
		with_collector[94] = 1;
		with_collector[95..127].copy_from_slice(key(77).as_ref());
		let legacy = LegacyMultisig::parse(&with_collector, &LEGACY_DISCRIMINATOR)
			.unwrap_or_else(|error| panic!("parse collector: {error:?}"));
		assert_eq!(legacy.rent_collector, Some(key(77)));
	}

	#[test]
	fn legacy_parser_rejects_foreign_or_broken_accounts() {
		let bytes = legacy_multisig_bytes();

		let mut foreign = bytes.clone();
		foreign[..8].copy_from_slice(&[0; 8]);
		assert!(LegacyMultisig::parse(&foreign, &LEGACY_DISCRIMINATOR).is_err());

		assert!(LegacyMultisig::parse(&bytes[..131], &LEGACY_DISCRIMINATOR).is_err());

		let mut too_many = bytes.clone();
		too_many[128..132].copy_from_slice(&25_u32.to_le_bytes());
		assert!(LegacyMultisig::parse(&too_many, &LEGACY_DISCRIMINATOR).is_err());

		// A duplicate member breaks the sorted-roster invariant.
		let mut unsorted = bytes.clone();
		unsorted[132..164].copy_from_slice(key(254).as_ref());
		assert!(LegacyMultisig::parse(&unsorted, &LEGACY_DISCRIMINATOR).is_err());

		// Zero threshold is not a consensus.
		let mut zero_threshold = bytes;
		zero_threshold[72..74].copy_from_slice(&0_u16.to_le_bytes());
		assert!(LegacyMultisig::parse(&zero_threshold, &LEGACY_DISCRIMINATOR).is_err());
	}

	#[test]
	fn instruction_codecs_roundtrip_their_arguments() {
		let (_keys, permissions) = sorted_members(2);
		let mut create_bytes = [0_u8; MultisigCreateIx::SIZE];
		MultisigCreateIx::initialize(&mut create_bytes, |ix| {
			ix.bump = 254;
			ix.threshold.set(2);
			ix.timelock.set(30);
			ix.ttl.set(0);
			ix.member_permissions = permissions;
			ix.config_authority = key(3);
			ix.rent_collector = Address::default();
			Ok(())
		})
		.unwrap_or_else(|error| panic!("initialize create ix: {error:?}"));
		let decoded = MultisigCreateIx::try_from_bytes(&create_bytes)
			.unwrap_or_else(|error| panic!("decode: {error:?}"));
		assert_eq!(decoded.threshold.get(), 2);
		assert_eq!(decoded.member_permissions[0], permissions[0]);
		assert_eq!(decoded.config_authority, key(3));

		let mut proposal_bytes = [0_u8; ProposalCreateIx::SIZE];
		ProposalCreateIx::initialize(&mut proposal_bytes, |ix| {
			ix.bump = 253;
			ix.kind = KIND_VAULT;
			ix.vault_index = 4;
			ix.ephemeral_signers = 2;
			ix.message_len.set(3);
			ix.message = [7; 640];
			ix.actions_len.set(0);
			Ok(())
		})
		.unwrap_or_else(|error| panic!("initialize proposal ix: {error:?}"));
		let decoded = ProposalCreateIx::try_from_bytes(&proposal_bytes)
			.unwrap_or_else(|error| panic!("decode: {error:?}"));
		assert_eq!(decoded.kind, KIND_VAULT);
		assert_eq!(decoded.message_len.get(), 3);
		assert_eq!(&decoded.message[..3], &[7, 7, 7]);

		let mut spend_bytes = [0_u8; SpendingLimitUseIx::SIZE];
		SpendingLimitUseIx::initialize(&mut spend_bytes, |ix| {
			ix.amount.set(123);
			ix.decimals = 9;
			Ok(())
		})
		.unwrap_or_else(|error| panic!("initialize spend ix: {error:?}"));
		let decoded = SpendingLimitUseIx::try_from_bytes(&spend_bytes)
			.unwrap_or_else(|error| panic!("decode: {error:?}"));
		assert_eq!(decoded.amount.get(), 123);
		assert_eq!(decoded.decimals, 9);
	}

	#[test]
	fn compact_size_projections_scale_with_active_tails() {
		assert_eq!(Multisig::projected_bytes(0, 0), Ok(Multisig::HEADER_SIZE));
		let three = Multisig::projected_bytes(3 * 32, 3)
			.unwrap_or_else(|error| panic!("three members: {error:?}"));
		assert_eq!(three, Multisig::HEADER_SIZE + 3 * 32 + 3);
		// Seventeen members exceed the sixteen-slot roster capacity.
		assert!(Multisig::projected_bytes(17 * 32, 17).is_err());

		let vault = Proposal::projected_bytes(2, 100, 0)
			.unwrap_or_else(|error| panic!("vault proposal: {error:?}"));
		assert_eq!(vault, Proposal::HEADER_SIZE + 2 + 100);
		let config = Proposal::projected_bytes(0, 0, 64)
			.unwrap_or_else(|error| panic!("config proposal: {error:?}"));
		assert_eq!(config, Proposal::HEADER_SIZE + 64);
		assert!(Proposal::projected_bytes(0, 641, 0).is_err());
		assert!(Proposal::projected_bytes(9, 0, 0).is_err());

		assert!(SpendingLimit::projected_bytes(2 * 32, 32).is_ok());
		// Rosters must hold whole addresses within their capacities.
		assert!(SpendingLimit::projected_bytes(16 * 32 + 1, 0).is_err());
		assert!(SpendingLimit::projected_bytes(0, 8 * 32 + 1).is_err());
	}

	#[test]
	fn period_rolls_align_to_whole_periods() {
		// One-time limits never roll.
		assert_eq!(roll_period(10_000_000, 0, PERIOD_ONE_TIME), Ok((0, false)));

		let day = DAY_SECONDS;
		// Inside the period: no reset.
		assert_eq!(roll_period(day - 1, 0, PERIOD_DAY), Ok((0, false)));
		// Exactly one period elapsed: the allowance resets only past the boundary.
		assert_eq!(roll_period(day, 0, PERIOD_DAY), Ok((0, false)));
		// A day and a second: one full period advanced.
		assert_eq!(roll_period(day + 1, 0, PERIOD_DAY), Ok((day, true)));
		// Eight days: eight whole periods; the anchor lands on day eight.
		assert_eq!(roll_period(8 * day + 5, 0, PERIOD_DAY), Ok((8 * day, true)));
		// Slow spenders keep an aligned anchor: two full periods past a
		// one-week anchor lands on week three.
		let week = WEEK_SECONDS;
		assert_eq!(
			roll_period(3 * week + 2, week, PERIOD_WEEK),
			Ok((3 * week, true))
		);
	}

	#[test]
	fn proposal_ttl_action_roundtrips_and_caps() {
		let mut stream = Vec::new();
		stream.push(1);
		stream.push(ACTION_SET_PROPOSAL_TTL);
		stream.extend_from_slice(&3600_u32.to_le_bytes());
		assert_eq!(validate_actions(&stream), Ok(()));

		let mut over = Vec::new();
		over.push(1);
		over.push(ACTION_SET_PROPOSAL_TTL);
		over.extend_from_slice(&(MAX_PROPOSAL_TTL + 1).to_le_bytes());
		assert_eq!(
			validate_actions(&over),
			Err(MultisigError::ProposalExpired.into())
		);
	}

	#[test]
	fn expiry_guard_stamps_zero_as_forever() {
		assert!(!is_expired(0, 1_000_000_000));
		assert!(!is_expired(100, 100));
		assert!(!is_expired(100, 99));
		assert!(is_expired(100, 101));
	}

	#[test]
	fn generated_pda_matches_the_documented_seed_derivation() {
		let ms: Address = [7; 32].into();
		let seeds = Proposal::seeds(&ms, 1);
		// The generated seeds struct must encode exactly the documented
		// byte slices: b"proposal", the address, and the u64 little-endian.
		assert_eq!(
			seeds.as_slices(),
			[
				b"proposal".as_slice(),
				ms.as_ref(),
				1_u64.to_le_bytes().as_slice()
			]
		);
		let with_bump = seeds.with_bump(254);
		assert_eq!(with_bump.as_slices()[3], &[254]);
	}

	#[test]
	fn proposal_status_event_codec_roundtrips() {
		let mut bytes = [0_u8; ProposalStatusEvent::SIZE];
		ProposalStatusEvent::initialize(&mut bytes, |event| {
			event.multisig = [9; 32].into();
			event.index.set(4);
			event.status = STATUS_APPROVED;
			event.timestamp.set(1_700_000_000);
			Ok(())
		})
		.unwrap_or_else(|error| panic!("initialize event record: {error:?}"));
		let decoded = ProposalStatusEvent::try_from_bytes(&bytes)
			.unwrap_or_else(|error| panic!("decode event record: {error:?}"));
		assert_eq!(decoded.multisig, [9; 32].into());
		assert_eq!(decoded.index.get(), 4);
		assert_eq!(decoded.status, STATUS_APPROVED);
		assert_eq!(decoded.timestamp.get(), 1_700_000_000);
	}

	#[test]
	fn pdas_are_namespaced_by_their_seeds() {
		let first = key(1);
		let second = key(2);
		assert_ne!(
			Multisig::find_pda(&first, &ID).0,
			Multisig::find_pda(&second, &ID).0
		);
		assert_ne!(
			Proposal::find_pda(&first, 1, &ID).0,
			Proposal::find_pda(&first, 2, &ID).0
		);
		assert_ne!(
			SpendingLimit::find_pda(&first, &second, &ID).0,
			SpendingLimit::find_pda(&second, &first, &ID).0
		);
	}

	#[test]
	fn parse_instruction_rejects_program_id_mismatch() {
		let mut data = [0_u8; 8];
		data[0] = MultisigInstruction::ProposalCreate as u8;
		let foreign = key(9);
		assert!(matches!(
			parse_instruction::<MultisigInstruction>(&foreign, &ID, &data),
			Err(ProgramError::IncorrectProgramId)
		));
	}
}
