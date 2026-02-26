use quote::quote;

use crate::account_impl;
use crate::accounts_derive_impl;
use crate::discriminator_impl;
use crate::error_impl;
use crate::event_impl;
use crate::instruction_impl;

/// Format a `proc_macro2::TokenStream` into a readable Rust string using
/// `prettyplease`.
fn pretty(tokens: proc_macro2::TokenStream) -> String {
	let file =
		syn::parse2(tokens).unwrap_or_else(|e| panic!("generated tokens are not valid Rust: {e}"));
	prettyplease::unparse(&file)
}

// ---------------------------------------------------------------------------
// #[discriminator] snapshots
// ---------------------------------------------------------------------------

#[test]
fn discriminator_u8_default() {
	let args = quote! {};
	let input = quote! {
		#[derive(Debug)]
		pub enum MyDiscriminator {
			First = 0,
			Second = 1,
			Third = 2,
		}
	};
	let output = pretty(discriminator_impl(args, input));
	insta::assert_snapshot!("discriminator_u8_default", output);
}

#[test]
fn discriminator_u16_primitive() {
	let args = quote! { primitive = u16, crate = ::pina };
	let input = quote! {
		pub enum WideDiscriminator {
			Alpha = 100,
			Beta = 200,
		}
	};
	let output = pretty(discriminator_impl(args, input));
	insta::assert_snapshot!("discriminator_u16_primitive", output);
}

#[test]
fn discriminator_u32_primitive() {
	let args = quote! { primitive = u32, crate = ::pina };
	let input = quote! {
		pub enum U32Discriminator {
			A = 0,
			B = 1000,
			C = 2000,
		}
	};
	let output = pretty(discriminator_impl(args, input));
	insta::assert_snapshot!("discriminator_u32_primitive", output);
}

#[test]
fn discriminator_u64_primitive() {
	let args = quote! { primitive = u64, crate = ::pina };
	let input = quote! {
		pub enum HugeDiscriminator {
			Mint = 0,
			Transfer = 1,
		}
	};
	let output = pretty(discriminator_impl(args, input));
	insta::assert_snapshot!("discriminator_u64_primitive", output);
}

#[test]
fn discriminator_final_attribute() {
	let args = quote! { primitive = u8, crate = ::pina, final };
	let input = quote! {
		#[derive(Debug)]
		pub enum FinalDiscriminator {
			Only = 0,
		}
	};
	let output = pretty(discriminator_impl(args, input));
	insta::assert_snapshot!("discriminator_final_attribute", output);
}

#[test]
fn discriminator_single_variant() {
	let args = quote! { crate = ::pina };
	let input = quote! {
		pub enum SingleVariant {
			Singleton = 42,
		}
	};
	let output = pretty(discriminator_impl(args, input));
	insta::assert_snapshot!("discriminator_single_variant", output);
}

#[test]
fn discriminator_many_variants() {
	let args = quote! { primitive = u16, crate = ::pina };
	let input = quote! {
		#[derive(Debug)]
		pub enum ManyVariants {
			Create = 0,
			Read = 1,
			Update = 2,
			Delete = 3,
			List = 4,
			Search = 5,
			Export = 6,
			Import = 7,
		}
	};
	let output = pretty(discriminator_impl(args, input));
	insta::assert_snapshot!("discriminator_many_variants", output);
}

// ---------------------------------------------------------------------------
// #[error] snapshots
// ---------------------------------------------------------------------------

#[test]
fn error_basic() {
	let args = quote! { crate = ::pina };
	let input = quote! {
		#[derive(Debug, Clone, Copy, PartialEq, Eq)]
		pub enum MyError {
			Invalid = 0,
			Duplicate = 1,
		}
	};
	let output = pretty(error_impl(args, input));
	insta::assert_snapshot!("error_basic", output);
}

#[test]
fn error_final() {
	let args = quote! { crate = ::pina, final };
	let input = quote! {
		#[derive(Debug)]
		pub enum FinalError {
			Unauthorized = 0,
		}
	};
	let output = pretty(error_impl(args, input));
	insta::assert_snapshot!("error_final", output);
}

#[test]
fn error_many_variants() {
	let args = quote! { crate = ::pina };
	let input = quote! {
		#[derive(Debug)]
		pub enum DetailedError {
			/// Not enough funds to complete the transaction.
			InsufficientFunds = 0,
			/// The account has already been initialized.
			AlreadyInitialized = 1,
			/// The provided authority does not match.
			InvalidAuthority = 2,
			/// The mint does not match.
			InvalidMint = 3,
			/// Arithmetic overflow occurred.
			Overflow = 4,
		}
	};
	let output = pretty(error_impl(args, input));
	insta::assert_snapshot!("error_many_variants", output);
}

