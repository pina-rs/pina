//! Adversarial tests for the 2026-09-21 security audit.
//!
//! Two framework-level findings are pinned here as executable proofs:
//!
//! 1. **Fixed-account PDA loaders verify only the stored bump.** A shadow
//!    account created at a noncanonical bump (whose stored bump field matches
//!    that noncanonical bump) passes `load_pda` / `load_pda_mut` byte for
//!    byte. The compact family grew `with_checked_pda` (canonical search,
//!    shadow-rejecting) in the 2026-09-18 remediation; the fixed-account
//!    family has no canonical loader at all, so a program using `load_pda`
//!    for a schema whose seeds do not bind a required signer accepts the
//!    shadow and the canonical account as the same logical entity.
//!
//! 2. **Discriminators are author-chosen integers with no cross-enum
//!    uniqueness.** Two account enums may declare the same value; when two
//!    account types behind different enums share the discriminator *and* the
//!    serialized size, `as_account::<T>` accepts either account for both
//!    types — the sealevel-attacks "type cosplay" class, undetectable by the
//!    within-enum duplicate check rustc performs.

#![allow(unsafe_code)]

use pina::*;
use pinocchio::account::NOT_BORROWED;
use pinocchio::account::RuntimeAccount;

const OWNER: Address = Address::new_from_array([9; 32]);

// ---------------------------------------------------------------------------
// Finding 1: fixed-account stored-bump loaders accept a noncanonical shadow
// ---------------------------------------------------------------------------

#[discriminator(crate = ::pina)]
pub enum AuditFixedKind {
	EscrowLike = 21,
}

#[account(crate = ::pina, discriminator = AuditFixedKind, variant = EscrowLike)]
#[pda(crate = ::pina, seeds = [b"audit-escrow", owner: Address], bump = bump)]
pub struct EscrowLike {
	pub owner: Address,
	pub bump: u8,
	pub amount: u64,
}

/// Find an owner key whose `audit-escrow` seeds admit a valid noncanonical
/// bump, returning `(owner, canonical, canonical_bump, shadow, shadow_bump)`.
fn find_shadow_fixture() -> (Address, Address, u8, Address, u8) {
	for byte in 1u8..=255 {
		let owner = Address::new_from_array([byte; 32]);
		let seeds: &[&[u8]] = &[b"audit-escrow", owner.as_ref()];
		let Some((canonical, canonical_bump)) = try_find_program_address(seeds, &OWNER) else {
			continue;
		};

		for shadow_bump in 0..canonical_bump {
			let bump_seed = [shadow_bump];
			let with_bump: &[&[u8]] = &[b"audit-escrow", owner.as_ref(), &bump_seed];
			if let Ok(shadow) = create_program_address(with_bump, &OWNER) {
				if shadow != canonical {
					return (owner, canonical, canonical_bump, shadow, shadow_bump);
				}
			}
		}
	}

	panic!("no noncanonical shadow fixture found for audit-escrow seeds");
}

fn escrow_bytes(owner: &Address, bump: u8, amount: u64) -> [u8; EscrowLike::SIZE] {
	let mut bytes = [0u8; EscrowLike::SIZE];
	EscrowLike::initialize(&mut bytes, |state| {
		state.owner = *owner;
		state.bump = bump;
		state.amount.set(amount);
		Ok(())
	})
	.unwrap_or_else(|error| panic!("escrow fixture must initialize: {error:?}"));
	bytes
}

#[repr(C)]
struct FixedTestAccount<const N: usize> {
	header: RuntimeAccount,
	data: [u8; N],
}

impl<const N: usize> FixedTestAccount<N> {
	fn new(address: Address, data: [u8; N]) -> Self {
		Self {
			header: RuntimeAccount {
				borrow_state: NOT_BORROWED,
				is_signer: 0,
				is_writable: 1,
				executable: 0,
				padding: [0; 4],
				address,
				owner: OWNER,
				lamports: 1,
				data_len: N as u64,
			},
			data,
		}
	}

	fn view(&mut self) -> AccountView {
		unsafe { AccountView::new_unchecked(core::ptr::addr_of_mut!(self.header)) }
	}
}

#[test]
fn fixed_account_load_pda_accepts_a_noncanonical_shadow_pda() {
	let (owner, canonical, canonical_bump, shadow, shadow_bump) = find_shadow_fixture();
	assert_ne!(canonical, shadow, "fixture must be a real shadow address");

	// The shadow account stores its own (noncanonical) bump, exactly as
	// `CreateProgramAccountWithUncheckedBump` would write it.
	let mut shadow_account =
		FixedTestAccount::new(shadow, escrow_bytes(&owner, shadow_bump, 1_000));
	let view = shadow_account.view();

	let state = EscrowLike::load_pda(&view, &owner, &OWNER)
		.expect("BUG PROVEN: load_pda accepts the noncanonical shadow PDA");

	assert_eq!(state.bump, shadow_bump);
	assert_eq!(state.amount.get(), 1_000);
}

