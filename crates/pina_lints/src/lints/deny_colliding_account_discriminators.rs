//! Deny colliding account discriminators across discriminator enums.

extern crate rustc_abi;
extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::BTreeMap;

use rustc_abi::Integer;
use rustc_abi::IntegerType;
use rustc_hir::Item;
use rustc_hir::ItemKind;
use rustc_hir::def_id::DefId;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_middle::mir::ConstValue;
use rustc_middle::ty::IntTy;
use rustc_middle::ty::Ty;
use rustc_middle::ty::TyKind;
use rustc_middle::ty::UintTy;
use rustc_span::Span;

use crate::collisions::Collision;
use crate::collisions::resolve_collisions;
use crate::diagnostics;

crate::impl_late_lint! {
	/// ### What it does
	///
	/// Rejects two account types whose `HasDiscriminator::VALUE` entries
	/// share the same numeric value (and discriminator width) through
	/// different discriminator enums.
	///
	/// ### Why is this bad?
	///
	/// Pina discriminators are author-chosen integers, and rustc only
	/// rejects duplicate values within one enum. Two account types behind
	/// different enums that agree on the value and the serialized size pass
	/// every typed loader check — owner, discriminator, exact size — so a
	/// handler calling `as_account::<T>` accepts either account for both
	/// types: the sealevel-attacks "type cosplay" class. Anchor-style
	/// hashed discriminators make this collision impossible; this lint is
	/// the integer-discriminator equivalent.
	///
	/// ### Example
	///
	/// ```ignore
	/// #[discriminator]
	/// pub enum VaultKind { Vault = 31 }
	/// #[discriminator]
	/// pub enum RegistryKind { Registry = 31 }
	///
	/// #[account(discriminator = VaultKind, variant = Vault)]
	/// pub struct VaultLedger { /* 32 + 8 bytes */ }
	/// #[account(discriminator = RegistryKind, variant = Registry)]
	/// pub struct AdminRegistry { /* same width */ }
	/// ```
	pub DENY_COLLIDING_ACCOUNT_DISCRIMINATORS,
	Deny,
	"two account types share a discriminator value, permitting type cosplay",
	DenyCollidingAccountDiscriminators
}

/// One `impl HasDiscriminator` observed in the crate.
struct Entry {
	span: Span,
	type_name: String,
	/// Discriminator width in bytes; only same-width values can collide,
	/// because the width is part of the account's serialized layout.
	width: u64,
	value: u128,
}

#[derive(Default)]
pub struct DenyCollidingAccountDiscriminators {
	entries: Vec<Entry>,
	/// Self types that implement pina's account traits. `#[event]` also
	/// generates a `HasDiscriminator` impl, and events live in a different
	/// namespace than account data, so only account types are compared.
	account_types: Vec<String>,
}

impl DenyCollidingAccountDiscriminators {
	fn record(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
		let ItemKind::Impl(impl_block) = &item.kind else {
			return;
		};
		if !impl_block.generics.params.is_empty() {
			return;
		}
		let Some(trait_ref) = &impl_block.of_trait else {
			return;
		};
		let Some(trait_def) = trait_ref.trait_ref.trait_def_id() else {
			return;
		};
		if cx.tcx.crate_name(trait_def.krate).as_str() != "pina" {
			return;
		}
		let trait_name = cx.tcx.item_name(trait_def).as_str().to_owned();
		if matches!(trait_name.as_str(), "PinaAccount" | "PinaCompactAccount") {
			let self_ty = cx
				.tcx
				.type_of(item.owner_id.def_id.to_def_id())
				.skip_binder();
			self.account_types.push(describe_type(cx, self_ty));
		}
		if trait_name != "HasDiscriminator" {
			return;
		}

		// The trait fixes the associated item names, so matching on them is
		// enough; the kind check adds nothing a hand-written impl could not
		// already fake.
		let Some(value_def) = assoc_item_def(cx, item, "VALUE") else {
			return;
		};
		let Some(type_def) = assoc_item_def(cx, item, "Type") else {
			return;
		};

		// The generated value is a discriminant-enum variant path; it is
		// always const-evaluable. Anything that is not a plain scalar (or a
		// width the layout query refuses) is skipped rather than guessed.
		let Ok(ConstValue::Scalar(scalar)) = cx.tcx.const_eval_poly(value_def) else {
			return;
		};
		let Some(int) = scalar.to_scalar_int().discard_err() else {
			return;
		};
		let value = int.to_bits(int.size());
		let discriminant_ty = cx.tcx.type_of(type_def).skip_binder();
		let Some(width) = scalar_width(discriminant_ty) else {
			return;
		};
		let self_ty = cx
			.tcx
			.type_of(item.owner_id.def_id.to_def_id())
			.skip_binder();
		let type_name = describe_type(cx, self_ty);

		self.entries.push(Entry {
			span: item.span,
			type_name,
			width,
			value,
		});
	}
}