#[test]
fn error_default_crate_path() {
	let args = quote! {};
	let input = quote! {
		pub enum DefaultCrateError {
			Something = 0,
		}
	};
	let output = pretty(error_impl(args, input));
	insta::assert_snapshot!("error_default_crate_path", output);
}

// ---------------------------------------------------------------------------
// #[account] snapshots
// ---------------------------------------------------------------------------

#[test]
fn account_basic() {
	let args = quote! { crate = ::pina, discriminator = MyAccount };
	let input = quote! {
		pub struct ConfigState {
			pub version: u8,
			pub bump: u8,
		}
	};
	let output = pretty(account_impl(args, input));
	insta::assert_snapshot!("account_basic", output);
}

#[test]
fn account_with_existing_derives() {
	let args = quote! { crate = ::pina, discriminator = MyAccount };
	let input = quote! {
		#[derive(Debug)]
		pub struct GameState {
			pub score: u8,
			pub level: u8,
		}
	};
	let output = pretty(account_impl(args, input));
	insta::assert_snapshot!("account_with_existing_derives", output);
}

#[test]
fn account_with_array_fields() {
	let args = quote! { crate = ::pina, discriminator = AccountDiscriminator };
	let input = quote! {
		pub struct DataAccount {
			pub authority: [u8; 32],
			pub data: [u8; 64],
			pub flags: [u8; 4],
		}
	};
	let output = pretty(account_impl(args, input));
	insta::assert_snapshot!("account_with_array_fields", output);
}

#[test]
fn account_with_pod_types() {
	let args = quote! { crate = ::pina, discriminator = MyDiscriminator };
	let input = quote! {
		pub struct BalanceAccount {
			pub owner: [u8; 32],
			pub amount: PodU64,
			pub decimals: u8,
			pub is_frozen: PodBool,
		}
	};
	let output = pretty(account_impl(args, input));
	insta::assert_snapshot!("account_with_pod_types", output);
}

#[test]
fn account_with_custom_variant() {
	let args = quote! { crate = ::pina, discriminator = AcctDisc, variant = Custom };
	let input = quote! {
		pub struct MyStruct {
			pub value: u8,
		}
	};
	let output = pretty(account_impl(args, input));
	insta::assert_snapshot!("account_with_custom_variant", output);
}

#[test]
fn account_many_fields() {
	let args = quote! { crate = ::pina, discriminator = MyAccount };
	let input = quote! {
		pub struct LargeState {
			pub authority: [u8; 32],
			pub bump: u8,
			pub treasury_bump: u8,
			pub mint_bump: u8,
			pub version: u8,
			pub padding: [u8; 3],
			pub total_supply: PodU64,
			pub name: [u8; 32],
		}
	};
	let output = pretty(account_impl(args, input));
	insta::assert_snapshot!("account_many_fields", output);
}

// ---------------------------------------------------------------------------
// #[instruction] snapshots
// ---------------------------------------------------------------------------

#[test]
fn instruction_minimal() {
	let args = quote! { crate = ::pina, discriminator = MyInstruction };
	let input = quote! {
		pub struct Initialize {}
	};
	let output = pretty(instruction_impl(args, input));
	insta::assert_snapshot!("instruction_minimal", output);
}

#[test]
fn instruction_many_fields() {
	let args = quote! { crate = ::pina, discriminator = MyInstruction };
	let input = quote! {
		pub struct FlipBit {
			pub section_index: u8,
			pub array_index: u8,
			pub offset: u8,
			pub value: u8,
		}
	};
	let output = pretty(instruction_impl(args, input));
	insta::assert_snapshot!("instruction_many_fields", output);
}

#[test]
fn instruction_with_existing_derive() {
	let args = quote! { crate = ::pina, discriminator = InstrDisc };
	let input = quote! {
		#[derive(Debug)]
		pub struct Transfer {
			pub amount: PodU64,
		}
	};
	let output = pretty(instruction_impl(args, input));
	insta::assert_snapshot!("instruction_with_existing_derive", output);
}

#[test]
fn instruction_with_custom_variant() {
	let args = quote! { crate = ::pina, discriminator = OpCode, variant = DoTransfer };
	let input = quote! {
		pub struct TransferData {
			pub amount: PodU64,
			pub destination: [u8; 32],
		}
	};
	let output = pretty(instruction_impl(args, input));
	insta::assert_snapshot!("instruction_with_custom_variant", output);
}

