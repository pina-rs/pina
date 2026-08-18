//! Compact account generation for the `#[account(compact)]` attribute.
//!
//! A compact account stores a fixed-size header (discriminator + inline pod
//! fields) followed by variable-length tail segments (strings, vecs, options)
//! in the account buffer. The on-chain size is `HEADER_SIZE + used tail
//! bytes`, so rent is paid only for the data actually stored.
//!
//! The generated code:
//! - `{Name}Header` — the fixed-size header struct (`#[repr(C)]`, `Pod`).
//! - `impl CompactAccount` — `validate` / `header` / `header_mut`.
//! - `{Name}Ref` / `{Name}RefMut` — validated views with tail accessors.
//! - `to_compact_bytes` / `compact_size` — compact serialization.

use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote;
use syn::Fields;
use syn::GenericArgument;
use syn::Ident;
use syn::ItemStruct;
use syn::PathArguments;
use syn::Type;
use syn::punctuated::Punctuated;
use syn::token::Comma;

/// A tail field classified from the struct fields.
enum TailKind {
	/// `PodString<N, PFX>` — length-prefixed UTF-8.
	String { max: usize, pfx: usize },
	/// `PodVec<T, N, PFX>` — length-prefixed element array.
	Vec { elem: Type, max: usize, pfx: usize },
	/// `Option<PodString<N, PFX>>` — tag byte + length-prefixed UTF-8.
	OptionString { max: usize, pfx: usize },
	/// `Option<PodVec<T, N, PFX>>` — tag byte + length-prefixed elements.
	OptionVec { elem: Type, max: usize, pfx: usize },
}

struct TailField {
	name: Ident,
	kind: TailKind,
}

/// Extracts the last path segment ident from a type, e.g. `PodString` from
/// `pina::PodString<32>`.
fn last_segment_ident(ty: &Type) -> Option<&Ident> {
	match ty {
		Type::Path(type_path) => type_path.path.segments.last().map(|s| &s.ident),
		_ => None,
	}
}

/// Extracts the generic arguments of a type, e.g. `[32]` from `PodString<32>`.
fn angle_args(ty: &Type) -> Option<&Punctuated<GenericArgument, Comma>> {
	match ty {
		Type::Path(type_path) => {
			match &type_path.path.segments.last()?.arguments {
				PathArguments::AngleBracketed(args) => Some(&args.args),
				_ => None,
			}
		}
		_ => None,
	}
}

/// Extracts a const generic argument as a `usize` value.
fn extract_const(arg: &GenericArgument) -> Option<usize> {
	match arg {
		GenericArgument::Const(expr) => {
			match expr {
				syn::Expr::Lit(lit) => {
					match &lit.lit {
						syn::Lit::Int(int) => int.base10_parse::<usize>().ok(),
						_ => None,
					}
				}
				_ => None,
			}
		}
		_ => None,
	}
}

/// Classifies a field type as a tail segment, or `None` if it is inline.
fn classify_tail(ty: &Type) -> Option<TailKind> {
	// `Option<...>` / `PodOption<...>` wrapping a string or vec.
	let outer = last_segment_ident(ty)?.to_string();
	if outer == "Option" || outer == "PodOption" {
		let args = angle_args(ty)?;
		let inner = match args.first()? {
			GenericArgument::Type(t) => t,
			_ => return None,
		};
		if let Some(kind) = classify_string(inner) {
			return Some(match kind {
				TailKind::String { max, pfx } => TailKind::OptionString { max, pfx },
				_ => return None,
			});
		}
		if let Some(kind) = classify_vec(inner) {
			return Some(match kind {
				TailKind::Vec { elem, max, pfx } => TailKind::OptionVec { elem, max, pfx },
				_ => return None,
			});
		}
		return None;
	}

	classify_string(ty).or_else(|| classify_vec(ty))
}

fn classify_string(ty: &Type) -> Option<TailKind> {
	let ident = last_segment_ident(ty)?;
	if ident != "String" && ident != "PodString" {
		return None;
	}
	let args = angle_args(ty)?;
	let mut iter = args.iter();
	let max = extract_const(iter.next()?)?;
	let pfx = iter.next().and_then(extract_const).unwrap_or(1);
	Some(TailKind::String { max, pfx })
}

