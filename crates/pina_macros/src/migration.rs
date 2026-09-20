//! Shared migration-manifest integration for data-schema macros.

use std::path::PathBuf;

use heck::ToSnakeCase as _;
use pina_abi::ContractHistory;
use pina_abi::ContractKind;
use pina_abi::DataSchema;
use pina_abi::LayoutKind;
use pina_abi::MANIFEST_PATH;
use pina_abi::MigrationManifest;
use pina_abi::MigrationVersionType;
use pina_abi::PhysicalLayout;
use pina_abi::TransitionMode;
use quote::format_ident;
use quote::quote;
use syn::ItemStruct;

/// Frozen migration metadata resolved before framework fields are injected.
pub(crate) struct MigrationExpansion {
	pub(crate) current_version: u32,
	pub(crate) version_type: MigrationVersionType,
	discriminator_bytes: u8,
	discriminator_value: u64,
	history: ContractHistory,
	/// `../` segments locating the program directory from the crate that
	/// expands this declaration.
	manifest_prefix: String,
}

#[derive(Clone, Copy)]
enum ImmutableContract {
	Instruction,
	Event,
}

impl MigrationExpansion {
	/// Load and verify the checked-in contract bound to `item`.
	pub(crate) fn load(
		item: &ItemStruct,
		kind: ContractKind,
		layout: LayoutKind,
		manifest: &MigrationManifest,
		program_dir: &std::path::Path,
		manifest_prefix: &str,
	) -> syn::Result<Self> {
		let history = manifest
			.contract_for_source(kind, &item.ident.to_string())
			.map_err(|error| syn::Error::new_spanned(item, error))?;
		let current = history.current().ok_or_else(|| {
			syn::Error::new_spanned(item, "migration contract contains no current schema")
		})?;
		let source_schema = pina_abi::data_schema(item, layout)
			.map_err(|error| syn::Error::new_spanned(item, error))?;
		verify_source_schema(item, &source_schema, &current.schema)?;
		verify_transition_files(item, program_dir, history)?;
		let current_version = history
			.current_version()
			.ok_or_else(|| syn::Error::new_spanned(item, "migration history has no versions"))?;
		let discriminator_bytes = history.identity.discriminator_bytes;
		let discriminator_value = history
			.identity
			.discriminator_value()
			.map_err(|error| syn::Error::new_spanned(item, error))?;
		Ok(Self {
			current_version,
			version_type: manifest.version_type,
			discriminator_bytes,
			discriminator_value,
			history: history.clone(),
			manifest_prefix: manifest_prefix.to_owned(),
		})
	}

	pub(crate) fn version_bytes(&self) -> usize {
		self.version_type.bytes()
	}

	pub(crate) fn version_type_tokens(&self) -> proc_macro2::TokenStream {
		version_type_tokens(self.version_type)
	}