#[test]
fn instruction_with_array_and_pod() {
	let args = quote! { crate = ::pina, discriminator = MyInstruction };
	let input = quote! {
		pub struct ComplexInstruction {
			pub seed: [u8; 32],
			pub amount: PodU64,
			pub bump: u8,
			pub flags: [u8; 4],
		}
	};
	let output = pretty(instruction_impl(args, input));
	insta::assert_snapshot!("instruction_with_array_and_pod", output);
}

// ---------------------------------------------------------------------------
// #[event] snapshots
// ---------------------------------------------------------------------------

#[test]
fn event_basic() {
	let args = quote! { crate = ::pina, discriminator = EventDisc };
	let input = quote! {
		pub struct TransferEvent {
			pub from: [u8; 32],
			pub to: [u8; 32],
			pub amount: PodU64,
		}
	};
	let output = pretty(event_impl(args, input));
	insta::assert_snapshot!("event_basic", output);
}

#[test]
fn event_with_variant() {
	let args = quote! { crate = ::pina, discriminator = EventKind, variant = Init };
	let input = quote! {
		pub struct InitializeEvent {
			pub choice: u8,
		}
	};
	let output = pretty(event_impl(args, input));
	insta::assert_snapshot!("event_with_variant", output);
}

#[test]
fn event_minimal() {
	let args = quote! { crate = ::pina, discriminator = EventDisc };
	let input = quote! {
		pub struct EmptyEvent {}
	};
	let output = pretty(event_impl(args, input));
	insta::assert_snapshot!("event_minimal", output);
}

#[test]
fn event_with_existing_derive() {
	let args = quote! { crate = ::pina, discriminator = EvtDisc };
	let input = quote! {
		#[derive(Debug)]
		pub struct AuditEvent {
			pub action: u8,
			pub timestamp: PodU64,
		}
	};
	let output = pretty(event_impl(args, input));
	insta::assert_snapshot!("event_with_existing_derive", output);
}

// ---------------------------------------------------------------------------
// #[derive(Accounts)] snapshots
// ---------------------------------------------------------------------------

#[test]
fn accounts_derive_basic() {
	let input = quote! {
		#[pina(crate = ::pina)]
		pub struct InitAccounts<'a> {
			pub payer: &'a AccountView,
			pub config: &'a AccountView,
			pub system_program: &'a AccountView,
		}
	};
	let output = pretty(accounts_derive_impl(input));
	insta::assert_snapshot!("accounts_derive_basic", output);
}

#[test]
fn accounts_derive_with_remaining() {
	let input = quote! {
		#[pina(crate = ::pina)]
		pub struct TransferAccounts<'a> {
			pub authority: &'a AccountView,
			pub source: &'a AccountView,
			pub destination: &'a AccountView,
			#[pina(remaining)]
			pub extra: &'a [AccountView],
		}
	};
	let output = pretty(accounts_derive_impl(input));
	insta::assert_snapshot!("accounts_derive_with_remaining", output);
}

#[test]
fn accounts_derive_single_field() {
	let input = quote! {
		#[pina(crate = ::pina)]
		pub struct SingleAccount<'a> {
			pub account: &'a AccountView,
		}
	};
	let output = pretty(accounts_derive_impl(input));
	insta::assert_snapshot!("accounts_derive_single_field", output);
}

#[test]
fn accounts_derive_many_fields() {
	let input = quote! {
		#[pina(crate = ::pina)]
		pub struct EscrowAccounts<'a> {
			pub maker: &'a AccountView,
			pub escrow: &'a AccountView,
			pub mint_a: &'a AccountView,
			pub mint_b: &'a AccountView,
			pub maker_ata_a: &'a AccountView,
			pub vault: &'a AccountView,
			pub token_program: &'a AccountView,
			pub associated_token_program: &'a AccountView,
			pub system_program: &'a AccountView,
		}
	};
	let output = pretty(accounts_derive_impl(input));
	insta::assert_snapshot!("accounts_derive_many_fields", output);
}

#[test]
fn accounts_derive_default_crate() {
	let input = quote! {
		pub struct DefaultCrateAccounts<'a> {
			pub authority: &'a AccountView,
			pub data: &'a AccountView,
		}
	};
	let output = pretty(accounts_derive_impl(input));
	insta::assert_snapshot!("accounts_derive_default_crate", output);
}