fn classify_vec(ty: &Type) -> Option<TailKind> {
	let ident = last_segment_ident(ty)?;
	if ident != "Vec" && ident != "PodVec" {
		return None;
	}
	let args = angle_args(ty)?;
	let mut iter = args.iter();
	let elem = match iter.next()? {
		GenericArgument::Type(t) => t.clone(),
		_ => return None,
	};
	let max = extract_const(iter.next()?)?;
	let pfx = iter.next().and_then(extract_const).unwrap_or(2);
	Some(TailKind::Vec { elem, max, pfx })
}

/// Generates the compact account impls for a `#[account(compact)]` struct.
///
/// `item_struct` must already have the discriminator field injected as the
/// first field and the `#[repr(C)]` / derive attributes applied.
pub(crate) fn compact_account_impl(
	crate_path: &syn::Path,
	item_struct: &ItemStruct,
) -> Result<TokenStream, syn::Error> {
	let struct_name = &item_struct.ident;
	let header_name = format_ident!("{}Header", struct_name);
	let ref_name = format_ident!("{}Ref", struct_name);
	let ref_mut_name = format_ident!("{}RefMut", struct_name);

	let Fields::Named(named_fields) = &item_struct.fields else {
		return Err(syn::Error::new_spanned(
			item_struct,
			"Compact account structs must have named fields",
		));
	};

	// Classify fields: the discriminator + pod fields are inline; collection
	// types are tail segments. Enforce the suffix-only rule.
	let mut inline_fields = Vec::new();
	let mut tail_fields = Vec::new();
	let mut seen_tail = false;
	let mut first_tail_name: Option<&Ident> = None;

	for field in &named_fields.named {
		let name = field.ident.as_ref().expect("named field must have ident");
		let ty = &field.ty;

		if let Some(kind) = classify_tail(ty) {
			if !seen_tail {
				seen_tail = true;
				first_tail_name = Some(name);
			}
			tail_fields.push(TailField {
				name: name.clone(),
				kind,
			});
		} else {
			if seen_tail {
				let tail_name = first_tail_name.unwrap();
				return Err(syn::Error::new_spanned(
					field,
					format!(
						"inline field `{name}` cannot come after tail field `{tail_name}` in \
						 compact mode; tail fields must be a suffix of the struct"
					),
				));
			}
			inline_fields.push(field);
		}
	}

	// Inline field names after the discriminator (used for header serialization).
	let inline_names: Vec<syn::Ident> = inline_fields
		.iter()
		.skip(1)
		.map(|f| f.ident.clone().expect("named field must have ident"))
		.collect();

	// The discriminator must be the first inline field.
	let header_fields = inline_fields.iter().map(|field| {
		let name = field.ident.as_ref().unwrap();
		let ty = &field.ty;
		let vis = &field.vis;
		quote! { #vis #name: #ty }
	});

	let header_derives = quote! {
		#[derive(
			::core::clone::Clone,
			::core::marker::Copy,
			::core::cmp::PartialEq,
			::core::cmp::Eq,
			::core::fmt::Debug,
			#crate_path::Pod,
			#crate_path::Zeroable,
		)]
	};

	// ---- validate() tail segment checks ----
	let mut tail_checks = Vec::new();
	let mut offset_expr = quote! { Self::HEADER_SIZE };
	for tail in &tail_fields {
		let name = &tail.name;
		match &tail.kind {
			TailKind::String { max, pfx } => {
				let pfx = *pfx;
				let max = *max;
				let read_len = read_len_expr(&quote! { data }, &offset_expr, pfx);
				tail_checks.push(quote! {
					if #offset_expr + #pfx > data.len() {
						return Err(#crate_path::ProgramError::InvalidAccountData);
					}
					let #name = #read_len;
					if #name > #max {
						return Err(#crate_path::ProgramError::InvalidAccountData);
					}
					let __payload = #offset_expr + #pfx;
					if __payload + #name > data.len() {
						return Err(#crate_path::ProgramError::InvalidAccountData);
					}
					if ::core::str::from_utf8(&data[__payload..__payload + #name]).is_err() {
						return Err(#crate_path::ProgramError::InvalidAccountData);
					}
				});
				offset_expr = quote! { #offset_expr + #pfx + #name };
			}
			TailKind::Vec { elem, max, pfx } => {
				let pfx = *pfx;
				let max = *max;
				let read_len = read_len_expr(&quote! { data }, &offset_expr, pfx);
				tail_checks.push(quote! {
					if #offset_expr + #pfx > data.len() {
						return Err(#crate_path::ProgramError::InvalidAccountData);
					}
					let #name = #read_len;
					if #name > #max {
						return Err(#crate_path::ProgramError::InvalidAccountData);
					}
					let __payload = #offset_expr + #pfx;
					let __byte_len = #name * ::core::mem::size_of::<#elem>();
					if __payload + __byte_len > data.len() {
						return Err(#crate_path::ProgramError::InvalidAccountData);
					}
				});
				offset_expr =
					quote! { #offset_expr + #pfx + #name * ::core::mem::size_of::<#elem>() };
			}
			TailKind::OptionString { max, pfx } => {
				let pfx = *pfx;
				let max = *max;
				let read_len = read_len_expr(&quote! { data }, &quote! { #offset_expr + 1 }, pfx);
				tail_checks.push(quote! {
					if #offset_expr + 1 > data.len() {
						return Err(#crate_path::ProgramError::InvalidAccountData);
					}
					match data[#offset_expr] {
						0 => {}
						1 => {
							if #offset_expr + 1 + #pfx > data.len() {
								return Err(#crate_path::ProgramError::InvalidAccountData);
							}
							let #name = #read_len;
							if #name > #max {
								return Err(#crate_path::ProgramError::InvalidAccountData);
							}
							let __payload = #offset_expr + 1 + #pfx;
							if __payload + #name > data.len() {
								return Err(#crate_path::ProgramError::InvalidAccountData);
							}
							if ::core::str::from_utf8(&data[__payload..__payload + #name]).is_err() {
								return Err(#crate_path::ProgramError::InvalidAccountData);
							}
						}
						_ => return Err(#crate_path::ProgramError::InvalidAccountData),
					}
				});
				offset_expr = quote! { #offset_expr + 1 + #pfx + #name };
			}
			TailKind::OptionVec { elem, max, pfx } => {
				let pfx = *pfx;
				let max = *max;
				let read_len = read_len_expr(&quote! { data }, &quote! { #offset_expr + 1 }, pfx);
				tail_checks.push(quote! {
					if #offset_expr + 1 > data.len() {
						return Err(#crate_path::ProgramError::InvalidAccountData);
					}
					match data[#offset_expr] {
						0 => {}
						1 => {
							if #offset_expr + 1 + #pfx > data.len() {
								return Err(#crate_path::ProgramError::InvalidAccountData);
							}
							let #name = #read_len;
							if #name > #max {
								return Err(#crate_path::ProgramError::InvalidAccountData);
							}
							let __payload = #offset_expr + 1 + #pfx;
							let __byte_len = #name * ::core::mem::size_of::<#elem>();
							if __payload + __byte_len > data.len() {
								return Err(#crate_path::ProgramError::InvalidAccountData);
							}
						}
						_ => return Err(#crate_path::ProgramError::InvalidAccountData),
					}
				});
				offset_expr =
					quote! { #offset_expr + 1 + #pfx + #name * ::core::mem::size_of::<#elem>() };
			}
		}
	}

	// ---- accessor offset computation ----
	// Each accessor recomputes the offset of its segment by walking the
	// preceding segments. `new()` has already validated the buffer, so the
	// bounds are guaranteed; the accessors re-read length prefixes to locate
	// their segment.
	let accessors = generate_accessors(crate_path, struct_name, &header_name, &tail_fields, false);
	let accessors_mut =
		generate_accessors(crate_path, struct_name, &header_name, &tail_fields, true);

	// ---- to_compact_bytes / compact_size ----
	let mut serialize_segments = Vec::new();
	let mut size_segments = Vec::new();
	for tail in &tail_fields {
		let name = &tail.name;
		match &tail.kind {
			TailKind::String { pfx, .. } => {
				let pfx = *pfx;
				let write_len = write_len_expr(pfx);
				serialize_segments.push(quote! {
					{
						let __len = self.#name.len();
						#write_len
						out.extend_from_slice(self.#name.as_bytes());
					}
				});
				size_segments.push(quote! { #pfx + self.#name.len() });
			}
			TailKind::Vec { elem, pfx, .. } => {
				let pfx = *pfx;
				let write_len = write_len_expr(pfx);
				serialize_segments.push(quote! {
					{
						let __len = self.#name.len();
						#write_len
						for __i in 0..__len {
							out.extend_from_slice(#crate_path::bytemuck::bytes_of(&self.#name[__i]));
						}
					}
				});
				size_segments
					.push(quote! { #pfx + self.#name.len() * ::core::mem::size_of::<#elem>() });
			}
			TailKind::OptionString { pfx, .. } => {
				let pfx = *pfx;
				let write_len = write_len_expr(pfx);
				serialize_segments.push(quote! {
					{
						match self.#name.get() {
							Some(s) => {
								out.push(1);
								let __len = s.len();
								#write_len
								out.extend_from_slice(s.as_bytes());
							}
							None => out.push(0),
						}
					}
				});
				size_segments.push(quote! {
					1 + match self.#name.get() {
						Some(s) => #pfx + s.len(),
						None => 0,
					}
				});
			}
			TailKind::OptionVec { elem, pfx, .. } => {
				let pfx = *pfx;
				let write_len = write_len_expr(pfx);
				serialize_segments.push(quote! {
					{
						match self.#name.get() {
							Some(v) => {
								out.push(1);
								let __len = v.len();
								#write_len
								for __i in 0..__len {
									out.extend_from_slice(#crate_path::bytemuck::bytes_of(&v[__i]));
								}
							}
							None => out.push(0),
						}
					}
				});
				size_segments.push(quote! {
					1 + match self.#name.get() {
						Some(v) => #pfx + v.len() * ::core::mem::size_of::<#elem>(),
						None => 0,
					}
				});
			}
		}
	}

	Ok(quote! {
		// -------------------------------------------------------------------
		// Compact header
		// -------------------------------------------------------------------
		#header_derives
		#[repr(C)]
		pub struct #header_name {
			#(#header_fields),*
		}

		// -------------------------------------------------------------------
		// CompactAccount impl
		// -------------------------------------------------------------------
		impl #crate_path::CompactAccount for #struct_name {
			type Header = #header_name;
			const HEADER_SIZE: usize = ::core::mem::size_of::<#header_name>();

			fn validate(data: &[u8]) -> ::core::result::Result<(), #crate_path::ProgramError> {
				if !<Self as #crate_path::HasDiscriminator>::matches_discriminator(data) {
					return Err(#crate_path::ProgramError::InvalidAccountData);
				}
				if data.len() < Self::HEADER_SIZE {
					return Err(#crate_path::ProgramError::InvalidAccountData);
				}
				#(#tail_checks)*
				Ok(())
			}

			#[allow(unsafe_code)]
			fn header(data: &[u8]) -> ::core::result::Result<&#header_name, #crate_path::ProgramError> {
				Self::validate(data)?;
				// SAFETY: `validate` guarantees `data.len() >= HEADER_SIZE` and
				// the header is `#[repr(C)]` with align-1 pod fields.
				Ok(unsafe { &*(data.as_ptr() as *const #header_name) })
			}

			#[allow(unsafe_code)]
			fn header_mut(data: &mut [u8]) -> ::core::result::Result<&mut #header_name, #crate_path::ProgramError> {
				Self::validate(data)?;
				// SAFETY: `validate` guarantees `data.len() >= HEADER_SIZE` and
				// the header is `#[repr(C)]` with align-1 pod fields.
				Ok(unsafe { &mut *(data.as_mut_ptr() as *mut #header_name) })
			}
		}

		// -------------------------------------------------------------------
		// Compact serialization
		// -------------------------------------------------------------------
		impl #struct_name {
			/// Returns the byte size of the compact serialization.
			pub fn compact_size(&self) -> usize {
				::core::mem::size_of::<#header_name>() + #(#size_segments)+*
			}

			/// Serializes the account to its compact byte representation
			/// (header + only the used tail bytes).
			pub fn to_compact_bytes(&self) -> ::std::vec::Vec<u8> {
				let mut out = ::std::vec::Vec::with_capacity(self.compact_size());
				out.extend_from_slice(&self.discriminator);
				#(out.extend_from_slice(#crate_path::bytemuck::bytes_of(&self.#inline_names));)*
				#(#serialize_segments)*
				out
			}
		}

		// -------------------------------------------------------------------
		// Immutable view
		// -------------------------------------------------------------------
		pub struct #ref_name<'a> {
			data: &'a [u8],
		}

		impl<'a> #ref_name<'a> {
			/// Validates the account data and returns a view over it.
			pub fn new(data: &'a [u8]) -> ::core::result::Result<Self, #crate_path::ProgramError> {
				<#struct_name as #crate_path::CompactAccount>::validate(data)?;
				Ok(Self { data })
			}

			/// Returns the fixed-size header.
			#[allow(unsafe_code)]
			pub fn header(&self) -> &#header_name {
				// SAFETY: `new` validated the buffer; the header is
				// `#[repr(C)]` with align-1 pod fields.
				unsafe { &*(self.data.as_ptr() as *const #header_name) }
			}

			#(#accessors)*
		}

		// -------------------------------------------------------------------
		// Mutable view
		// -------------------------------------------------------------------
		pub struct #ref_mut_name<'a> {
			data: &'a mut [u8],
		}

		impl<'a> #ref_mut_name<'a> {
			/// Validates the account data and returns a mutable view over it.
			pub fn new(data: &'a mut [u8]) -> ::core::result::Result<Self, #crate_path::ProgramError> {
				<#struct_name as #crate_path::CompactAccount>::validate(data)?;
				Ok(Self { data })
			}

			/// Returns the fixed-size header.
			#[allow(unsafe_code)]
			pub fn header(&self) -> &#header_name {
				// SAFETY: `new` validated the buffer; the header is
				// `#[repr(C)]` with align-1 pod fields.
				unsafe { &*(self.data.as_ptr() as *const #header_name) }
			}

			/// Returns a mutable reference to the fixed-size header.
			#[allow(unsafe_code)]
			pub fn header_mut(&mut self) -> &mut #header_name {
				// SAFETY: `new` validated the buffer; the header is
				// `#[repr(C)]` with align-1 pod fields.
				unsafe { &mut *(self.data.as_mut_ptr() as *mut #header_name) }
			}

			#(#accessors_mut)*
		}
	})
}

/// Builds a length-prefix write expression for a given value and prefix size.
fn write_len_expr(pfx: usize) -> TokenStream {
	match pfx {
		1 => quote! { out.push(__len as u8); },
		2 => quote! { out.extend_from_slice(&(__len as u16).to_le_bytes()); },
		4 => quote! { out.extend_from_slice(&(__len as u32).to_le_bytes()); },
		8 => quote! { out.extend_from_slice(&(__len as u64).to_le_bytes()); },
		_ => unreachable!("pfx must be 1, 2, 4, or 8"),
	}
}

/// Builds a length-prefix read expression for a given data expression,
/// offset, and prefix size.
fn read_len_expr(data: &TokenStream, offset: &TokenStream, pfx: usize) -> TokenStream {
	match pfx {
		1 => quote! { #data[#offset] as usize },
		2 => {
			quote! {
				u16::from_le_bytes([#data[#offset], #data[#offset + 1]]) as usize
			}
		}
		4 => {
			quote! {
				u32::from_le_bytes([
					#data[#offset],
					#data[#offset + 1],
					#data[#offset + 2],
					#data[#offset + 3],
				]) as usize
			}
		}
		8 => {
			quote! {
				u64::from_le_bytes([
					#data[#offset],
					#data[#offset + 1],
					#data[#offset + 2],
					#data[#offset + 3],
					#data[#offset + 4],
					#data[#offset + 5],
					#data[#offset + 6],
					#data[#offset + 7],
				]) as usize
			}
		}
		_ => unreachable!("pfx must be 1, 2, 4, or 8"),
	}
}

/// Generates the tail accessor methods for a view struct.
fn generate_accessors(
	crate_path: &syn::Path,
	struct_name: &Ident,
	header_name: &Ident,
	tail_fields: &[TailField],
	is_mut: bool,
) -> Vec<TokenStream> {
	let mut accessors = Vec::new();
	let mut offset_expr = quote! { <#struct_name as #crate_path::CompactAccount>::HEADER_SIZE };

	for tail in tail_fields {
		let name = &tail.name;
		match &tail.kind {
			TailKind::String { pfx, .. } => {
				let pfx = *pfx;
				let read_len = read_len_expr(&quote! { self.data }, &offset_expr, pfx);
				accessors.push(quote! {
					/// Returns the string segment as `&str`.
					pub fn #name(&self) -> ::core::result::Result<&str, #crate_path::ProgramError> {
						let __len = #read_len;
						let __payload = #offset_expr + #pfx;
						::core::str::from_utf8(&self.data[__payload..__payload + __len])
							.map_err(|_| #crate_path::ProgramError::InvalidAccountData)
					}
				});
				offset_expr = quote! { #offset_expr + #pfx + #read_len };
			}
			TailKind::Vec { elem, pfx, .. } => {
				let pfx = *pfx;
				let read_len = read_len_expr(&quote! { self.data }, &offset_expr, pfx);
				accessors.push(quote! {
					/// Returns the element segment as a slice.
					#[allow(unsafe_code)]
					pub fn #name(&self) -> ::core::result::Result<&[#elem], #crate_path::ProgramError> {
						let __len = #read_len;
						let __payload = #offset_expr + #pfx;
						// SAFETY: `new` validated the buffer, so
						// `__payload + __len * size_of::<#elem>()` is in
						// bounds, and `#elem: Pod` makes every bit pattern valid.
						Ok(unsafe {
							::core::slice::from_raw_parts(
								self.data.as_ptr().add(__payload) as *const #elem,
								__len,
							)
						})
					}
				});
				offset_expr = quote! {
					#offset_expr + #pfx + #read_len * ::core::mem::size_of::<#elem>()
				};
			}
			TailKind::OptionString { pfx, .. } => {
				let pfx = *pfx;
				let read_len =
					read_len_expr(&quote! { self.data }, &quote! { #offset_expr + 1 }, pfx);
				accessors.push(quote! {
					/// Returns the optional string segment.
					pub fn #name(&self) -> ::core::result::Result<Option<&str>, #crate_path::ProgramError> {
						match self.data[#offset_expr] {
							0 => Ok(None),
							1 => {
								let __len = #read_len;
								let __payload = #offset_expr + 1 + #pfx;
								::core::str::from_utf8(&self.data[__payload..__payload + __len])
									.map(Some)
									.map_err(|_| #crate_path::ProgramError::InvalidAccountData)
							}
							_ => Err(#crate_path::ProgramError::InvalidAccountData),
						}
					}
				});
				offset_expr = quote! { #offset_expr + 1 + #pfx + #read_len };
			}
			TailKind::OptionVec { elem, pfx, .. } => {
				let pfx = *pfx;
				let read_len =
					read_len_expr(&quote! { self.data }, &quote! { #offset_expr + 1 }, pfx);
				accessors.push(quote! {
					/// Returns the optional element segment.
					#[allow(unsafe_code)]
					pub fn #name(&self) -> ::core::result::Result<Option<&[#elem]>, #crate_path::ProgramError> {
						match self.data[#offset_expr] {
							0 => Ok(None),
							1 => {
								let __len = #read_len;
								let __payload = #offset_expr + 1 + #pfx;
								// SAFETY: `new` validated the buffer, so the
								// segment is in bounds and `#elem: Pod` makes
								// every bit pattern valid.
								Ok(Some(unsafe {
									::core::slice::from_raw_parts(
										self.data.as_ptr().add(__payload) as *const #elem,
										__len,
									)
								}))
							}
							_ => Err(#crate_path::ProgramError::InvalidAccountData),
						}
					}
				});
				offset_expr = quote! {
					#offset_expr + 1 + #pfx + #read_len * ::core::mem::size_of::<#elem>()
				};
			}
		}
	}

	// Silence unused-variable warnings for the mutable view when there are no
	// tail fields.
	let _ = (is_mut, header_name);
	accessors
}
