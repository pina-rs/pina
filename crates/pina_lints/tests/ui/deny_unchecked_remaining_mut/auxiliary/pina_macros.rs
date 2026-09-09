// no-prefer-dynamic
// compile-flags: --emit=link

#![crate_type = "proc-macro"]

extern crate proc_macro;

use proc_macro::TokenStream;

#[proc_macro_derive(Accounts)]
pub fn accounts_derive(_input: TokenStream) -> TokenStream {
	"impl DerivedAccounts {
		fn parse(cursor: &mut pina::traits::AccountsCursor) -> Result<(), ()> {
			let _ = cursor.remaining_mut()?;
			Ok(())
		}
	}"
	.parse()
	.expect("the fixed Accounts expansion must parse")
}