fn assoc_item_def(cx: &LateContext<'_>, item: &Item<'_>, name: &str) -> Option<DefId> {
	let ItemKind::Impl(impl_block) = &item.kind else {
		return None;
	};
	for candidate in impl_block.items {
		let def_id = candidate.owner_id.def_id.to_def_id();
		let assoc = cx.tcx.associated_item(def_id);
		if assoc.name().as_str() == name {
			return Some(def_id);
		}
	}
	None
}

/// Describe a self type for the diagnostic message.
fn describe_type(cx: &LateContext<'_>, ty: Ty<'_>) -> String {
	match ty.kind() {
		TyKind::Adt(def, _) => cx.tcx.def_path_str(def.did()),
		_ => ty.to_string(),
	}
}

fn scalar_width(ty: Ty<'_>) -> Option<u64> {
	// Declared enums carry `#[repr(uN)]`, so the repr names the width;
	// hand-written impls name the primitive directly. Anything without a
	// known fixed width is skipped rather than guessed.
	match ty.kind() {
		TyKind::Adt(def, _) if def.is_enum() => {
			match def.repr().int? {
				IntegerType::Fixed(Integer::I8, _) => 1,
				IntegerType::Fixed(Integer::I16, _) => 2,
				IntegerType::Fixed(Integer::I32, _) => 4,
				IntegerType::Fixed(Integer::I64, _) => 8,
				IntegerType::Fixed(Integer::I128, _) => 16,
				IntegerType::Pointer(_) => return None,
			}
		}
		TyKind::Uint(UintTy::U8) | TyKind::Int(IntTy::I8) => 1,
		TyKind::Uint(UintTy::U16) | TyKind::Int(IntTy::I16) => 2,
		TyKind::Uint(UintTy::U32) | TyKind::Int(IntTy::I32) => 4,
		TyKind::Uint(UintTy::U64) | TyKind::Int(IntTy::I64) => 8,
		TyKind::Uint(UintTy::U128) | TyKind::Int(IntTy::I128) => 16,
		_ => return None,
	}
	.into()
}

impl<'tcx> LateLintPass<'tcx> for DenyCollidingAccountDiscriminators {
	fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
		self.record(cx, item);
	}

	fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
		let discriminators: Vec<(&str, u64, u128)> = self
			.entries
			.iter()
			.map(|entry| (entry.type_name.as_str(), entry.width, entry.value))
			.collect();
		let account_types: Vec<&str> = self.account_types.iter().map(String::as_str).collect();
		let collisions = resolve_collisions(&discriminators, &account_types);
		let spans: BTreeMap<&str, Span> = self
			.entries
			.iter()
			.map(|entry| (entry.type_name.as_str(), entry.span))
			.collect();

		for collision in collisions {
			let Some(span) = spans.get(collision.type_name) else {
				continue;
			};
			diagnostics::emit(cx, DENY_COLLIDING_ACCOUNT_DISCRIMINATORS, |diag| {
				diag.span(*span);
				diag.primary_message(format!(
					"account discriminator value {value} ({width} byte{plural}) is also claimed \
					 by {others}",
					value = collision.value,
					width = collision.width,
					others = collision.others,
					plural = if collision.width == 1 { "" } else { "s" },
				));
				diag.note(
					"two account types that share a discriminator value and a serialized width \
					 pass every typed loader check (owner, discriminator, exact size), so either \
					 account deserializes as the other: the type-cosplay class",
				);
				diag.help(
					"give every account type across every discriminator enum a unique value; one \
					 enum for all accounts of a program makes this impossible by construction",
				);
			});
		}
	}
}
