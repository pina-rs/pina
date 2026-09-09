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
	) -> syn::Result<Self> {
		let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR").ok_or_else(|| {
			syn::Error::new_spanned(
				item,
				"could not locate Cargo manifest for migration-aware schema",
			)
		})?;
		let program_dir = PathBuf::from(manifest_dir);
		let path = program_dir.join(MANIFEST_PATH);
		let source = std::fs::read(&path).map_err(|error| {
			syn::Error::new_spanned(
				item,
				format!(
					"{} is marked `migrations`, but {} could not be read ({error}); run `pina \
					 migrations make`",
					item.ident,
					path.display()
				),
			)
		})?;
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
		let history = manifest
			.contract_for_source(kind, &item.ident.to_string())
			.map_err(|error| syn::Error::new_spanned(item, error))?;
		let current = history.current().ok_or_else(|| {
			syn::Error::new_spanned(item, "migration contract contains no current schema")
		})?;
		let source_schema = pina_abi::data_schema(item, layout)
			.map_err(|error| syn::Error::new_spanned(item, error))?;
		verify_source_schema(item, &source_schema, &current.schema)?;
		verify_transition_files(item, &program_dir, history)?;
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
	) -> syn::Result<Option<proc_macro2::TokenStream>> {
		if self
			.history
			.versions
			.iter()
			.any(|version| version.schema.layout != LayoutKind::Fixed)
		{
			return self
				.variable_account_implementation(crate_path, struct_name)
				.map(Some);
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
						#current => {
							if data.len() != #current_size {
								return Err(#crate_path::ProgramError::InvalidAccountData);
							}
							<Self as #crate_path::PinaAccount>::validate_account_data(data)
						}
						_ => Err(#crate_path::PinaProgramError::InvalidMigrationVersion.into()),
					}
				}
			}
		}))
	}

	/// Generate one adjacent, allocator-free migration step at a time when any
	/// historical representation is compact. The executor repeats this contract
	/// atomically until it reaches the current version.
	fn variable_account_implementation(
		&self,
		crate_path: &syn::Path,
		struct_name: &syn::Ident,
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
						quote! {
							<Self as #crate_path::PinaAccount>::validate_account_data(data)
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
) -> syn::Result<proc_macro2::TokenStream> {
	let name = historical_struct_name(struct_name, version.version);
	let discriminator_bytes = usize::from(discriminator_bytes);
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
			Ok(quote!(#name: #ty))
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
		#[derive(#crate_path::pinapod::PinaPod)]
		#attribute
		struct #name {
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