	pub(crate) fn field(&self, patchable: bool) -> syn::Field {
		let bytes = self.version_bytes();
		let attribute = if patchable {
			quote!(#[pinapod(skip_accessor, skip_patch)])
		} else {
			quote!(#[pinapod(skip_accessor)])
		};
		syn::parse_quote! {
			#attribute
			migration_version: [u8; #bytes]
		}
	}

	pub(crate) fn implementation(
		&self,
		crate_path: &syn::Path,
		struct_name: &syn::Ident,
		discriminator: &syn::Path,
		variant: &syn::Ident,
	) -> proc_macro2::TokenStream {
		let version_type = self.version_type_tokens();
		let current = self.current_version;
		let discriminator_bytes = self.discriminator_bytes;
		let discriminator_value = self.discriminator_value;
		let manifest_path = proc_macro2::Literal::string(&format!(
			"/{}migrations/manifest.json",
			self.manifest_prefix
		));

		quote! {
			const _: &[u8] = include_bytes!(concat!(
				env!("CARGO_MANIFEST_DIR"),
				#manifest_path
			));

			const _: () = {
				::core::assert!(#discriminator::BYTES == #discriminator_bytes as usize);
				::core::assert!(#discriminator::#variant as u64 == #discriminator_value);
			};

			impl #crate_path::HasMigrationVersion for #struct_name {
				type Version = #version_type;

				const CURRENT_VERSION: Self::Version = #current as #version_type;
			}
		}
	}

	pub(crate) fn write_zc_version(&self) -> proc_macro2::TokenStream {
		let current = self.current_version;
		let version_type = self.version_type_tokens();
		quote! {
			value.migration_version = (#current as #version_type).to_le_bytes();
		}
	}

	pub(crate) fn require_current(crate_path: &syn::Path) -> proc_macro2::TokenStream {
		quote! {
			<Self as #crate_path::HasMigrationVersion>::require_current_migration_version(data)?;
		}
	}

	pub(crate) fn write_current(crate_path: &syn::Path) -> proc_macro2::TokenStream {
		quote! {
			<Self as #crate_path::HasMigrationVersion>::write_current_migration_version(data)?;
		}
	}

	/// Generate historical instruction normalization and a process helper.
	pub(crate) fn instruction_implementation(
		&self,
		crate_path: &syn::Path,
		struct_name: &syn::Ident,
	) -> syn::Result<proc_macro2::TokenStream> {
		self.immutable_implementation(crate_path, struct_name, ImmutableContract::Instruction)
	}

	/// Generate immutable historical event projection with source provenance.
	pub(crate) fn event_implementation(
		&self,
		crate_path: &syn::Path,
		struct_name: &syn::Ident,
	) -> syn::Result<proc_macro2::TokenStream> {
		self.immutable_implementation(crate_path, struct_name, ImmutableContract::Event)
	}

	fn immutable_implementation(
		&self,
		crate_path: &syn::Path,
		struct_name: &syn::Ident,
		contract: ImmutableContract,
	) -> syn::Result<proc_macro2::TokenStream> {
		let contract_label = match contract {
			ImmutableContract::Instruction => "instruction",
			ImmutableContract::Event => "event",
		};
		if self
			.history
			.versions
			.iter()
			.any(|version| version.schema.layout != LayoutKind::Fixed)
		{
			return Err(syn::Error::new_spanned(
				struct_name,
				format!("migration-aware {contract_label} history must use fixed layouts"),
			));
		}

		let header_size = usize::from(self.discriminator_bytes) + self.version_bytes();
		let versions = &self.history.versions;
		let current = self.current_version;
		let sizes = versions
			.iter()
			.map(|version| {
				version
					.schema
					.fixed_payload_size()
					.and_then(|size| header_size.checked_add(size))
			})
			.collect::<Option<Vec<_>>>()
			.ok_or_else(|| {
				syn::Error::new_spanned(
					struct_name,
					format!("{contract_label} migration size overflowed"),
				)
			})?;
		let current_size = *sizes.last().ok_or_else(|| {
			syn::Error::new_spanned(struct_name, format!("{contract_label} history is empty"))
		})?;
		let working_size = sizes.iter().copied().max().unwrap_or(current_size);
		let module_name = format_ident!(
			"__pina_{}_{}_migrations",
			struct_name.to_string().to_snake_case(),
			contract_label,
		);
		let transition_modules = versions.iter().enumerate().skip(1).map(|(to, version)| {
			version
				.transition
				.as_ref()
				.expect("validated immutable history has adjacent transitions");
			let from = to - 1;
			let name = format_ident!("v{}_to_v{}", from, to);
			// `transition_path` keys on the on-chain version numbers, which are
			// `u32`; the enumeration index is a `usize`.
			let (from, to) = (from as u32, to as u32);
			let relative = pina_abi::transition_path(&self.history.identity, from, to)
				.to_string_lossy()
				.replace('\\', "/");
			let include_path = format!("/{}{}", self.manifest_prefix, relative);

			quote! {
				pub(crate) mod #name {
					include!(concat!(env!("CARGO_MANIFEST_DIR"), #include_path));
				}
			}
		});
		let historical_structs = versions
			.iter()
			.enumerate()
			.take(versions.len().saturating_sub(1))
			.map(|(number, version)| {
				historical_struct(
					crate_path,
					struct_name,
					version,
					number as u32,
					self.discriminator_bytes,
					self.version_bytes(),
					&syn::Visibility::Inherited,
				)
			})
			.collect::<syn::Result<Vec<_>>>()?;
		let migration_arms = versions
			.iter()
			.enumerate()
			.take(versions.len().saturating_sub(1))
			.map(|(number, _version)| {
				let number = number as u32;
				let source_type = historical_struct_name(struct_name, number);
				if current - number > u32::from(MAX_INLINE_STEPS) {
					return quote! {
						#number => Err(#crate_path::PinaProgramError::MigrationRequired.into()),
					};
				}
				let calls = ((number + 1)..=current).map(|to| {
					let destination_version = &versions[to as usize];
					let transition = destination_version
						.transition
						.as_ref()
						.expect("validated history has adjacent transition");
					let transition_name = format_ident!("v{}_to_v{}", to - 1, to);
					let invoke = match transition.mode {
						TransitionMode::Automatic => {
							quote!(#module_name::#transition_name::migrate(workspace);)
						}
						TransitionMode::Manual => {
							quote! {
								if !#module_name::#transition_name::migrate(workspace) {
									return Err(#crate_path::ProgramError::InvalidInstructionData);
								}
							}
						}
					};
					let destination_size = sizes[to as usize];
					let validate = if to == current {
						quote! {
							<#struct_name as #crate_path::PinaPodFixed>::validate_exact(
								&workspace[..#destination_size],
							)
							.map_err(|_| #crate_path::ProgramError::InvalidInstructionData)?;
						}
					} else {
						let destination_type = historical_struct_name(struct_name, to);
						quote! {
							<#destination_type as #crate_path::PinaPodFixed>::validate_exact(
								&workspace[..#destination_size],
							)
							.map_err(|_| #crate_path::ProgramError::InvalidInstructionData)?;
						}
					};
					quote! {
						#invoke
						#validate
					}
				});
				quote! {
					#number => {
						<#source_type as #crate_path::PinaPodFixed>::validate_exact(data)
							.map_err(|_| #crate_path::ProgramError::InvalidInstructionData)?;
						workspace[..#working_size].fill(0);
						workspace[..data.len()].copy_from_slice(data);
						#(#calls)*
						Ok(())
					}
				}
			});
		let max_inline = current.min(u32::from(MAX_INLINE_STEPS)) as u16;
		let (trait_name, migrate_method, validate_method, consumer_impl) = match contract {
			ImmutableContract::Instruction => {
				let consumer_impl = quote! {
					impl #struct_name {
						/// Normalize historical instruction data and run a current-data handler.
						pub fn with_current_instruction_data<R>(
							data: &[u8],
							handler: impl FnOnce(&[u8]) -> Result<R, #crate_path::ProgramError>,
						) -> Result<R, #crate_path::ProgramError> {
							let mut workspace = [0_u8; #working_size];
							let current = #crate_path::normalize_instruction_data::<Self>(
								data,
								&mut workspace,
							)?;
							handler(current.as_bytes())
						}

						/// Normalize historical data before invoking the current account process.
						pub fn process_versioned<'accounts, A>(
							accounts: A,
							data: &[u8],
						) -> #crate_path::ProgramResult
						where
							A: #crate_path::ProcessAccountInfos<'accounts>,
						{
							Self::with_current_instruction_data(data, |current| {
								accounts.process(current)
							})
						}
					}
				};
				(
					format_ident!("MigratableInstruction"),
					format_ident!("migrate_stale_instruction"),
					format_ident!("validate_current_instruction"),
					consumer_impl,
				)
			}
			ImmutableContract::Event => {
				let consumer_impl = quote! {
					impl #struct_name {
						/// Project historical event bytes and preserve their source version.
						pub fn with_current_event_data<R>(
							data: &[u8],
							handler: impl FnOnce(
								&[u8],
								<Self as #crate_path::HasMigrationVersion>::Version,
							) -> Result<R, #crate_path::ProgramError>,
						) -> Result<R, #crate_path::ProgramError> {
							let mut workspace = [0_u8; #working_size];
							let current = #crate_path::normalize_event_data::<Self>(
								data,
								&mut workspace,
							)?;
							handler(current.as_bytes(), current.source_version())
						}
					}
				};
				(
					format_ident!("MigratableEvent"),
					format_ident!("migrate_stale_event"),
					format_ident!("validate_current_event"),
					consumer_impl,
				)
			}
		};

		Ok(quote! {
			#[allow(dead_code)]
			mod #module_name {
				#(#transition_modules)*
			}

			#(#historical_structs)*

			const _: () = assert!(
				#working_size <= #crate_path::MAX_MIGRATION_WORKSPACE,
				concat!(
					"migration workspace for ",
					stringify!(#struct_name),
					" exceeds the SBF stack budget; reduce the payload or field count",
				),
			);

			impl #crate_path::#trait_name for #struct_name {
				const CURRENT_SIZE: usize = #current_size;
				const WORKING_SIZE: usize = #working_size;
				const MAX_INLINE_STEPS: u16 = #max_inline;

				fn #migrate_method(
					data: &[u8],
					workspace: &mut [u8],
				) -> #crate_path::ProgramResult {
					if workspace.len() < #working_size
						|| !<Self as #crate_path::HasDiscriminator>::matches_discriminator(data)
					{
						return Err(#crate_path::ProgramError::InvalidInstructionData);
					}
					let stored =
						<Self as #crate_path::HasMigrationVersion>::read_migration_version(data)?;
					let stored = <<Self as #crate_path::HasMigrationVersion>::Version as #crate_path::MigrationVersion>::into_u32(stored);
					match stored {
						#(#migration_arms)*
						_ => Err(#crate_path::PinaProgramError::InvalidMigrationVersion.into()),
					}
				}

				fn #validate_method(data: &[u8]) -> #crate_path::ProgramResult {
					if data.len() != #current_size
						|| !<Self as #crate_path::HasDiscriminator>::matches_discriminator(data)
					{
						return Err(#crate_path::ProgramError::InvalidInstructionData);
					}
					<Self as #crate_path::HasMigrationVersion>::require_current_migration_version(data)?;
					<Self as #crate_path::PinaPodFixed>::validate_exact(data)
						.map_err(|_| #crate_path::ProgramError::InvalidInstructionData)
				}
			}

			#consumer_impl
		})
	}

	/// Generate an allocator-free in-place implementation for an account history.
	/// Manual transitions use the same preflighted, infallible runtime boundary
	/// as generated transitions.
	pub(crate) fn account_implementation(
		&self,
		crate_path: &syn::Path,
		struct_name: &syn::Ident,
		visibility: &syn::Visibility,
	) -> syn::Result<Option<proc_macro2::TokenStream>> {
		let versioned_view =
			self.versioned_view_implementation(crate_path, struct_name, visibility);

		if self
			.history
			.versions
			.iter()
			.any(|version| version.schema.layout != LayoutKind::Fixed)
		{
			let implementation =
				self.variable_account_implementation(crate_path, struct_name, visibility)?;

			return Ok(Some(quote! {
				#implementation
				#versioned_view
			}));
		}

		let header_size = usize::from(self.discriminator_bytes) + self.version_bytes();
		let versions = &self.history.versions;
		let current = self.current_version;
		let current_size = versions
			.last()
			.and_then(|version| version.schema.fixed_payload_size())
			.and_then(|size| header_size.checked_add(size))
			.ok_or_else(|| {
				syn::Error::new_spanned(struct_name, "current fixed migration size overflowed")
			})?;
		let sizes = versions
			.iter()
			.map(|version| {
				version
					.schema
					.fixed_payload_size()
					.and_then(|size| header_size.checked_add(size))
			})
			.collect::<Option<Vec<_>>>()
			.ok_or_else(|| {
				syn::Error::new_spanned(struct_name, "historical fixed migration size overflowed")
			})?;
		let module_name = format_ident!(
			"__pina_{}_account_migrations",
			struct_name.to_string().to_snake_case(),
		);
		let transition_modules = versions.iter().enumerate().skip(1).map(|(to, version)| {
			version
				.transition
				.as_ref()
				.expect("automatic history has every adjacent transition");
			let from = to - 1;
			let name = format_ident!("v{}_to_v{}", from, to);
			// `transition_path` keys on the on-chain version numbers, which are
			// `u32`; the enumeration index is a `usize`.
			let (from, to) = (from as u32, to as u32);
			let relative = pina_abi::transition_path(&self.history.identity, from, to)
				.to_string_lossy()
				.replace('\\', "/");
			let include_path = format!("/{}{}", self.manifest_prefix, relative);

			quote! {
				pub(crate) mod #name {
					include!(concat!(env!("CARGO_MANIFEST_DIR"), #include_path));
				}
			}
		});
		let historical_structs = versions
			.iter()
			.enumerate()
			.take(versions.len().saturating_sub(1))
			.map(|(number, version)| {
				historical_struct(
					crate_path,
					struct_name,
					version,
					number as u32,
					self.discriminator_bytes,
					self.version_bytes(),
					visibility,
				)
			})
			.collect::<syn::Result<Vec<_>>>()?;
		let planner_arms = versions
			.iter()
			.enumerate()
			.take(versions.len().saturating_sub(1))
			.map(|(number, _version)| {
				let number = number as u32;
				let destination = number + 1;
				let historical = historical_struct_name(struct_name, number);
				let destination_size = sizes[destination as usize];
				let transition_working_size = sizes[number as usize].max(destination_size);
				quote! {
					#number => {
						<#historical as #crate_path::PinaPodFixed>::read_exact(data)
							.map_err(|_| #crate_path::ProgramError::InvalidAccountData)?;
						#crate_path::AccountMigrationPlan::try_with_working_size(
							#number,
							#destination,
							#destination_size,
							#transition_working_size,
							1,
							#number,
						)
					}
				}
			});
		let apply_arms = versions
			.iter()
			.enumerate()
			.take(versions.len().saturating_sub(1))
			.map(|(number, _version)| {
				let number = number as u32;
				let transition_name = format_ident!("v{}_to_v{}", number, number + 1);
				quote! {
					#number => #module_name::#transition_name::migrate(destination),
				}
			});
		let historical_validation_arms = versions
			.iter()
			.enumerate()
			.skip(1)
			.take(versions.len().saturating_sub(2))
			.map(|(number, _version)| {
				let number = number as u32;
				let size = sizes[number as usize];
				let historical = historical_struct_name(struct_name, number);
				quote! {
					#number => {
						if data.len() != #size {
							return Err(#crate_path::ProgramError::InvalidAccountData);
						}
						<#historical as #crate_path::PinaPodFixed>::validate_exact(data)
							.map_err(|_| #crate_path::ProgramError::InvalidAccountData)
					}
				}
			});
		let max_inline = current.min(u32::from(MAX_INLINE_STEPS)) as u16;
		// Destination validation runs before the version marker is committed,
		// so the current arm must validate structurally without the migration
		// envelope check that `PinaAccount::validate_account_data` enforces.
		#[cfg(feature = "validation")]
		let current_destination_validation = quote! {
			#current => {
				if data.len() != #current_size {
					return Err(#crate_path::ProgramError::InvalidAccountData);
				}
				let value = <Self as #crate_path::PinaPodFixed>::read_exact(data)
					.map_err(|_| #crate_path::ProgramError::InvalidAccountData)?;
				<Self as #crate_path::PinaAccount>::validate_account_value(value)
			}
		};
		#[cfg(not(feature = "validation"))]
		let current_destination_validation = quote! {
			#current => {
				if data.len() != #current_size {
					return Err(#crate_path::ProgramError::InvalidAccountData);
				}
				<Self as #crate_path::PinaPodFixed>::validate_exact(data)
					.map_err(|_| #crate_path::ProgramError::InvalidAccountData)
			}
		};

		Ok(Some(quote! {
			#[allow(dead_code)]
			mod #module_name {
				#(#transition_modules)*
			}

			#(#historical_structs)*

			impl #crate_path::MigratableAccount for #struct_name {
				type Plan = u32;

				const MAX_INLINE_STEPS: u16 = #max_inline;

				fn plan_migration(
					data: &[u8],
				) -> Result<#crate_path::AccountMigrationPlan<Self::Plan>, #crate_path::ProgramError> {
					if !<Self as #crate_path::HasDiscriminator>::matches_discriminator(data) {
						return Err(#crate_path::ProgramError::InvalidAccountData);
					}
					let stored = <Self as #crate_path::HasMigrationVersion>::read_migration_version(data)?;
					let stored = <<Self as #crate_path::HasMigrationVersion>::Version as #crate_path::MigrationVersion>::into_u32(stored);
					match stored {
						#(#planner_arms)*
						_ => Err(#crate_path::PinaProgramError::InvalidMigrationVersion.into()),
					}
				}

				fn apply_migration(plan: Self::Plan, destination: &mut [u8]) {
					match plan {
						#(#apply_arms)*
						_ => {}
					}
				}

				fn validate_migration_destination(
					version: u32,
					data: &[u8],
				) -> #crate_path::ProgramResult {
					if !<Self as #crate_path::HasDiscriminator>::matches_discriminator(data) {
						return Err(#crate_path::ProgramError::InvalidAccountData);
					}
					match version {
						#(#historical_validation_arms)*
						#current_destination_validation
						_ => Err(#crate_path::PinaProgramError::InvalidMigrationVersion.into()),
					}
				}
			}

			#versioned_view
		}))
	}

	/// Generate the read-only, version-dispatched view of one account history.
	///
	/// Every arm validates exactly one stored representation through the same
	/// `PinaPod` reader the migration planner uses and borrows it immutably, so
	/// the view can never migrate, resize, or require a writable account.
	fn versioned_view_implementation(
		&self,
		crate_path: &syn::Path,
		struct_name: &syn::Ident,
		visibility: &syn::Visibility,
	) -> proc_macro2::TokenStream {
		let versions = &self.history.versions;
		let current = self.current_version;
		let version_type = self.version_type_tokens();
		let enum_name = versioned_enum_name(struct_name);
		let current_layout = versions
			.last()
			.map(|version| version.schema.layout)
			.expect("validated migration history has a current version");
		let error = quote!(#crate_path::ProgramError::InvalidAccountData);

		let mut variants = Vec::with_capacity(versions.len());
		let mut arms = Vec::with_capacity(versions.len());
		let mut version_arms = Vec::with_capacity(versions.len());

		for (number, version) in versions
			.iter()
			.take(versions.len().saturating_sub(1))
			.enumerate()
		{
			let number = number as u32;
			let variant = versioned_variant_name(number);
			let historical = historical_struct_name(struct_name, number);
			let (view, read) = match version.schema.layout {
				LayoutKind::Fixed => {
					let view = format_ident!("{}Zc", historical);
					(
						quote!(&'data #view),
						quote! {
							<#historical as #crate_path::PinaPodFixed>::read_exact(data)
								.map_err(|_| #error)?
						},
					)
				}
				LayoutKind::Compact => {
					let view = format_ident!("{}Ref", historical);
					(
						quote!(#view<'data>),
						quote! {
							#view::new(data).map_err(|_| #error)?
						},
					)
				}
			};
			let documentation = format!(
				"Account bytes stored at version {number}. Reading this variant does not migrate \
				 them."
			);
			variants.push(quote! {
				#[doc = #documentation]
				#variant(#view),
			});
			arms.push(quote! {
				#number => Ok(#enum_name::#variant(#read)),
			});
			version_arms.push(quote! {
				Self::#variant(_) => #number as #version_type,
			});
		}

		let current_view = match current_layout {
			LayoutKind::Fixed => {
				let view = format_ident!("{}Zc", struct_name);
				quote!(&'data #view)
			}
			LayoutKind::Compact => {
				let view = format_ident!("{}Ref", struct_name);
				quote!(#view<'data>)
			}
		};
		variants.push(quote! {
			/// Account bytes that already use the current representation.
			Current(#current_view),
		});
		arms.push(quote! {
			#current => Ok(#enum_name::Current(Self::try_from_bytes(data)?)),
		});
		version_arms.push(quote! {
			Self::Current(_) => <#struct_name as #crate_path::HasMigrationVersion>::CURRENT_VERSION,
		});

		quote! {
			/// A read-only view of stored account bytes, dispatched by the version envelope.
			///
			/// The view validates one exact generated representation and borrows the bytes
			/// immutably: it never rewrites, resizes, or clears the account, and it never
			/// requires a writable borrow.
			///
			/// Prefer this only for read-mostly accounts whose one-time writable touch is
			/// genuinely hard to schedule. A caller that reads historical layouts must
			/// handle every representation this enum exposes, which is the branching the
			/// migration system exists to remove; prefer migrating the account whenever a
			/// writable touch is schedulable.
			#[allow(dead_code)]
			#visibility enum #enum_name<'data> {
				#(#variants)*
			}

			impl #enum_name<'_> {
				/// Return the stored version whose representation this view borrows.
				#[must_use]
				pub fn version(&self) -> <#struct_name as #crate_path::HasMigrationVersion>::Version {
					match self {
						#(#version_arms)*
					}
				}
			}

			impl #struct_name {
				/// Validate and borrow stored bytes at the version their envelope names.
				///
				/// This accessor never mutates, resizes, or requires a writable borrow, so it
				/// reads a stale account the current transaction cannot write. Unknown
				/// versions, future versions, foreign discriminators, and malformed
				/// representations all fail closed.
				///
				/// # Errors
				///
				/// Returns `DataTooShort` when the bytes end inside the version envelope,
				/// `InvalidAccountData` when the discriminator is foreign or the bytes are not
				/// one exact representation of the stored version, and
				/// `InvalidMigrationVersion` when the stored version is unknown to this program
				/// or newer than its current schema.
				pub fn try_from_bytes_versioned(
					data: &[u8],
				) -> Result<#enum_name<'_>, #crate_path::ProgramError> {
					if !<Self as #crate_path::HasDiscriminator>::matches_discriminator(data) {
						return Err(#error);
					}
					let stored =
						<Self as #crate_path::HasMigrationVersion>::read_migration_version(data)?;
					let stored = <<Self as #crate_path::HasMigrationVersion>::Version as #crate_path::MigrationVersion>::into_u32(stored);

					match stored {
						#(#arms)*
						_ => Err(#crate_path::PinaProgramError::InvalidMigrationVersion.into()),
					}
				}
			}
		}
	}

	/// Generate one adjacent, allocator-free migration step at a time when any
	/// historical representation is compact. The executor repeats this contract
	/// atomically until it reaches the current version.
	fn variable_account_implementation(
		&self,
		crate_path: &syn::Path,
		struct_name: &syn::Ident,
		visibility: &syn::Visibility,
	) -> syn::Result<proc_macro2::TokenStream> {
		let versions = &self.history.versions;
		let current = self.current_version;
		let module_name = format_ident!(
			"__pina_{}_account_migrations",
			struct_name.to_string().to_snake_case(),
		);
		let transition_modules = versions.iter().enumerate().skip(1).map(|(to, version)| {
			version
				.transition
				.as_ref()
				.expect("validated account history has adjacent transitions");
			let from = to - 1;
			let name = format_ident!("v{}_to_v{}", from, to);
			// `transition_path` keys on the on-chain version numbers, which are
			// `u32`; the enumeration index is a `usize`.
			let (from, to) = (from as u32, to as u32);
			let relative = pina_abi::transition_path(&self.history.identity, from, to)
				.to_string_lossy()
				.replace('\\', "/");
			let include_path = format!("/{}{}", self.manifest_prefix, relative);

			quote! {
				pub(crate) mod #name {
					include!(concat!(env!("CARGO_MANIFEST_DIR"), #include_path));
				}
			}
		});
		let historical_structs = versions
			.iter()
			.enumerate()
			.take(versions.len().saturating_sub(1))
			.map(|(number, version)| {
				historical_struct(
					crate_path,
					struct_name,
					version,
					number as u32,
					self.discriminator_bytes,
					self.version_bytes(),
					visibility,
				)
			})
			.collect::<syn::Result<Vec<_>>>()?;

		let mut planner_arms = Vec::with_capacity(versions.len().saturating_sub(1));
		let mut apply_arms = Vec::with_capacity(versions.len().saturating_sub(1));
		for (index, source) in versions
			.iter()
			.take(versions.len().saturating_sub(1))
			.enumerate()
		{
			let destination = &versions[index + 1];
			let from = index as u32;
			let to = from + 1;
			let source_type = historical_struct_name(struct_name, from);
			let destination_type = (to != current).then(|| historical_struct_name(struct_name, to));
			let transition_name = format_ident!("v{from}_to_v{to}");
			let validate_source = match source.schema.layout {
				LayoutKind::Fixed => {
					quote! {
						<#source_type as #crate_path::PinaPodFixed>::read_exact(data)
							.map_err(|_| #crate_path::ProgramError::InvalidAccountData)?;
					}
				}
				LayoutKind::Compact => {
					quote! {
						<#source_type as #crate_path::PinaPodCompact>::validate(data)
							.map_err(|_| #crate_path::ProgramError::InvalidAccountData)?;
					}
				}
			};
			let dynamic = source.schema.layout == LayoutKind::Compact
				|| destination.schema.layout == LayoutKind::Compact;
			let sizes = if dynamic {
				quote! {
					let target_size = #module_name::#transition_name::target_size(data)
						.ok_or(#crate_path::PinaProgramError::MigrationUnavailable)?;
					let working_size = #module_name::#transition_name::working_size(
						data,
						target_size,
					)
					.ok_or(#crate_path::PinaProgramError::MigrationUnavailable)?;
				}
			} else {
				quote! {
					let target_size = #module_name::#transition_name::DESTINATION_SIZE;
					let working_size = #module_name::#transition_name::WORKING_SIZE;
				}
			};
			let validate_size = match (&destination.schema.layout, destination_type) {
				(LayoutKind::Fixed, None) => {
					quote! {
						if target_size
							!= ::core::mem::size_of::<<#struct_name as #crate_path::PinaPodFixed>::Zc>()
						{
							return Err(#crate_path::ProgramError::InvalidAccountData);
						}
					}
				}
				(LayoutKind::Fixed, Some(destination_type)) => {
					quote! {
						if target_size
							!= ::core::mem::size_of::<<#destination_type as #crate_path::PinaPodFixed>::Zc>()
						{
							return Err(#crate_path::ProgramError::InvalidAccountData);
						}
					}
				}
				(LayoutKind::Compact, None) => {
					quote! {
						<#struct_name as #crate_path::PinaPodCompact>::validate_storage_len(target_size)
							.map_err(|_| #crate_path::ProgramError::InvalidAccountData)?;
					}
				}
				(LayoutKind::Compact, Some(destination_type)) => {
					quote! {
						<#destination_type as #crate_path::PinaPodCompact>::validate_storage_len(target_size)
							.map_err(|_| #crate_path::ProgramError::InvalidAccountData)?;
					}
				}
			};
			planner_arms.push(quote! {
				#from => {
					#validate_source
					#sizes
					#validate_size
					#crate_path::AccountMigrationPlan::try_with_working_size(
						#from,
						#to,
						target_size,
						working_size,
						1,
						#from,
					)
				}
			});
			apply_arms.push(quote! {
				#from => #module_name::#transition_name::migrate(destination),
			});
		}

		let mut destination_validation_arms = Vec::with_capacity(versions.len().saturating_sub(1));
		for (number, version) in versions.iter().enumerate().skip(1) {
			let number = number as u32;
			let validation = if number == current {
				match version.schema.layout {
					LayoutKind::Fixed => {
						// Destination validation runs before the version marker
						// is committed, so validate structurally without the
						// migration envelope check that the generic account
						// validation path enforces.
						#[cfg(feature = "validation")]
						{
							quote! {
								if data.len() != ::core::mem::size_of::<
									<Self as #crate_path::PinaPodFixed>::Zc,
								>() {
									return Err(#crate_path::ProgramError::InvalidAccountData);
								}
								let value = <Self as #crate_path::PinaPodFixed>::read_exact(data)
									.map_err(|_| #crate_path::ProgramError::InvalidAccountData)?;
								<Self as #crate_path::PinaAccount>::validate_account_value(value)
							}
						}
						#[cfg(not(feature = "validation"))]
						{
							quote! {
								<Self as #crate_path::PinaPodFixed>::validate_exact(data)
									.map_err(|_| #crate_path::ProgramError::InvalidAccountData)
							}
						}
					}
					LayoutKind::Compact => {
						quote! {
							<Self as #crate_path::PinaPodCompact>::validate(data)
								.map_err(|_| #crate_path::ProgramError::InvalidAccountData)
						}
					}
				}
			} else {
				let historical = historical_struct_name(struct_name, number);
				match version.schema.layout {
					LayoutKind::Fixed => {
						quote! {
							<#historical as #crate_path::PinaPodFixed>::validate_exact(data)
								.map_err(|_| #crate_path::ProgramError::InvalidAccountData)
						}
					}
					LayoutKind::Compact => {
						quote! {
							<#historical as #crate_path::PinaPodCompact>::validate(data)
								.map_err(|_| #crate_path::ProgramError::InvalidAccountData)
						}
					}
				}
			};
			destination_validation_arms.push(quote! {
				#number => #validation,
			});
		}
		let validate_current = (versions
			.last()
			.is_some_and(|version| version.schema.layout == LayoutKind::Compact))
		.then(|| {
			quote! {
				fn validate_current_migration(data: &[u8]) -> #crate_path::ProgramResult {
					<Self as #crate_path::HasMigrationVersion>::require_current_migration_version(data)?;
					<Self as #crate_path::PinaCompactAccount>::validate_account_data(data)
				}
			}
		});
		let max_inline = current.min(u32::from(MAX_INLINE_STEPS)) as u16;

		Ok(quote! {
			#[allow(dead_code)]
			mod #module_name {
				#(#transition_modules)*
			}

			#(#historical_structs)*

			impl #crate_path::MigratableAccount for #struct_name {
				type Plan = u32;

				const MAX_INLINE_STEPS: u16 = #max_inline;

				fn plan_migration(
					data: &[u8],
				) -> Result<#crate_path::AccountMigrationPlan<Self::Plan>, #crate_path::ProgramError> {
					if !<Self as #crate_path::HasDiscriminator>::matches_discriminator(data) {
						return Err(#crate_path::ProgramError::InvalidAccountData);
					}
					let stored = <Self as #crate_path::HasMigrationVersion>::read_migration_version(data)?;
					let stored = <<Self as #crate_path::HasMigrationVersion>::Version as #crate_path::MigrationVersion>::into_u32(stored);
					match stored {
						#(#planner_arms)*
						_ => Err(#crate_path::PinaProgramError::InvalidMigrationVersion.into()),
					}
				}

				fn apply_migration(plan: Self::Plan, destination: &mut [u8]) {
					match plan {
						#(#apply_arms)*
						_ => {}
					}
				}

				fn validate_migration_destination(
					version: u32,
					data: &[u8],
				) -> #crate_path::ProgramResult {
					if !<Self as #crate_path::HasDiscriminator>::matches_discriminator(data) {
						return Err(#crate_path::ProgramError::InvalidAccountData);
					}
					match version {
						#(#destination_validation_arms)*
						_ => Err(#crate_path::PinaProgramError::InvalidMigrationVersion.into()),
					}
				}

				#validate_current
			}
		})
	}
}

const MAX_INLINE_STEPS: u16 = 8;

/// Render the Rust integer type for one program-wide version encoding.
pub(crate) fn version_type_tokens(version_type: MigrationVersionType) -> proc_macro2::TokenStream {
	match version_type {
		MigrationVersionType::U8 => quote!(u8),
		MigrationVersionType::U16 => quote!(u16),
		MigrationVersionType::U32 => quote!(u32),
	}
}

/// The instruction-space envelope gate for one `#[discriminator]` enum.
///
/// Derived from the checked-in manifest: every recorded instruction contract
/// pins its discriminant value to the newest version the program knows, so
/// instruction dispatch cannot accept a version byte the deployed program
/// never emitted. Zero-field instructions are the motivation — their programs
/// have no payload to parse, so without this gate no byte in the version slot
/// would ever be checked on the wire.
pub(crate) struct InstructionEnvelopeGate {
	/// `discriminant value → newest recorded version`, sorted by value.
	versions: Vec<(u64, u32)>,
	version_type: MigrationVersionType,
}

impl InstructionEnvelopeGate {
	/// Emit an envelope-checking `IntoDiscriminator` implementation.
	///
	/// The generated parse keeps every error the discriminator check produces,
	/// reports a missing version byte as `InvalidInstructionData` (the same
	/// wrong-width error the payload path returns), and reports an unknown or
	/// future version as `InvalidMigrationVersion` (the same typed error the
	/// payload path returns). Known stale versions still parse: migration-aware
	/// dispatch receives them and normalizes.
	pub(crate) fn implementation(
		&self,
		crate_path: &syn::Path,
		enum_name: &syn::Ident,
		primitive: &impl quote::ToTokens,
	) -> proc_macro2::TokenStream {
		let version_type = version_type_tokens(self.version_type);
		let version_bytes = self.version_type.bytes();
		let header_end = quote!(
			(::core::mem::size_of::<#primitive>() + #version_bytes)
		);
		let arms = self.versions.iter().map(|(value, current)| {
			// Unsuffixed so the arm infers the enum's primitive width: the
			// manifest stores discriminators as `u64` regardless of encoding.
			let value = proc_macro2::Literal::u64_unsuffixed(*value);
			quote! {
				#value => ::core::option::Option::Some(#current as #version_type),
			}
		});

		quote! {
			const _: () = assert!(
				::core::mem::size_of::<#enum_name>() == ::core::mem::size_of::<#primitive>(),
				concat!(
					"The size of the enum `",
					stringify!(#enum_name),
					"` must match the size of its primitive representation
					`",
					stringify!(#primitive),
					"`."
				),
			);

			impl #crate_path::IntoDiscriminator for #enum_name {
				#[inline]
				fn discriminator_from_bytes(
					bytes: &[u8],
				) -> ::core::result::Result<Self, #crate_path::ProgramError> {
					// One parse feeds both the envelope gate and the final
					// conversion: the gate must not pay a second decode on the
					// dispatch hot path.
					let value =
						<#primitive as #crate_path::IntoDiscriminator>::discriminator_from_bytes(
							bytes,
						)?;
					if let ::core::option::Option::Some(current) =
						Self::__pina_instruction_envelope_current(value)
					{
						let encoded = bytes
							.get(::core::mem::size_of::<#primitive>()..#header_end)
							.ok_or(#crate_path::ProgramError::InvalidInstructionData)?;
						let mut stored = [0_u8; #version_bytes];
						stored.copy_from_slice(encoded);
						if #version_type::from_le_bytes(stored) > current {
							return Err(#crate_path::PinaProgramError::InvalidMigrationVersion
								.into());
						}
					}
					Self::try_from(value)
				}

				fn write_discriminator(&self, bytes: &mut [u8]) {
					(*self as #primitive).write_discriminator(bytes);
				}

				fn try_write_discriminator(
					&self,
					bytes: &mut [u8],
				) -> ::core::result::Result<(), #crate_path::ProgramError> {
					(*self as #primitive).try_write_discriminator(bytes)
				}

				fn matches_discriminator(&self, bytes: &[u8]) -> bool {
					(*self as #primitive).matches_discriminator(bytes)
				}
			}

			impl #enum_name {
				/// Newest recorded version per enveloped instruction
				/// discriminant, or `None` when the discriminant names an
				/// unenveloped instruction space.
				#[doc(hidden)]
				const fn __pina_instruction_envelope_current(
					value: #primitive,
				) -> ::core::option::Option<#version_type> {
					match value {
						#(#arms)*
						_ => ::core::option::Option::None,
					}
				}
			}
		}
	}
}

/// Resolve the instruction-space envelope gate for a `#[discriminator]` enum.
///
/// Returns `None` when the expanding crate has no migration manifest or the
/// manifest records no instruction contracts: programs that never opted into
/// envelopes keep the plain discriminator parser and its exact expansion.
pub(crate) fn instruction_envelope_gate(
	enum_name: &syn::Ident,
) -> syn::Result<Option<InstructionEnvelopeGate>> {
	let Some((program_dir, _prefix)) = discover_program_dir() else {
		return Ok(None);
	};
	instruction_envelope_gate_at(enum_name, &program_dir.join(MANIFEST_PATH))
}

/// Pure variant for tests: resolve the gate from the manifest at `path`.
fn instruction_envelope_gate_at(
	enum_name: &syn::Ident,
	path: &std::path::Path,
) -> syn::Result<Option<InstructionEnvelopeGate>> {
	let Some(manifest) = read_validated_manifest(enum_name, path)? else {
		return Ok(None);
	};

	let mut versions = manifest
		.contracts
		.values()
		.filter(|history| history.identity.kind == ContractKind::Instruction)
		.map(|history| {
			let value = history.identity.discriminator_value().map_err(|error| {
				syn::Error::new_spanned(
					enum_name,
					format!("invalid migration manifest discriminator: {error}"),
				)
			})?;
			// `manifest.validate()` bounds every version number by the
			// encoding's maximum, so the subtraction cannot underflow and the
			// stored current version always fits the encoding.
			let current = history.versions.len().saturating_sub(1) as u32;

			Ok((value, current))
		})
		.collect::<syn::Result<Vec<_>>>()?;
	versions.sort_unstable();
	versions.dedup();

	if versions.is_empty() {
		return Ok(None);
	}

	Ok(Some(InstructionEnvelopeGate {
		versions,
		version_type: manifest.version_type,
	}))
}

/// Resolve whether one schema declaration opts into ABI history.
///
/// The checked-in manifest is the only policy source: an explicit per-item
/// `migrations = false` wins over the recorded auto policy, and a missing
/// manifest means no auto policy exists yet. A `migrations = false` on a
/// contract the manifest already records is rejected because stripping an
/// envelope is itself a wire-format change.
pub(crate) fn resolve_opt_in(
	item: &ItemStruct,
	kind: ContractKind,
	declared: Option<bool>,
	manifest: Option<&MigrationManifest>,
) -> syn::Result<bool> {
	match declared {
		Some(false) => {
			if manifest.is_some_and(|manifest| {
				manifest
					.contract_for_source(kind, &item.ident.to_string())
					.is_ok()
			}) {
				return Err(syn::Error::new_spanned(
					item,
					format!(
						"`migrations = false` on `{}` would remove an envelope the migration \
						 manifest already records; removing an envelope is a wire-format change \
						 that `pina migrations make` must record deliberately. Keep the contract \
						 migration-aware or retire its history first",
						item.ident
					),
				));
			}
			Ok(false)
		}
		Some(true) => Ok(true),
		None => Ok(manifest.is_some_and(|manifest| manifest.auto.contains(kind))),
	}
}

/// Read and validate the checked-in manifest for a schema declaration.
///
/// The manifest is read for every schema declaration because the recorded
/// auto policy decides whether an unannotated declaration opts in. Returns
/// `None` in place of the manifest when the program has no manifest yet. The
/// program directory is always returned so callers can name the expected path
/// in diagnostics.
pub(crate) fn read_manifest(
	item: &ItemStruct,
) -> syn::Result<(Option<MigrationManifest>, PathBuf, String)> {
	let Some((program_dir, prefix)) = discover_program_dir() else {
		return Ok((
			None,
			PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default()),
			String::new(),
		));
	};
	let manifest = read_manifest_at(item, &program_dir)?;
	Ok((manifest, program_dir, prefix))
}

/// Locate the program directory that owns `migrations/manifest.json`.
///
/// Starts at `CARGO_MANIFEST_DIR` and walks up. A crate that source-includes a
/// program (`#[path = "../../src/lib.rs"]`, as Surfpool harnesses do) expands
/// the program's macros with the harness's manifest directory, so a direct
/// lookup would miss the manifest and silently compile the program
/// unenveloped. Returns the discovered directory and the `../` prefix that
/// resolves generated file paths from the expanding crate.
pub(crate) fn discover_program_dir() -> Option<(PathBuf, String)> {
	let start = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR")?);
	let mut current = start.clone();
	let mut prefix = String::new();
	loop {
		if current.join(MANIFEST_PATH).is_file() {
			return Some((current, prefix));
		}
		if !current.pop() {
			return None;
		}
		prefix.push_str("../");
	}
}

/// Read and validate `migrations/manifest.json` under `program_dir`.
fn read_manifest_at(
	item: &ItemStruct,
	program_dir: &std::path::Path,
) -> syn::Result<Option<MigrationManifest>> {
	let path = program_dir.join(MANIFEST_PATH);
	let source = match std::fs::read(&path) {
		Ok(source) => source,
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
		Err(error) => {
			// This fires before the opt-in is resolved, so the message must
			// not claim that the declaration opted into anything.
			return Err(syn::Error::new_spanned(
				item,
				format!(
					"`{}` cannot resolve its migration policy because {} could not be read \
					 ({error}); repair the file, or delete it if the program should not opt into \
					 migrations (`pina migrations make` regenerates it)",
					item.ident,
					path.display()
				),
			));
		}
	};
	let manifest: MigrationManifest = pina_abi::decode_manifest(&source).map_err(|error| {
		syn::Error::new_spanned(
			item,
			format!("invalid migration manifest {}: {error}", path.display()),
		)
	})?;
	manifest.validate().map_err(|error| {
		syn::Error::new_spanned(
			item,
			format!("invalid migration manifest {}: {error}", path.display()),
		)
	})?;
	Ok(Some(manifest))
}

/// Resolve opt-in from the manifest policy and load the contract history.
///
/// Declarations covered by the manifest's auto policy resolve exactly like an
/// explicit `migrations` token, so a new contract still fails the build with
/// the `pina migrations make` remedy until it has a snapshot.
pub(crate) fn expansion(
	item: &ItemStruct,
	kind: ContractKind,
	layout: LayoutKind,
	declared: Option<bool>,
) -> syn::Result<Option<MigrationExpansion>> {
	let (manifest, program_dir, manifest_prefix) = read_manifest(item)?;
	let Some(manifest) = resolve_manifest(item, kind, declared, manifest, &program_dir)? else {
		return Ok(None);
	};
	MigrationExpansion::load(
		item,
		kind,
		layout,
		&manifest,
		&program_dir,
		&manifest_prefix,
	)
	.map(Some)
}

/// Resolve the opt-in and require a manifest when the declaration is enabled.
fn resolve_manifest(
	item: &ItemStruct,
	kind: ContractKind,
	declared: Option<bool>,
	manifest: Option<MigrationManifest>,
	program_dir: &std::path::Path,
) -> syn::Result<Option<MigrationManifest>> {
	if !resolve_opt_in(item, kind, declared, manifest.as_ref())? {
		return Ok(None);
	}

	manifest.map(Some).ok_or_else(|| {
		syn::Error::new_spanned(
			item,
			format!(
				"{} is opted into migrations, but {} does not exist; run `pina migrations make`",
				item.ident,
				program_dir.join(MANIFEST_PATH).display()
			),
		)
	})
}

/// Derive the reserved-`Migrate` ladder from the checked-in manifest.
///
/// Every enveloped account contract becomes a slot, in the manifest's
/// identity-sorted key order — the same order generated clients compose, so
/// the endpoint and its callers can never disagree about slot assignment.
/// Instruction and event contracts are skipped: only accounts migrate.
/// Returns an empty ladder when no manifest exists (the program has not
/// opted into migrations).
pub(crate) fn manifest_account_ladder(
	enum_name: &syn::Ident,
) -> syn::Result<Vec<proc_macro2::Ident>> {
	let Some((program_dir, _prefix)) = discover_program_dir() else {
		return Ok(Vec::new());
	};
	manifest_account_ladder_at(enum_name, &program_dir.join(MANIFEST_PATH))
}

/// Read and validate the manifest at `path` for diagnostics anchored at
/// `enum_name`.
///
/// Returns `None` when the file does not exist. Every reader that derives
/// macro output from the checked-in manifest shares this function so a hostile
/// or stale document is reported identically.
fn read_validated_manifest(
	enum_name: &syn::Ident,
	path: &std::path::Path,
) -> syn::Result<Option<MigrationManifest>> {
	let source = match std::fs::read(path) {
		Ok(source) => source,
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
		Err(error) => {
			return Err(syn::Error::new_spanned(
				enum_name,
				format!(
					"the migration-aware discriminator cannot read {} ({error}); repair the file, \
					 or delete it if the program should not opt into migrations",
					path.display()
				),
			));
		}
	};
	let manifest: MigrationManifest = pina_abi::decode_manifest(&source).map_err(|error| {
		syn::Error::new_spanned(
			enum_name,
			format!("invalid migration manifest {}: {error}", path.display()),
		)
	})?;
	manifest.validate().map_err(|error| {
		syn::Error::new_spanned(
			enum_name,
			format!("invalid migration manifest {}: {error}", path.display()),
		)
	})?;

	Ok(Some(manifest))
}

/// Pure variant for tests: derive the ladder from the manifest at `path`.
fn manifest_account_ladder_at(
	enum_name: &syn::Ident,
	path: &std::path::Path,
) -> syn::Result<Vec<proc_macro2::Ident>> {
	let Some(manifest) = read_validated_manifest(enum_name, path)? else {
		return Ok(Vec::new());
	};

	let ladder = manifest
		.contracts
		.values()
		.filter(|history| history.identity.kind == ContractKind::Account)
		.map(|history| syn::Ident::new(&history.rust_name, enum_name.span()))
		.collect();
	Ok(ladder)
}

/// Verify that every contract named by a dispatch migration ladder is
/// recorded in the checked-in manifest.
///
/// The generated ladder calls `MigratableAccount`, so a contract with no
/// snapshot would otherwise fail later as an unsatisfied trait bound. Reporting
/// the `pina migrations make` remedy here keeps the failure mode identical to
/// a schema that opts into migrations without a snapshot.
pub(crate) fn verify_migration_contracts(
	enum_name: &syn::Ident,
	ladder: &[syn::Path],
) -> syn::Result<()> {
	let program_dir = std::env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from);

	verify_ladder(enum_name, ladder, program_dir.as_deref())
}

/// Verify a dispatch migration ladder against the manifest under
/// `program_dir`.
///
/// The manifest location is passed in rather than read from the environment so
/// every failure mode stays unit-testable.
fn verify_ladder(
	enum_name: &syn::Ident,
	ladder: &[syn::Path],
	program_dir: Option<&std::path::Path>,
) -> syn::Result<()> {
	let Some(program_dir) = program_dir else {
		return Err(syn::Error::new_spanned(
			enum_name,
			"could not locate Cargo manifest for migration-aware dispatch",
		));
	};
	let path = program_dir.join(MANIFEST_PATH);
	let source = match std::fs::read(&path) {
		Ok(source) => source,
		// The diagnostics name the manifest by its fixed location, never by the
		// resolved path: an absolute path embeds the cargo target directory,
		// which differs between a normal build and a coverage build and would
		// make the emitted message unstable across environments.
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
			return Err(syn::Error::new_spanned(
				enum_name,
				"this program declares a migration ladder, but its `migrations/manifest.json` \
				 does not exist; run `pina migrations make`",
			));
		}
		Err(error) => {
			return Err(syn::Error::new_spanned(
				enum_name,
				format!(
					"the migration ladder cannot be verified because `migrations/manifest.json` \
					 could not be read ({error}); run `pina migrations make` to regenerate it"
				),
			));
		}
	};
	let manifest: MigrationManifest = pina_abi::decode_manifest(&source).map_err(|error| {
		syn::Error::new_spanned(
			enum_name,
			format!("invalid migration manifest `migrations/manifest.json`: {error}"),
		)
	})?;
	manifest.validate().map_err(|error| {
		syn::Error::new_spanned(
			enum_name,
			format!("invalid migration manifest `migrations/manifest.json`: {error}"),
		)
	})?;

	require_ladder_contracts(&manifest, ladder)
}

/// Require every ladder entry to be a recorded migratable account contract.
fn require_ladder_contracts(manifest: &MigrationManifest, ladder: &[syn::Path]) -> syn::Result<()> {
	for account in ladder {
		let name = account
			.segments
			.last()
			.expect("darling parses ladder entries as paths")
			.ident
			.to_string();

		manifest
			.contract_for_source(ContractKind::Account, &name)
			.map_err(|error| syn::Error::new_spanned(account, error))?;
	}

	Ok(())
}

fn verify_transition_files(
	item: &ItemStruct,
	program_dir: &std::path::Path,
	history: &ContractHistory,
) -> syn::Result<()> {
	for (number, version) in history.versions.iter().enumerate().skip(1) {
		let number = number as u32;
		let transition = version.transition.as_ref().ok_or_else(|| {
			syn::Error::new_spanned(
				item,
				format!("migration version {number} has no adjacent transition"),
			)
		})?;
		let path = program_dir.join(pina_abi::transition_path(
			&history.identity,
			number - 1,
			number,
		));
		let bytes = std::fs::read(&path).map_err(|error| {
			syn::Error::new_spanned(
				item,
				format!(
					"could not read migration transition {}: {error}",
					path.display()
				),
			)
		})?;
		if transition.mode == TransitionMode::Manual
			&& bytes
				.windows("TODO(pina-manual-migration)".len())
				.any(|window| window == b"TODO(pina-manual-migration)")
		{
			return Err(syn::Error::new_spanned(
				item,
				format!(
					"manual migration {} is unfinished; replace its TODO body and run `pina \
					 migrations make`",
					path.display()
				),
			));
		}
		let hash = pina_abi::sha256_bytes(&bytes);
		if transition.implementation_sha256.as_deref() != Some(hash.as_str()) {
			return Err(syn::Error::new_spanned(
				item,
				format!(
					"migration transition {} differs from its checked-in hash; run `pina \
					 migrations make`",
					path.display()
				),
			));
		}
	}
	Ok(())
}

fn historical_struct(
	crate_path: &syn::Path,
	struct_name: &syn::Ident,
	version: &pina_abi::SchemaVersion,
	number: u32,
	discriminator_bytes: u8,
	version_bytes: usize,
	visibility: &syn::Visibility,
) -> syn::Result<proc_macro2::TokenStream> {
	let name = historical_struct_name(struct_name, number);
	let discriminator_bytes = usize::from(discriminator_bytes);
	// Account histories borrow the account's visibility so the generated
	// versioned view can read their payload fields without widening the source
	// account's API. Instruction and event histories stay private; only their
	// internal migrators use them.
	let fields = version
		.schema
		.fields
		.iter()
		.map(|field| {
			let name = syn::parse_str::<syn::Ident>(&field.name).map_err(|error| {
				syn::Error::new_spanned(
					struct_name,
					format!("invalid historical field `{}`: {error}", field.name),
				)
			})?;
			let ty = abi_type_tokens(&field.rust_type, crate_path, struct_name)?;
			Ok(quote!(#visibility #name: #ty))
		})
		.collect::<syn::Result<Vec<_>>>()?;
	let physical = version
		.schema
		.physical()
		.map_err(|error| syn::Error::new_spanned(struct_name, error))?;
	let (attribute, proof) = match (version.schema.layout, &physical) {
		(LayoutKind::Fixed, PhysicalLayout::Fixed { size, .. }) => {
			let payload_size = usize::try_from(*size).map_err(|_| {
				syn::Error::new_spanned(struct_name, "historical fixed schema exceeds usize")
			})?;
			let expected_size = discriminator_bytes
				.checked_add(version_bytes)
				.and_then(|header| header.checked_add(payload_size))
				.ok_or_else(|| {
					syn::Error::new_spanned(struct_name, "historical schema size overflowed")
				})?;
			(
				quote!(#[pinapod(crate = #crate_path::pinapod, no_inherent)]),
				quote! {
					const _: () = {
						::core::assert!(::core::mem::size_of::<<#name as #crate_path::PinaPodFixed>::Zc>() == #expected_size);
					};
				},
			)
		}
		(
			LayoutKind::Compact,
			PhysicalLayout::Compact {
				header_size,
				maximum_size,
				tail_alignment,
				..
			},
		) => {
			let envelope_size =
				discriminator_bytes
					.checked_add(version_bytes)
					.ok_or_else(|| {
						syn::Error::new_spanned(
							struct_name,
							"historical compact envelope overflowed",
						)
					})?;
			let header_size = usize::try_from(*header_size)
				.ok()
				.and_then(|size| envelope_size.checked_add(size))
				.ok_or_else(|| {
					syn::Error::new_spanned(struct_name, "historical compact header overflowed")
				})?;
			let maximum_size = usize::try_from(*maximum_size)
				.ok()
				.and_then(|size| envelope_size.checked_add(size))
				.ok_or_else(|| {
					syn::Error::new_spanned(struct_name, "historical compact maximum overflowed")
				})?;
			let tail_alignment = usize::try_from(*tail_alignment).map_err(|_| {
				syn::Error::new_spanned(struct_name, "historical compact alignment exceeds usize")
			})?;
			(
				quote!(#[pinapod(crate = #crate_path::pinapod, compact, no_inherent)]),
				quote! {
					const _: () = {
						::core::assert!(<#name as #crate_path::PinaPodCompact>::HEADER_SIZE == #header_size);
						::core::assert!(<#name as #crate_path::PinaPodCompact>::MIN_SIZE == #header_size);
						::core::assert!(<#name as #crate_path::PinaPodCompact>::MAX_SIZE == #maximum_size);
						::core::assert!(<#name as #crate_path::PinaPodCompact>::TAIL_ALIGNMENT == #tail_alignment);
					};
				},
			)
		}
		_ => {
			return Err(syn::Error::new_spanned(
				struct_name,
				"historical schema physical descriptor does not match its layout family",
			));
		}
	};

	Ok(quote! {
		#[allow(dead_code)]
		#[doc(hidden)]
		#[derive(#crate_path::pinapod::PinaPod)]
		#attribute
		#visibility struct #name {
			discriminator: [u8; #discriminator_bytes],
			migration_version: [u8; #version_bytes],
			#(#fields,)*
		}

		#proof
	})
}

fn historical_struct_name(struct_name: &syn::Ident, version: u32) -> syn::Ident {
	format_ident!("__PinaMigration{}V{}", struct_name, version)
}

fn versioned_enum_name(struct_name: &syn::Ident) -> syn::Ident {
	format_ident!("{}Versioned", struct_name)
}

fn versioned_variant_name(version: u32) -> syn::Ident {
	format_ident!("V{}", version)
}

fn abi_type_tokens(
	rust_type: &str,
	crate_path: &syn::Path,
	span: &syn::Ident,
) -> syn::Result<proc_macro2::TokenStream> {
	let ty = syn::parse_str::<syn::Type>(rust_type).map_err(|error| {
		syn::Error::new_spanned(
			span,
			format!("invalid historical type `{rust_type}`: {error}"),
		)
	})?;
	render_abi_type(&ty, crate_path, span)
}

fn render_abi_type(
	ty: &syn::Type,
	crate_path: &syn::Path,
	span: &syn::Ident,
) -> syn::Result<proc_macro2::TokenStream> {
	match ty {
		syn::Type::Array(array) => {
			let element = render_abi_type(&array.elem, crate_path, span)?;
			let length = &array.len;
			Ok(quote!([#element; #length]))
		}
		syn::Type::Path(path) if path.qself.is_none() => {
			let segment = path.path.segments.last().ok_or_else(|| {
				syn::Error::new_spanned(span, "historical type path cannot be empty")
			})?;
			let ident = &segment.ident;
			let arguments = match &segment.arguments {
				syn::PathArguments::None => quote!(),
				syn::PathArguments::AngleBracketed(arguments) => {
					let rendered = arguments
						.args
						.iter()
						.map(|argument| {
							match argument {
								syn::GenericArgument::Type(ty) => {
									render_abi_type(ty, crate_path, span)
								}
								other => Ok(quote!(#other)),
							}
						})
						.collect::<syn::Result<Vec<_>>>()?;
					quote!(<#(#rendered),*>)
				}
				syn::PathArguments::Parenthesized(_) => {
					return Err(syn::Error::new_spanned(
						span,
						"function-like historical field types are unsupported",
					));
				}
			};
			let name = ident.to_string();
			if name == "Option" {
				Ok(quote!(::core::option::Option #arguments))
			} else if matches!(
				name.as_str(),
				"Address"
					| "String" | "Vec"
					| "PodString" | "PodVec"
					| "PodOption" | "PodBool"
					| "PodU16" | "PodU32"
					| "PodU64" | "PodU128"
					| "PodI16" | "PodI32"
					| "PodI64" | "PodI128"
			) {
				Ok(quote!(#crate_path::#ident #arguments))
			} else {
				Ok(quote!(#ident #arguments))
			}
		}
		_ => {
			Err(syn::Error::new_spanned(
				span,
				"unsupported historical ABI field type",
			))
		}
	}
}

fn verify_source_schema(
	item: &ItemStruct,
	actual: &DataSchema,
	expected: &DataSchema,
) -> syn::Result<()> {
	if actual == expected {
		return Ok(());
	}

	Err(syn::Error::new_spanned(
		item,
		format!(
			"migration-aware schema `{}` differs from its checked-in snapshot; run `pina \
			 migrations make` and review the generated transition",
			item.ident
		),
	))
}

#[cfg(test)]
mod tests {
	use std::path::Path;

	use pina_abi::ContractIdentity;
	use pina_abi::ContractKind;
	use pina_abi::MigrationAuto;
	use pina_abi::MigrationManifest;
	use pina_abi::MigrationVersionType;
	use pina_abi::SchemaVersion;
	use tempfile::TempDir;

	use super::*;

	fn manifest_for(
		item: &ItemStruct,
		kind: ContractKind,
		auto: MigrationAuto,
	) -> MigrationManifest {
		let schema = pina_abi::data_schema(item, LayoutKind::Fixed)
			.unwrap_or_else(|error| panic!("test schema: {error}"));
		let identity = ContractIdentity::try_new(kind, 1, 1)
			.unwrap_or_else(|error| panic!("test identity: {error}"));
		let key = identity.key();
		let mut manifest = MigrationManifest::new("program".to_owned(), MigrationVersionType::U8);
		manifest.auto = auto;
		let history = ContractHistory {
			identity,
			rust_name: item.ident.to_string(),
			versions: vec![SchemaVersion {
				schema,
				process: None,
				transition: None,
			}],
		};
		manifest.contracts.insert(key, history);
		manifest
	}

	fn item_struct(name: &str) -> ItemStruct {
		syn::parse_str(&format!("struct {name} {{ value: u64 }}"))
			.unwrap_or_else(|error| panic!("test item: {error}"))
	}

	fn gate_contract(
		kind: ContractKind,
		discriminator: u64,
		version_count: usize,
	) -> ContractHistory {
		let identity = ContractIdentity::try_new(kind, 1, discriminator)
			.unwrap_or_else(|error| panic!("test identity: {error}"));
		let schema = pina_abi::data_schema(&item_struct("Placeholder"), LayoutKind::Fixed)
			.unwrap_or_else(|error| panic!("test schema: {error}"));
		// Only instruction contracts record a process; the encode path
		// requires one on their first version.
		let process = (kind == ContractKind::Instruction).then_some(pina_abi::ProcessContract {
			accounts: Vec::new(),
		});
		ContractHistory {
			identity,
			rust_name: "Placeholder".to_owned(),
			versions: (0..version_count)
				.map(|number| {
					SchemaVersion {
						schema: schema.clone(),
						process: process.clone(),
						// Version zero carries no transition; every later
						// version records its (never-executed) adjacent step.
						transition: (number > 0).then_some(pina_abi::Transition {
							mode: pina_abi::TransitionMode::Automatic,
							renames: Vec::new(),
							implementation_sha256: None,
						}),
					}
				})
				.collect(),
		}
	}

	fn write_gate_manifest(dir: &Path, contracts: &[ContractHistory]) -> PathBuf {
		let mut manifest = MigrationManifest::new("program".to_owned(), MigrationVersionType::U8);
		for contract in contracts {
			manifest
				.contracts
				.insert(contract.identity.key(), contract.clone());
		}
		write_manifest(dir, &encode(&manifest))
	}

	fn gate_at(path: &Path) -> Option<Vec<(u64, u32)>> {
		let enum_name = syn::Ident::new("Instruction", proc_macro2::Span::call_site());
		instruction_envelope_gate_at(&enum_name, path)
			.unwrap_or_else(|error| panic!("resolve gate: {error}"))
			.map(|gate| gate.versions)
	}

	#[test]
	fn envelope_gate_pins_only_instruction_contracts_to_their_newest_version() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let path = write_gate_manifest(
			temp.path(),
			&[
				// A three-version instruction ladder pins its newest version.
				gate_contract(ContractKind::Instruction, 0, 3),
				// A single-version instruction pins version zero.
				gate_contract(ContractKind::Instruction, 5, 1),
				// Events and accounts are enveloped too, but they are not
				// dispatched through the instruction space.
				gate_contract(ContractKind::Event, 9, 1),
				gate_contract(ContractKind::Account, 7, 1),
			],
		);

		assert_eq!(gate_at(&path), Some(vec![(0, 2), (5, 0)]));
	}

	#[test]
	fn envelope_gate_is_absent_without_a_manifest_or_instruction_contracts() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));

		// No manifest file: the program never opted into envelopes.
		assert!(gate_at(&temp.path().join(MANIFEST_PATH)).is_none());

		// A manifest with no instruction contracts leaves the plain parser.
		let path = write_gate_manifest(temp.path(), &[gate_contract(ContractKind::Account, 1, 1)]);
		assert!(gate_at(&path).is_none());
	}

	#[test]
	fn envelope_gate_implementation_renders_the_fail_closed_check() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let path = write_gate_manifest(
			temp.path(),
			&[gate_contract(ContractKind::Instruction, 0, 1)],
		);
		let gate = instruction_envelope_gate_at(
			&syn::Ident::new("Instruction", proc_macro2::Span::call_site()),
			&path,
		)
		.unwrap_or_else(|error| panic!("resolve gate: {error}"))
		.unwrap_or_else(|| panic!("an instruction contract must produce a gate"));
		let crate_path: syn::Path = syn::parse_quote!(::pina);
		let enum_name: syn::Ident = syn::parse_quote!(Instruction);
		let primitive: syn::Path = syn::parse_quote!(u8);
		let rendered = gate
			.implementation(&crate_path, &enum_name, &primitive)
			.to_string();

		// A missing version byte is a wrong-width error; an unknown version is
		// the same typed error the payload path returns.
		assert!(
			rendered.contains("__pina_instruction_envelope_current"),
			"rendered: {rendered}"
		);
		assert!(
			rendered.contains("InvalidInstructionData"),
			"rendered: {rendered}"
		);
		assert!(
			rendered.contains("InvalidMigrationVersion"),
			"rendered: {rendered}"
		);
	}

	#[test]
	fn explicit_token_opts_in_for_every_kind() {
		for kind in ContractKind::ALL {
			assert!(
				resolve_opt_in(&item_struct("State"), kind, Some(true), None)
					.unwrap_or_else(|error| panic!("explicit opt-in: {error}")),
			);
		}
	}

	#[test]
	fn manifest_auto_policy_opts_in_unannotated_declarations() {
		for kind in ContractKind::ALL {
			let item = item_struct("State");
			let manifest = manifest_for(&item, kind, MigrationAuto::all());
			assert!(
				resolve_opt_in(&item, kind, None, Some(&manifest))
					.unwrap_or_else(|error| panic!("auto opt-in: {error}")),
			);

			// A policy for another kind leaves this declaration opted out.
			let other = ContractKind::ALL
				.into_iter()
				.find(|candidate| *candidate != kind)
				.unwrap_or_else(|| panic!("a second kind exists"));
			let mut policy = MigrationAuto::none();
			policy.add(other);
			let manifest = manifest_for(&item, kind, policy);
			assert!(
				!resolve_opt_in(&item, kind, None, Some(&manifest))
					.unwrap_or_else(|error| panic!("uncovered kind: {error}")),
			);
		}
	}

	#[test]
	fn missing_manifest_has_no_auto_policy() {
		assert!(
			!resolve_opt_in(&item_struct("State"), ContractKind::Account, None, None)
				.unwrap_or_else(|error| panic!("absent policy: {error}")),
		);
	}

	#[test]
	fn disabled_override_wins_over_auto_and_is_rejected_once_recorded() {
		for kind in ContractKind::ALL {
			let item = item_struct("State");
			let manifest = manifest_for(&item, kind, MigrationAuto::all());

			// Not recorded: the explicit override stands even under auto.
			let other = item_struct("Other");
			assert!(
				!resolve_opt_in(&other, kind, Some(false), Some(&manifest))
					.unwrap_or_else(|error| panic!("explicit opt-out: {error}")),
			);

			// Recorded: removing the envelope is a deliberate wire-format change.
			let error = resolve_opt_in(&item, kind, Some(false), Some(&manifest))
				.expect_err("a recorded contract cannot opt out silently");
			let message = error.to_string();
			assert!(
				message.contains("wire-format change"),
				"unexpected message: {message}"
			);
			assert!(
				message.contains("must record deliberately"),
				"unexpected message: {message}"
			);
		}
	}

	#[test]
	fn manifest_records_do_not_opt_in_uncovered_kinds() {
		let item = item_struct("State");
		let manifest = manifest_for(&item, ContractKind::Instruction, MigrationAuto::none());
		// Auto does not cover accounts, so an unannotated account stays out even
		// though the manifest holds another contract.
		assert!(
			!resolve_opt_in(&item, ContractKind::Account, None, Some(&manifest))
				.unwrap_or_else(|error| panic!("uncovered account: {error}")),
		);
	}

	fn write_manifest(program_dir: &Path, source: &[u8]) -> PathBuf {
		let path = program_dir.join(MANIFEST_PATH);
		let parent = path
			.parent()
			.unwrap_or_else(|| panic!("manifest path has a parent directory"));
		std::fs::create_dir_all(parent)
			.unwrap_or_else(|error| panic!("create manifest directory: {error}"));
		std::fs::write(&path, source).unwrap_or_else(|error| panic!("write manifest: {error}"));
		path
	}

	fn encode(manifest: &MigrationManifest) -> Vec<u8> {
		pina_abi::encode_manifest(manifest)
			.unwrap_or_else(|error| panic!("encode manifest: {error}"))
	}

	#[test]
	fn derived_ladder_keeps_account_contracts_in_identity_order() {
		let temp = tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir: {error}"));
		let root = std::fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("canonical temp: {error}"));

		// Two accounts plus an event: only accounts become slots, and manifest
		// key order (`account:1:01` < `account:1:02`) decides their order,
		// matching the order generated clients compose.
		let mut manifest = MigrationManifest::new("program".to_owned(), MigrationVersionType::U8);
		for (name, kind, value) in [
			("Zebra", ContractKind::Account, 1u64),
			("Alpha", ContractKind::Account, 2u64),
			("Runner", ContractKind::Event, 3u64),
		] {
			let item = item_struct(name);
			let schema = pina_abi::data_schema(&item, LayoutKind::Fixed)
				.unwrap_or_else(|error| panic!("schema: {error}"));
			let identity = ContractIdentity::try_new(kind, 1, value)
				.unwrap_or_else(|error| panic!("identity: {error}"));
			manifest.contracts.insert(
				identity.key(),
				ContractHistory {
					identity,
					rust_name: name.to_owned(),
					versions: vec![SchemaVersion {
						schema,
						process: None,
						transition: None,
					}],
				},
			);
		}
		let path = write_manifest(&root, &encode(&manifest));
		let enum_name = syn::Ident::new("Instruction", proc_macro2::Span::call_site());
		let ladder = manifest_account_ladder_at(&enum_name, &path)
			.unwrap_or_else(|error| panic!("derive ladder: {error}"));

		let names: Vec<String> = ladder.iter().map(ToString::to_string).collect();
		assert_eq!(names, vec!["Zebra".to_owned(), "Alpha".to_owned()]);
	}

	#[test]
	fn absent_manifest_derives_an_empty_ladder() {
		let temp = tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir: {error}"));
		let root = std::fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("canonical temp: {error}"));
		let enum_name = syn::Ident::new("Instruction", proc_macro2::Span::call_site());
		let path = root.join(MANIFEST_PATH);
		let ladder = manifest_account_ladder_at(&enum_name, &path)
			.unwrap_or_else(|error| panic!("derive ladder: {error}"));

		assert!(ladder.is_empty());
	}

	#[test]
	fn read_manifest_reports_the_absent_program_manifest() {
		let item = item_struct("State");
		let (manifest, _program_dir, _prefix) =
			read_manifest(&item).unwrap_or_else(|error| panic!("read absent manifest: {error}"));

		assert!(manifest.is_none());
	}

	#[test]
	fn read_manifest_at_round_trips_the_recorded_auto_policy() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let item = item_struct("State");
		let mut manifest = manifest_for(&item, ContractKind::Account, MigrationAuto::all());
		manifest.auto.add(ContractKind::Event);
		write_manifest(temp.path(), &encode(&manifest));

		let loaded = read_manifest_at(&item, temp.path())
			.unwrap_or_else(|error| panic!("read manifest: {error}"))
			.expect("a written manifest must be read");

		assert_eq!(loaded.auto, manifest.auto);
		assert_eq!(loaded.contracts.len(), 1);
	}

	#[test]
	fn read_manifest_at_rejects_invalid_and_inconsistent_documents() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let item = item_struct("State");
		assert!(
			read_manifest_at(&item, temp.path())
				.unwrap_or_else(|error| panic!("read absent manifest: {error}"))
				.is_none()
		);

		// A future document version is rejected before the typed model is read.
		write_manifest(temp.path(), br#"{"abiVersion":"99.0"}"#);
		let future = read_manifest_at(&item, temp.path()).expect_err("future formats must fail");
		assert!(
			future.to_string().contains("invalid migration manifest"),
			"unexpected message: {future}"
		);

		// A document whose contract key disagrees with its identity fails
		// validation after a successful parse.
		let mut value: serde_json::Value = serde_json::from_slice(&encode(&manifest_for(
			&item,
			ContractKind::Account,
			MigrationAuto::all(),
		)))
		.unwrap_or_else(|error| panic!("manifest json: {error}"));
		let contracts = value["contracts"]
			.as_object_mut()
			.unwrap_or_else(|| panic!("manifest contracts object"));
		let history = contracts
			.iter()
			.next()
			.map(|(key, history)| (key.clone(), history.clone()))
			.unwrap_or_else(|| panic!("one recorded contract"));
		contracts.remove(&history.0);
		contracts.insert("account:9:09".to_owned(), history.1);
		write_manifest(
			temp.path(),
			&serde_json::to_vec(&value).unwrap_or_else(|error| panic!("encode json: {error}")),
		);

		let inconsistent =
			read_manifest_at(&item, temp.path()).expect_err("mismatched keys must fail");
		assert!(
			inconsistent
				.to_string()
				.contains("invalid migration manifest"),
			"unexpected message: {inconsistent}"
		);
	}

	#[test]
	fn read_manifest_at_reports_unreadable_manifest_paths() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let item = item_struct("State");

		// A directory at the manifest path fails every read except `NotFound`,
		// so the resolution failure is reported instead of a silent "no
		// policy". The message must not claim that `State` opted into anything.
		let path = temp.path().join(MANIFEST_PATH);
		std::fs::create_dir_all(&path)
			.unwrap_or_else(|error| panic!("create manifest directory: {error}"));

		let error =
			read_manifest_at(&item, temp.path()).expect_err("a directory cannot be a manifest");
		let message = error.to_string();
		assert!(
			message.contains("cannot resolve its migration policy"),
			"message: {message}"
		);
		assert!(message.contains("could not be read"), "message: {message}");
		assert!(
			message.contains("pina migrations make"),
			"message: {message}"
		);
		assert!(
			message.contains(&path.display().to_string()),
			"message: {message}"
		);
		assert!(!message.contains("is opted into migrations"));
	}

	#[test]
	fn read_manifest_at_rejects_manifests_that_are_not_utf8_documents() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let item = item_struct("State");

		// `read_manifest_at` never decodes UTF-8 itself, so garbage bytes fail
		// inside `decode_manifest` and are reported as an invalid document.
		let path = write_manifest(temp.path(), &[0xFF, 0xFE, b'{', b'}']);
		let garbage =
			read_manifest_at(&item, temp.path()).expect_err("invalid UTF-8 cannot be a manifest");
		let message = garbage.to_string();
		assert!(
			message.contains("invalid migration manifest"),
			"message: {message}"
		);
		assert!(
			message.contains(&path.display().to_string()),
			"message: {message}"
		);
	}

	#[test]
	fn enabled_declarations_require_the_manifest_file() {
		let item = item_struct("State");
		let program_dir = std::path::Path::new("program-root");

		// An unannotated declaration without a manifest has no policy to resolve.
		assert!(
			resolve_manifest(&item, ContractKind::Account, None, None, program_dir)
				.unwrap_or_else(|error| panic!("absent manifest: {error}"))
				.is_none()
		);

		// An explicit token still names the missing file and the remedy.
		let error = resolve_manifest(&item, ContractKind::Account, Some(true), None, program_dir)
			.expect_err("an enabled declaration requires a manifest");
		let message = error.to_string();
		assert!(message.contains("does not exist"), "message: {message}");
		assert!(
			message.contains("pina migrations make"),
			"message: {message}"
		);
		assert!(
			message.contains(&program_dir.join(MANIFEST_PATH).display().to_string()),
			"message: {message}"
		);

		// With a manifest, an auto-covered declaration resolves to it.
		let manifest = manifest_for(&item, ContractKind::Account, MigrationAuto::all());
		assert!(
			resolve_manifest(
				&item,
				ContractKind::Account,
				None,
				Some(manifest),
				program_dir
			)
			.unwrap_or_else(|error| panic!("auto manifest: {error}"))
			.is_some()
		);
	}
	fn ladder_path(name: &str) -> syn::Path {
		syn::parse_str(name).unwrap_or_else(|error| panic!("test path: {error}"))
	}

	#[test]
	fn ladder_without_a_cargo_manifest_dir_is_rejected() {
		let error = verify_ladder(
			&syn::parse_str::<syn::Ident>("Instruction")
				.unwrap_or_else(|error| panic!("ident: {error}")),
			&[ladder_path("State")],
			None,
		)
		.unwrap_err();

		assert!(
			error
				.to_string()
				.contains("could not locate Cargo manifest")
		);
	}

	#[test]
	fn ladder_without_a_manifest_names_the_remedy() {
		let dir = TempDir::new().unwrap_or_else(|error| panic!("tempdir: {error}"));
		let error = verify_ladder(
			&syn::parse_str::<syn::Ident>("Instruction")
				.unwrap_or_else(|error| panic!("ident: {error}")),
			&[ladder_path("State")],
			Some(dir.path()),
		)
		.unwrap_err();

		let message = error.to_string();
		assert!(message.contains("does not exist"), "message: {message}");
		assert!(
			message.contains("pina migrations make"),
			"message: {message}"
		);
		// The diagnostic must not embed the resolved path: it contains the cargo
		// target directory, which differs between build environments.
		assert!(!message.contains(dir.path().to_str().unwrap_or_default()));
	}

	#[test]
	fn unreadable_manifest_is_reported() {
		let dir = TempDir::new().unwrap_or_else(|error| panic!("tempdir: {error}"));
		let migrations = dir.path().join("migrations");
		std::fs::create_dir_all(&migrations)
			.unwrap_or_else(|error| panic!("test migrations dir: {error}"));
		// A directory in place of the manifest makes the read fail with a
		// non-NotFound error on every supported platform.
		std::fs::create_dir_all(migrations.join("manifest.json"))
			.unwrap_or_else(|error| panic!("test manifest dir: {error}"));

		let error = verify_ladder(
			&syn::parse_str::<syn::Ident>("Instruction")
				.unwrap_or_else(|error| panic!("ident: {error}")),
			&[ladder_path("State")],
			Some(dir.path()),
		)
		.unwrap_err();

		let message = error.to_string();
		assert!(message.contains("could not be read"), "message: {message}");
	}

	#[test]
	fn malformed_manifest_is_reported() {
		let dir = TempDir::new().unwrap_or_else(|error| panic!("tempdir: {error}"));
		write_manifest(dir.path(), b"not json");

		let error = verify_ladder(
			&syn::parse_str::<syn::Ident>("Instruction")
				.unwrap_or_else(|error| panic!("ident: {error}")),
			&[ladder_path("State")],
			Some(dir.path()),
		)
		.unwrap_err();

		assert!(error.to_string().contains("invalid migration manifest"));
	}

	#[test]
	fn manifest_failing_validation_is_reported() {
		let dir = TempDir::new().unwrap_or_else(|error| panic!("tempdir: {error}"));
		// A document version this build cannot read fails validation.
		write_manifest(
			dir.path(),
			br#"{"abiVersion":"99.0","programId":"GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS","versionType":"u8","contracts":{}}"#,
		);

		let error = verify_ladder(
			&syn::parse_str::<syn::Ident>("Instruction")
				.unwrap_or_else(|error| panic!("ident: {error}")),
			&[ladder_path("State")],
			Some(dir.path()),
		)
		.unwrap_err();

		assert!(error.to_string().contains("invalid migration manifest"));
	}

	#[test]
	fn ladder_entry_missing_from_the_manifest_is_reported() {
		let dir = TempDir::new().unwrap_or_else(|error| panic!("tempdir: {error}"));
		let item = item_struct("State");
		let manifest = manifest_for(&item, ContractKind::Account, MigrationAuto::none());
		write_manifest(dir.path(), &encode(&manifest));

		let error = verify_ladder(
			&syn::parse_str::<syn::Ident>("Instruction")
				.unwrap_or_else(|error| panic!("ident: {error}")),
			&[ladder_path("Unrelated")],
			Some(dir.path()),
		)
		.unwrap_err();

		let message = error.to_string();
		assert!(message.contains("Unrelated"), "message: {message}");
	}

	#[test]
	fn ladder_fully_recorded_in_the_manifest_passes() {
		let dir = TempDir::new().unwrap_or_else(|error| panic!("tempdir: {error}"));
		let item = item_struct("State");
		let manifest = manifest_for(&item, ContractKind::Account, MigrationAuto::none());
		write_manifest(dir.path(), &encode(&manifest));

		verify_ladder(
			&syn::parse_str::<syn::Ident>("Instruction")
				.unwrap_or_else(|error| panic!("ident: {error}")),
			&[ladder_path("State")],
			Some(dir.path()),
		)
		.unwrap_or_else(|error| panic!("recorded contract: {error}"));
	}
}