#[test]
fn fixed_account_shadow_and_canonical_load_through_the_same_call() {
	// The ambiguity, stated concretely: both addresses hold valid accounts
	// for the same seeds, and `load_pda` accepts either one. A handler that
	// loads whichever account the client supplied has no way to tell that
	// only the canonical address is the one creation builders would produce.
	let (owner, canonical, canonical_bump, shadow, shadow_bump) = find_shadow_fixture();

	let mut canonical_account =
		FixedTestAccount::new(canonical, escrow_bytes(&owner, canonical_bump, 7));
	let canonical_view = canonical_account.view();
	let canonical_state =
		EscrowLike::load_pda(&canonical_view, &owner, &OWNER).expect("canonical PDA must load");
	assert_eq!(canonical_state.amount.get(), 7);

	let mut shadow_account = FixedTestAccount::new(shadow, escrow_bytes(&owner, shadow_bump, 9));
	let shadow_view = shadow_account.view();
	let shadow_state =
		EscrowLike::load_pda(&shadow_view, &owner, &OWNER).expect("shadow PDA must also load");
	assert_eq!(shadow_state.amount.get(), 9);

	// The canonical search the fixed-account family could not perform before
	// `load_checked_pda`: the two live accounts disagree on which address the
	// seeds name.
	let (searched, searched_bump) =
		try_find_program_address(&[b"audit-escrow", owner.as_ref()], &OWNER)
			.expect("canonical address must exist");
	assert_eq!(searched, canonical);
	assert_eq!(searched_bump, canonical_bump);
	assert_ne!(shadow_bump, canonical_bump);
}

#[test]
fn fixed_account_checked_loader_rejects_the_shadow_pda() {
	// The remediation for the stored-bump gap: `load_checked_pda` /
	// `load_checked_pda_mut` search for the canonical bump and reject both a
	// shadow address and a stored bump that is not canonical. The same
	// fixture `load_pda` accepts fails here.
	let (owner, canonical, canonical_bump, shadow, shadow_bump) = find_shadow_fixture();

	let mut shadow_account =
		FixedTestAccount::new(shadow, escrow_bytes(&owner, shadow_bump, 1_000));
	let shadow_view = shadow_account.view();
	assert!(
		EscrowLike::load_checked_pda(&shadow_view, &owner, &OWNER).is_err(),
		"the checked loader must reject the noncanonical shadow PDA"
	);

	let mut canonical_account =
		FixedTestAccount::new(canonical, escrow_bytes(&owner, canonical_bump, 5));
	let canonical_view = canonical_account.view();
	let state = EscrowLike::load_checked_pda(&canonical_view, &owner, &OWNER)
		.expect("the canonical PDA must load through the checked loader");
	assert_eq!(state.amount.get(), 5);
}

// ---------------------------------------------------------------------------
// Finding 2: cross-enum discriminator collisions permit type cosplay
// ---------------------------------------------------------------------------

#[discriminator(crate = ::pina)]
pub enum VaultKind {
	Vault = 31,
}

#[discriminator(crate = ::pina)]
pub enum RegistryKind {
	Registry = 31,
}

/// A value-bearing account whose discriminator comes from `VaultKind`.
#[account(crate = ::pina, discriminator = VaultKind, variant = Vault)]
pub struct VaultLedger {
	pub authority: Address,
	pub amount: u64,
}

/// A privileged account whose discriminator comes from a *different* enum
/// that reuses the same numeric value. Nothing relates the two enums, and no
/// cross-enum uniqueness check exists in `pina_macros` or `pina_lints`.
#[account(crate = ::pina, discriminator = RegistryKind, variant = Registry)]
pub struct AdminRegistry {
	pub admin: Address,
	pub nonce: u64,
}

#[test]
fn cross_enum_discriminator_collision_permits_type_cosplay() {
	// Build a legitimate `VaultLedger` account: owner-checked, discriminator
	// checked, exact size checked — every guard the typed loader provides.
	let attacker = Address::new_from_array([0xAB; 32]);
	let mut bytes = [0u8; VaultLedger::SIZE];
	VaultLedger::initialize(&mut bytes, |state| {
		state.authority = attacker;
		state.amount.set(500);
		Ok(())
	})
	.unwrap_or_else(|error| panic!("vault fixture must initialize: {error:?}"));

	let mut account = FixedTestAccount::new(Address::new_from_array([5; 32]), bytes);

	// The same bytes deserialize as the privileged `AdminRegistry`: the
	// discriminator value matches (31) and so does the size (1 + 32 + 8). A
	// handler calling `as_account::<AdminRegistry>` now reads the attacker's
	// key as `admin`.
	let view = account.view();
	let registry = view
		.as_account::<AdminRegistry>(&OWNER)
		.expect("BUG PROVEN: cross-enum discriminator collision accepts vault bytes");

	assert_eq!(registry.admin, attacker);
	assert_eq!(registry.nonce.get(), 500);
}

#[test]
fn different_size_discriminator_collision_is_rejected_by_the_length_check() {
	// Control: when the colliding layouts differ in size, the exact-size
	// check catches the substitution. The cosplay above only needs the two
	// types to agree on width — which same-shaped business types often do.
	let mut bytes = [0u8; VaultLedger::SIZE];
	VaultLedger::initialize(&mut bytes, |state| {
		state.authority = Address::default();
		state.amount.set(1);
		Ok(())
	})
	.unwrap();
	let mut account = FixedTestAccount::new(Address::new_from_array([6; 32]), bytes);

	// `TinyMarker` shares discriminator 31 through a third enum but is one
	// byte wide, so the loader rejects the vault bytes on length.
	let view = account.view();
	let marker = view.as_account::<TinyMarker>(&OWNER);
	assert!(
		marker.is_err(),
		"a different-size collision must not type-cosplay"
	);
}

#[discriminator(crate = ::pina)]
pub enum MarkerKind {
	Marker = 31,
}

#[account(crate = ::pina, discriminator = MarkerKind, variant = Marker)]
pub struct TinyMarker {
	pub flag: u8,
}
