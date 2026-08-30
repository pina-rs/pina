// no-prefer-dynamic
// compile-flags: --emit=link

#![crate_type = "proc-macro"]

extern crate proc_macro;

use proc_macro::TokenStream;

#[proc_macro_derive(Accounts, attributes(pina))]
pub fn accounts(_input: TokenStream) -> TokenStream {
	TokenStream::new()
}
