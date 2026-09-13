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
		let current_version = current.version;
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
		})
	}

	pub(crate) fn version_bytes(&self) -> usize {
		self.version_type.bytes()
	}

	pub(crate) fn version_type_tokens(&self) -> proc_macro2::TokenStream {
		match self.version_type {
			MigrationVersionType::U8 => quote!(u8),
			MigrationVersionType::U16 => quote!(u16),
			MigrationVersionType::U32 => quote!(u32),
		}
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

		quote! {
			const _: &[u8] = include_bytes!(concat!(
				env!("CARGO_MANIFEST_DIR"),
				"/migrations/manifest.json",
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
		let transition_modules = versions.iter().skip(1).map(|version| {
			let transition = version
				.transition
				.as_ref()
				.expect("validated immutable history has adjacent transitions");
			let name = format_ident!("v{}_to_v{}", transition.from, transition.to);
			let relative =
				pina_abi::transition_path(&self.history.identity, transition.from, transition.to)
					.to_string_lossy()
					.replace('\\', "/");
			let include_path = format!("/{relative}");

			quote! {
				pub(crate) mod #name {
					include!(concat!(env!("CARGO_MANIFEST_DIR"), #include_path));
				}
			}
		});
		let historical_structs = versions
			.iter()
			.take(versions.len().saturating_sub(1))
			.map(|version| {
				historical_struct(
					crate_path,
					struct_name,
					version,
					self.discriminator_bytes,
					self.version_bytes(),
					&syn::Visibility::Inherited,
				)
			})
			.collect::<syn::Result<Vec<_>>>()?;
		let migration_arms =
			versions
				.iter()
				.take(versions.len().saturating_sub(1))
				.map(|version| {
					let number = version.version;
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
		let transition_modules = versions.iter().skip(1).map(|version| {
			let transition = version
				.transition
				.as_ref()
				.expect("automatic history has every adjacent transition");
			let name = format_ident!("v{}_to_v{}", transition.from, transition.to);
			let relative =
				pina_abi::transition_path(&self.history.identity, transition.from, transition.to)
					.to_string_lossy()
					.replace('\\', "/");
			let include_path = format!("/{relative}");

			quote! {
				pub(crate) mod #name {
					include!(concat!(env!("CARGO_MANIFEST_DIR"), #include_path));
				}
			}
		});
		let historical_structs = versions
			.iter()
			.take(versions.len().saturating_sub(1))
			.map(|version| {
				historical_struct(
					crate_path,
					struct_name,
					version,
					self.discriminator_bytes,
					self.version_bytes(),
					visibility,
				)
			})
			.collect::<syn::Result<Vec<_>>>()?;
		let planner_arms = versions
			.iter()
			.take(versions.len().saturating_sub(1))
			.map(|version| {
				let number = version.version;
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
			.take(versions.len().saturating_sub(1))
			.map(|version| {
				let number = version.version;
				let transition_name = format_ident!("v{}_to_v{}", number, number + 1);
				quote! {
					#number => #module_name::#transition_name::migrate(destination),
				}
			});
		let historical_validation_arms = versions
			.iter()
			.skip(1)
			.take(versions.len().saturating_sub(2))
			.map(|version| {
				let number = version.version;
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

		for version in versions.iter().take(versions.len().saturating_sub(1)) {
			let number = version.version;
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
		let transition_modules = versions.iter().skip(1).map(|version| {
			let transition = version
				.transition
				.as_ref()
				.expect("validated account history has adjacent transitions");
			let name = format_ident!("v{}_to_v{}", transition.from, transition.to);
			let relative =
				pina_abi::transition_path(&self.history.identity, transition.from, transition.to)
					.to_string_lossy()
					.replace('\\', "/");
			let include_path = format!("/{relative}");

			quote! {
				pub(crate) mod #name {
					include!(concat!(env!("CARGO_MANIFEST_DIR"), #include_path));
				}
			}
		});
		let historical_structs = versions
			.iter()
			.take(versions.len().saturating_sub(1))
			.map(|version| {
				historical_struct(
					crate_path,
					struct_name,
					version,
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
			let from = source.version;
			let to = destination.version;
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
		for version in versions.iter().skip(1) {
			let number = version.version;
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

/// Read and validate the checked-in manifest for a migration-aware schema.
///
/// Returns `None` in place of the manifest when the program has no manifest
/// yet. The program directory is always returned so callers can name the
/// expected path in diagnostics.
pub(crate) fn read_manifest(
	item: &ItemStruct,
) -> syn::Result<(Option<MigrationManifest>, PathBuf)> {
	let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR").ok_or_else(|| {
		syn::Error::new_spanned(
			item,
			"could not locate Cargo manifest for migration-aware schema",
		)
	})?;
	let program_dir = PathBuf::from(manifest_dir);
	let manifest = read_manifest_at(item, &program_dir)?;
	Ok((manifest, program_dir))
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
			return Err(syn::Error::new_spanned(
				item,
				format!(
					"{} is opted into migrations, but {} could not be read ({error}); run `pina \
					 migrations make`",
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
	let (manifest, program_dir) = read_manifest(item)?;
	let Some(manifest) = resolve_manifest(item, kind, declared, manifest, &program_dir)? else {
		return Ok(None);
	};
	MigrationExpansion::load(item, kind, layout, &manifest, &program_dir).map(Some)
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

fn verify_transition_files(
	item: &ItemStruct,
	program_dir: &std::path::Path,
	history: &ContractHistory,
) -> syn::Result<()> {
	for version in history.versions.iter().skip(1) {
		let transition = version.transition.as_ref().ok_or_else(|| {
			syn::Error::new_spanned(
				item,
				format!(
					"migration version {} has no adjacent transition",
					version.version
				),
			)
		})?;
		let path = program_dir.join(pina_abi::transition_path(
			&history.identity,
			transition.from,
			transition.to,
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
	discriminator_bytes: u8,
	version_bytes: usize,
	visibility: &syn::Visibility,
) -> syn::Result<proc_macro2::TokenStream> {
	let name = historical_struct_name(struct_name, version.version);
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
	let (attribute, proof) = match version.schema.layout {
		LayoutKind::Fixed => {
			let PhysicalLayout::Fixed { size, .. } = &version.schema.physical else {
				return Err(syn::Error::new_spanned(
					struct_name,
					"historical fixed schema has a non-fixed physical descriptor",
				));
			};
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
		LayoutKind::Compact => {
			let PhysicalLayout::Compact {
				header_size,
				maximum_size,
				tail_alignment,
				..
			} = &version.schema.physical
			else {
				return Err(syn::Error::new_spanned(
					struct_name,
					"historical compact schema has a non-compact physical descriptor",
				));
			};
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
				version: 0,
				schema_sha256: schema.sha256(),
				schema,
				process: None,
				process_sha256: None,
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

	fn write_manifest(program_dir: &std::path::Path, source: &[u8]) -> PathBuf {
		let path = program_dir.join(pina_abi::MANIFEST_PATH);
		let parent = path
			.parent()
			.unwrap_or_else(|| panic!("manifest path has a parent directory"));
		std::fs::create_dir_all(parent)
			.unwrap_or_else(|error| panic!("create manifest directory: {error}"));
		std::fs::write(&path, source).unwrap_or_else(|error| panic!("write manifest: {error}"));
		path
	}

	fn encode(manifest: &MigrationManifest) -> Vec<u8> {
		pina_abi::encode_manifest_for_format(manifest, pina_abi::MANIFEST_FORMAT_VERSION)
			.unwrap_or_else(|error| panic!("encode manifest: {error}"))
	}

	#[test]
	fn read_manifest_reports_the_absent_program_manifest() {
		let item = item_struct("State");
		let (manifest, program_dir) =
			read_manifest(&item).unwrap_or_else(|error| panic!("read absent manifest: {error}"));

		assert!(manifest.is_none());
		assert_eq!(program_dir, PathBuf::from(env!("CARGO_MANIFEST_DIR")));
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

		// A future format is rejected before the typed model is read.
		write_manifest(temp.path(), br#"{"formatVersion": 99}"#);
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
		// so the remedy is reported instead of a silent "no policy".
		let path = temp.path().join(pina_abi::MANIFEST_PATH);
		std::fs::create_dir_all(&path)
			.unwrap_or_else(|error| panic!("create manifest directory: {error}"));

		let error =
			read_manifest_at(&item, temp.path()).expect_err("a directory cannot be a manifest");
		let message = error.to_string();
		assert!(message.contains("could not be read"), "message: {message}");
		assert!(
			message.contains("pina migrations make"),
			"message: {message}"
		);
		assert!(
			message.contains(&path.display().to_string()),
			"message: {message}"
		);
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
			message.contains(
				&program_dir
					.join(pina_abi::MANIFEST_PATH)
					.display()
					.to_string()
			),
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
}
