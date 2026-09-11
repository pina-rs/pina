//! Language-neutral CLI model extracted from a Codama [`RootNode`].
//!
//! The model captures everything the CLI emitters need: one command per
//! instruction with typed argument flags, account resolution rules (constant,
//! PDA-derived, payer, or required), fetch commands per state account, and the
//! program error table.

use codama_nodes::AccountNode;
use codama_nodes::ConstantPdaSeedNode;
use codama_nodes::CountNode;
use codama_nodes::DefaultValueStrategy;
use codama_nodes::Endianness;
use codama_nodes::ErrorNode;
use codama_nodes::HasKind;
use codama_nodes::InstructionAccountNode;
use codama_nodes::InstructionArgumentNode;
use codama_nodes::InstructionInputValueNode;
use codama_nodes::InstructionNode;
use codama_nodes::IsSigner;
use codama_nodes::NestedTypeNodeTrait;
use codama_nodes::Number;
use codama_nodes::NumberFormat;
use codama_nodes::NumberTypeNode;
use codama_nodes::NumberValueNode;
use codama_nodes::PdaLinkNode;
use codama_nodes::PdaNode;
use codama_nodes::PdaSeedNode;
use codama_nodes::PdaSeedValueNode;
use codama_nodes::PdaSeedValueValue;
use codama_nodes::PdaValueNode;
use codama_nodes::ProgramNode;
use codama_nodes::PublicKeyValueNode;
use codama_nodes::RootNode;
use codama_nodes::SizePrefixTypeNode;
use codama_nodes::StringTypeNode;
use codama_nodes::StringValueNode;
use codama_nodes::StructFieldTypeNode;
use codama_nodes::TypeNode;
use codama_nodes::VariablePdaSeedNode;
use heck::ToKebabCase;
use heck::ToLowerCamelCase;
use heck::ToSnakeCase;
use heck::ToUpperCamelCase;

use crate::error::RenderError;
use crate::error::Result;

/// A rendered CLI application model for one program.
#[derive(Debug, Clone)]
pub struct CliModel {
	/// Program name in camelCase, e.g. `counterProgram`.
	pub program_camel: String,
	/// Program name in `snake_case`, e.g. `counter_program`.
	pub program_snake: String,
	/// Binary name in kebab-case with a `-cli` suffix, e.g. `counter-program-cli`.
	pub bin_name: String,
	/// Cargo package name, e.g. `counter_program_cli`.
	pub package_name: String,
	/// On-chain program address from the IDL.
	pub program_address: String,
	/// Program version from the IDL.
	pub program_version: String,
	/// Single-line description built from the program docs or name.
	pub about: String,
	pub instructions: Vec<InstructionModel>,
	pub accounts: Vec<AccountModel>,
	pub errors: Vec<ErrorModel>,
}

/// One instruction subcommand.
#[derive(Debug, Clone)]
pub struct InstructionModel {
	/// Instruction name in `snake_case`, e.g. `initialize`.
	pub snake: String,
	/// Instruction name in `PascalCase`, e.g. `Initialize`.
	pub pascal: String,
	pub docs: Vec<String>,
	/// Client accounts-struct identifier, e.g. `Initialize`.
	pub accounts_struct: String,
	/// Client instruction-data identifier, e.g. `InitializeInstructionData`.
	pub data_ident: String,
	pub args: Vec<ArgModel>,
	pub accounts: Vec<AccountRefModel>,
}

impl InstructionModel {
	/// Whether any argument survives discriminator omission.
	#[must_use]
	pub fn has_data_args(&self) -> bool {
		!self.args.is_empty()
	}
}

/// A typed instruction-argument flag.
#[derive(Debug, Clone)]
pub struct ArgModel {
	/// Argument name in `snake_case`; clap derives the kebab-case flag from it.
	pub snake: String,
	pub docs: Vec<String>,
	pub kind: ArgKind,
}

/// The argument shapes a generated CLI can express as flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgKind {
	/// Fixed-width number with its Rust type.
	Number {
		rust_ty: &'static str,
	},
	Bool,
	/// A base58 public key carried in instruction data.
	Pubkey,
	/// A fixed-size byte array rendered from a base58 flag.
	FixedBytes {
		size: usize,
	},
	/// A capacity-bound byte vector rendered from a base58 flag.
	ByteVec {
		capacity: usize,
	},
	/// A capacity-bound string (`pina::String<capacity>`).
	FixedString {
		capacity: usize,
	},
}

/// How a CLI resolves one instruction account.
#[derive(Debug, Clone)]
pub struct AccountRefModel {
	/// Account name in `snake_case`; clap derives the kebab-case flag from it.
	pub snake: String,
	pub docs: Vec<String>,
	pub is_optional: bool,
	pub resolution: Resolution,
}

/// Account resolution strategy.
#[derive(Debug, Clone)]
pub enum Resolution {
	/// A constant address baked into the IDL.
	Constant(String),
	/// A PDA derived from seeds; the flag overrides the derived address.
	Pda { seeds: Vec<SeedModel> },
	/// Defaults to the payer keypair; the flag overrides it.
	Payer,
	/// Always provided through an explicit flag.
	Required,
}

/// One PDA seed expression.
#[derive(Debug, Clone)]
pub enum SeedModel {
	/// A UTF-8 constant seed.
	Utf8(String),
	/// A base58 constant seed.
	ConstPubkey(String),
	/// A little-endian constant number seed.
	ConstNumber { value: i128, rust_ty: &'static str },
	/// A seed taken from a previously resolved account (always a pubkey).
	Account(String),
	/// A seed taken from an instruction argument flag.
	Arg { arg: String, kind: ArgKind },
}

/// One fetchable state account.
#[derive(Debug, Clone)]
pub struct AccountModel {
	/// Account name in `snake_case`, e.g. `counter_state`.
	pub snake: String,
	/// Command name in kebab-case, e.g. `counter-state`.
	pub kebab: String,
	/// Client struct identifier, e.g. `CounterState`.
	pub pascal: String,
	pub docs: Vec<String>,
	/// PDA derivation seeds when the account is a PDA.
	pub seeds: Option<Vec<FetchSeedModel>>,
	/// Data fields rendered by `fetch` output.
	pub fields: Vec<FieldModel>,
	/// Compact accounts decode into `<Pascal>Ref` views instead of `<Pascal>Zc`.
	pub compact: bool,
}

/// One seed of a `fetch` PDA derivation: a fixed constant or a variable flag.
#[derive(Debug, Clone)]
pub struct FetchSeedModel {
	/// Seed name in `snake_case` (also the flag name for variables).
	pub snake: String,
	pub kind: ArgKind,
	/// Fixed constant seeds need no flag.
	pub constant: Option<SeedModel>,
}

/// One account data field.
#[derive(Debug, Clone)]
pub struct FieldModel {
	/// Field name in `snake_case`.
	pub snake: String,
	pub kind: FieldKind,
}

/// Accessor shape of one account data field.
#[derive(Debug, Clone)]
pub enum FieldKind {
	/// Plain in the zero-copy view (`u8`/`i8`) or pod-wrapped.
	Number {
		plain: bool,
	},
	Bool,
	Pubkey,
	/// Capacity-bound string, accessed through `as_str()`.
	String,
	/// Inline vector; elements are plain for `u8`/`i8`, pod-wrapped otherwise.
	Vec {
		plain_element: bool,
	},
	/// Fixed-size byte array, printed as lowercase hex.
	FixedBytes,
	/// Optional pod value.
	Option,
	/// Optional capacity-bound string.
	OptionString,
}

/// One program error entry.
#[derive(Debug, Clone)]
pub struct ErrorModel {
	pub pascal: String,
	pub code: u32,
	#[allow(dead_code)] // reserved for future error-decoding surfaces
	pub message: String,
}

impl CliModel {
	/// Extract the CLI model from a Codama root node.
	///
	/// # Errors
	///
	/// Returns an error when the IDL uses argument, account, or seed shapes the
	/// generated CLIs cannot express.
	pub fn from_root(root: &RootNode) -> Result<Self> {
		let program = &root.program;
		let program_camel = program.name.as_ref().to_lower_camel_case();
		let program_snake = program_camel.to_snake_case();

		let instructions = program
			.instructions
			.iter()
			.map(|instruction| instruction_model(instruction, program))
			.collect::<Result<Vec<_>>>()?;

		let accounts = program
			.accounts
			.iter()
			.map(|account| account_model(account, program))
			.collect::<Result<Vec<_>>>()?;

		let errors = program.errors.iter().map(error_model).collect::<Vec<_>>();

		Ok(Self {
			about: about_for(program),
			bin_name: format!("{}-cli", program_snake.to_kebab_case()),
			package_name: format!("{program_snake}_cli"),
			program_camel,
			program_snake,
			program_address: program.public_key.clone(),
			program_version: program.version.clone(),
			instructions,
			accounts,
			errors,
		})
	}

	/// Whether the program declares at least one command surface.
	#[must_use]
	pub fn has_commands(&self) -> bool {
		!self.instructions.is_empty() || !self.accounts.is_empty()
	}
}

fn about_for(program: &ProgramNode) -> String {
	program
		.docs
		.iter()
		.find(|line| !line.trim().is_empty())
		.map_or_else(
			|| format!("Command-line interface for {}", program.name.as_ref()),
			|line| line.trim_end_matches('.').to_string(),
		)
}

fn instruction_model(
	instruction: &InstructionNode,
	program: &ProgramNode,
) -> Result<InstructionModel> {
	let pascal = instruction.name.as_ref().to_upper_camel_case();
	let snake = pascal.to_snake_case();
	let context = format!("instruction `{pascal}`");

	let args = instruction
		.arguments
		.iter()
		.filter(|argument| {
			argument.default_value_strategy != Some(DefaultValueStrategy::Omitted)
				|| argument.default_value.is_none()
		})
		.map(|argument| argument_model(argument, &context))
		.collect::<Result<Vec<_>>>()?;

	let mut accounts = Vec::new();
	for account in &instruction.accounts {
		accounts.push(account_ref_model(
			account, &accounts, &args, program, &context,
		)?);
	}

	Ok(InstructionModel {
		data_ident: format!("{pascal}InstructionData"),
		accounts_struct: pascal.clone(),
		pascal,
		snake,
		docs: instruction.docs.iter().cloned().collect(),
		args,
		accounts,
	})
}

fn argument_model(argument: &InstructionArgumentNode, context: &str) -> Result<ArgModel> {
	let pascal = argument.name.as_ref().to_upper_camel_case();
	let snake = pascal.to_snake_case();

	Ok(ArgModel {
		kind: arg_kind(&argument.r#type, context)?,
		docs: argument.docs.iter().cloned().collect(),
		snake,
	})
}

fn arg_kind(type_node: &TypeNode, context: &str) -> Result<ArgKind> {
	match type_node {
		TypeNode::Number(number) => {
			little_endian(number, context).and_then(|()| {
				number_kind(number.format)
					.map(|rust_ty| ArgKind::Number { rust_ty })
					.ok_or_else(|| unsupported_number(number.format, context))
			})
		}
		TypeNode::Boolean(_) => Ok(ArgKind::Bool),
		TypeNode::PublicKey(_) => Ok(ArgKind::Pubkey),
		TypeNode::FixedSize(fixed_size) => {
			match fixed_size.r#type.as_ref() {
				TypeNode::Bytes(_) => {
					Ok(ArgKind::FixedBytes {
						size: fixed_size.size,
					})
				}
				TypeNode::SizePrefix(SizePrefixTypeNode { r#type, .. }) => {
					match r#type.as_ref() {
						TypeNode::String(StringTypeNode { .. }) => {
							Ok(ArgKind::FixedString {
								capacity: fixed_size.size,
							})
						}
						other => {
							Err(unsupported(
								other,
								context,
								"only UTF-8 size-prefixed strings",
							))
						}
					}
				}
				TypeNode::Array(array) => {
					let TypeNode::Number(NumberTypeNode { format, endian, .. }) =
						array.item.as_ref()
					else {
						return Err(unsupported(
							array.item.as_ref(),
							context,
							"only number items",
						));
					};
					if !matches!(format, NumberFormat::U8) {
						return Err(unsupported(array.item.as_ref(), context, "only u8 items"));
					}
					little_endian_endian(*endian, context)?;
					match array.count.as_ref() {
						CountNode::Fixed(count) => {
							Ok(ArgKind::FixedBytes {
								size: usize::try_from(count.value).map_err(|_| {
									unsupported(
										type_node,
										context,
										"an item count that fits in usize",
									)
								})?,
							})
						}
						CountNode::Prefixed(_) => {
							Ok(ArgKind::ByteVec {
								capacity: fixed_size.size,
							})
						}
						CountNode::Remainder(_) => {
							Err(unsupported(type_node, context, "a bounded item count"))
						}
					}
				}
				other => {
					Err(unsupported(
						other,
						context,
						"bytes, size-prefixed strings, or fixed u8 arrays",
					))
				}
			}
		}
		other => {
			Err(unsupported(
				other,
				context,
				"numbers, booleans, pubkeys, fixed bytes, or fixed strings",
			))
		}
	}
}

fn little_endian(number: &NumberTypeNode, context: &str) -> Result<()> {
	little_endian_endian(number.endian, context)
}

fn little_endian_endian(endian: Endianness, context: &str) -> Result<()> {
	if matches!(endian, Endianness::Le) {
		Ok(())
	} else {
		Err(RenderError::UnsupportedIdl {
			context: context.to_string(),
			reason: "only little-endian numbers are supported".to_string(),
		})
	}
}

fn number_kind(format: NumberFormat) -> Option<&'static str> {
	match format {
		NumberFormat::U8 => Some("u8"),
		NumberFormat::U16 => Some("u16"),
		NumberFormat::U32 => Some("u32"),
		NumberFormat::U64 => Some("u64"),
		NumberFormat::I8 => Some("i8"),
		NumberFormat::I16 => Some("i16"),
		NumberFormat::I32 => Some("i32"),
		NumberFormat::I64 => Some("i64"),
		NumberFormat::ShortU16
		| NumberFormat::U128
		| NumberFormat::I128
		| NumberFormat::F32
		| NumberFormat::F64 => None,
	}
}

fn unsupported(type_node: &TypeNode, context: &str, reason: &str) -> RenderError {
	RenderError::UnsupportedIdl {
		context: context.to_string(),
		reason: format!("type `{}` is not supported ({reason})", type_node.kind()),
	}
}

fn unsupported_number(format: NumberFormat, context: &str) -> RenderError {
	RenderError::UnsupportedIdl {
		context: context.to_string(),
		reason: format!("number format `{format:?}` is not supported"),
	}
}

fn account_ref_model(
	account: &InstructionAccountNode,
	resolved: &[AccountRefModel],
	args: &[ArgModel],
	program: &ProgramNode,
	context: &str,
) -> Result<AccountRefModel> {
	let snake = account.name.as_ref().to_snake_case();

	let resolution = match account.default_value.as_ref().as_ref() {
		Some(InstructionInputValueNode::PublicKeyValue(PublicKeyValueNode {
			public_key, ..
		})) => Resolution::Constant(public_key.clone()),
		Some(InstructionInputValueNode::PayerValue(_)) => Resolution::Payer,
		Some(InstructionInputValueNode::PdaValue(PdaValueNode { pda, seeds, .. })) => {
			Resolution::Pda {
				seeds: pda_seeds(pda, seeds, resolved, args, program, context)?,
			}
		}
		Some(other) => {
			return Err(RenderError::UnsupportedIdl {
				context: context.to_string(),
				reason: format!(
					"account default `{}` is not supported; use a constant, PDA, or payer default",
					other.kind(),
				),
			});
		}
		None if matches!(account.is_signer, IsSigner::True) => Resolution::Payer,
		None => Resolution::Required,
	};

	Ok(AccountRefModel {
		is_optional: account.is_optional.unwrap_or_default(),
		resolution,
		docs: account.docs.iter().cloned().collect(),
		snake,
	})
}

/// Build the ordered seed list for a PDA default by walking the linked
/// [`PdaNode`] and resolving variable seeds through the provided values.
fn pda_seeds(
	pda: &codama_nodes::PdaValuePda,
	values: &[PdaSeedValueNode],
	resolved: &[AccountRefModel],
	args: &[ArgModel],
	program: &ProgramNode,
	context: &str,
) -> Result<Vec<SeedModel>> {
	let pda_node: &PdaNode = match pda {
		codama_nodes::PdaValuePda::Pda(node) => node,
		codama_nodes::PdaValuePda::PdaLink(link) => {
			program
				.pdas
				.iter()
				.find(|candidate| candidate.name == link.name)
				.ok_or_else(|| {
					RenderError::UnsupportedIdl {
						context: context.to_string(),
						reason: format!("links to missing PDA `{}`", link.name.as_ref()),
					}
				})?
		}
	};

	let mut seeds = Vec::new();
	for seed in &pda_node.seeds {
		match seed {
			PdaSeedNode::Constant(ConstantPdaSeedNode { value, .. }) => {
				seeds.push(constant_seed(value, context)?);
			}
			PdaSeedNode::Variable(VariablePdaSeedNode { name, .. }) => {
				let value = values
					.iter()
					.find(|value| &value.name == name)
					.ok_or_else(|| {
						RenderError::UnsupportedIdl {
							context: context.to_string(),
							reason: format!("PDA seed `{}` has no provided value", name.as_ref()),
						}
					})?;
				seeds.push(seed_model(value, resolved, args, context)?);
			}
		}
	}
	Ok(seeds)
}

/// Map a constant PDA seed value to its rendered form.
fn constant_seed(value: &codama_nodes::ConstantPdaSeedValue, context: &str) -> Result<SeedModel> {
	use codama_nodes::ConstantPdaSeedValue;

	match value {
		ConstantPdaSeedValue::String(StringValueNode { string }) => {
			Ok(SeedModel::Utf8(string.clone()))
		}
		ConstantPdaSeedValue::PublicKey(PublicKeyValueNode { public_key, .. }) => {
			Ok(SeedModel::ConstPubkey(public_key.clone()))
		}
		ConstantPdaSeedValue::Number(NumberValueNode {
			number: Number::UnsignedInteger(value),
		}) => {
			Ok(SeedModel::ConstNumber {
				value: i128::from(*value),
				rust_ty: "u64",
			})
		}
		ConstantPdaSeedValue::Number(NumberValueNode {
			number: Number::SignedInteger(value),
		}) => {
			Ok(SeedModel::ConstNumber {
				value: i128::from(*value),
				rust_ty: "i64",
			})
		}
		other => {
			Err(RenderError::UnsupportedIdl {
				context: context.to_string(),
				reason: format!("constant PDA seed `{}` is not supported", other.kind(),),
			})
		}
	}
}

fn seed_model(
	seed: &PdaSeedValueNode,
	resolved: &[AccountRefModel],
	args: &[ArgModel],
	context: &str,
) -> Result<SeedModel> {
	let PdaSeedValueNode { name, value } = seed;
	let seed_name = name.as_ref().to_snake_case();

	match value.as_ref() {
		PdaSeedValueValue::Account(account) => {
			let account_snake = account.name.as_ref().to_snake_case();
			if !resolved
				.iter()
				.any(|resolved| resolved.snake == account_snake)
			{
				return Err(RenderError::UnsupportedIdl {
					context: context.to_string(),
					reason: format!(
						"PDA seed `{seed_name}` depends on account `{account_snake}`, which must \
						 be declared first"
					),
				});
			}
			Ok(SeedModel::Account(account_snake))
		}
		PdaSeedValueValue::Argument(argument) => {
			let arg_snake = argument.name.as_ref().to_snake_case();
			let kind = args
				.iter()
				.find(|arg| arg.snake == arg_snake)
				.map(|arg| arg.kind.clone())
				.filter(|kind| matches!(kind, ArgKind::Pubkey | ArgKind::Number { .. }))
				.ok_or_else(|| {
					RenderError::UnsupportedIdl {
						context: context.to_string(),
						reason: format!(
							"PDA seed `{seed_name}` must reference a pubkey or number argument"
						),
					}
				})?;
			Ok(SeedModel::Arg {
				arg: arg_snake,
				kind,
			})
		}
		PdaSeedValueValue::String(StringValueNode { string }) => {
			Ok(SeedModel::Utf8(string.clone()))
		}
		PdaSeedValueValue::PublicKey(PublicKeyValueNode { public_key, .. }) => {
			Ok(SeedModel::ConstPubkey(public_key.clone()))
		}
		PdaSeedValueValue::Number(NumberValueNode {
			number: Number::UnsignedInteger(value),
		}) => {
			Ok(SeedModel::ConstNumber {
				value: i128::from(*value),
				rust_ty: "u64",
			})
		}
		PdaSeedValueValue::Number(NumberValueNode {
			number: Number::SignedInteger(value),
		}) => {
			Ok(SeedModel::ConstNumber {
				value: i128::from(*value),
				rust_ty: "i64",
			})
		}
		other => {
			Err(RenderError::UnsupportedIdl {
				context: context.to_string(),
				reason: format!(
					"PDA seed `{seed_name}` uses an unsupported `{}` value",
					PdaSeedValueValue::kind(other),
				),
			})
		}
	}
}

fn account_model(account: &AccountNode, program: &ProgramNode) -> Result<AccountModel> {
	let pascal = account.name.as_ref().to_upper_camel_case();
	let snake = pascal.to_snake_case();

	let seeds = account
		.pda
		.as_ref()
		.map(|PdaLinkNode { name, .. }| {
			program
				.pdas
				.iter()
				.find(|pda| pda.name == *name)
				.ok_or_else(|| {
					RenderError::UnsupportedIdl {
						context: format!("account `{pascal}`"),
						reason: format!("links to missing PDA `{}`", name.as_ref()),
					}
				})
				.and_then(|pda| fetch_seeds(pda, &pascal))
		})
		.transpose()?;

	Ok(AccountModel {
		kebab: snake.to_kebab_case(),
		fields: data_fields(account, &pascal)?,
		compact: account
			.data
			.get_nested_type_node()
			.fields
			.iter()
			.any(|field| is_compact_tail(&field.r#type)),
		seeds,
		pascal,
		docs: account.docs.iter().cloned().collect(),
		snake,
	})
}

fn fetch_seeds(pda: &PdaNode, context: &str) -> Result<Vec<FetchSeedModel>> {
	let mut seeds = Vec::new();
	for seed in &pda.seeds {
		match seed {
			PdaSeedNode::Constant(ConstantPdaSeedNode { value, .. }) => {
				seeds.push(FetchSeedModel {
					snake: String::default(),
					kind: ArgKind::Pubkey,
					constant: Some(constant_seed(value, context)?),
				});
			}
			PdaSeedNode::Variable(VariablePdaSeedNode { name, r#type, .. }) => {
				seeds.push(FetchSeedModel {
					snake: name.as_ref().to_snake_case(),
					kind: arg_kind(r#type, context)?,
					constant: None,
				});
			}
		}
	}
	Ok(seeds)
}

/// Mirror of the client renderer's compact-tail detection so fetch decodes
/// through the same view type (`<Pascal>Ref`) the client exports.
fn is_compact_tail(type_node: &TypeNode) -> bool {
	fn core(type_node: &TypeNode) -> &TypeNode {
		match type_node {
			TypeNode::PreOffset(offset) => core(&offset.r#type),
			TypeNode::PostOffset(offset) => core(&offset.r#type),
			other => other,
		}
	}

	match core(type_node) {
		TypeNode::Array(array) => {
			matches!(
				array.count.as_ref(),
				CountNode::Prefixed(_) | CountNode::Remainder(_)
			) && !matches!(type_node, TypeNode::FixedSize(_))
		}
		TypeNode::SizePrefix(SizePrefixTypeNode { r#type, .. }) => {
			matches!(r#type.as_ref(), TypeNode::String(StringTypeNode { .. }))
				&& !matches!(type_node, TypeNode::FixedSize(_))
		}
		TypeNode::Option(option) if option.fixed != Some(true) => {
			matches!(option.item.as_ref(), TypeNode::Array(array)
				if matches!(array.count.as_ref(), CountNode::Prefixed(_) | CountNode::Remainder(_)))
				|| matches!(option.item.as_ref(), TypeNode::SizePrefix(SizePrefixTypeNode { r#type, .. })
					if matches!(r#type.as_ref(), TypeNode::String(StringTypeNode { .. })))
		}
		_ => false,
	}
}

fn data_fields(account: &AccountNode, pascal: &str) -> Result<Vec<FieldModel>> {
	let context = format!("account `{pascal}`");
	let data_type = account.data.get_nested_type_node();

	data_type
		.fields
		.iter()
		.filter(|field| field.default_value_strategy != Some(DefaultValueStrategy::Omitted))
		.map(|StructFieldTypeNode { name, r#type, .. }| {
			Ok(FieldModel {
				snake: name.as_ref().to_snake_case(),
				kind: field_kind(r#type, &context)?,
			})
		})
		.collect()
}

fn field_kind(type_node: &TypeNode, context: &str) -> Result<FieldKind> {
	match type_node {
		TypeNode::PreOffset(offset) => field_kind(&offset.r#type, context),
		TypeNode::PostOffset(offset) => field_kind(&offset.r#type, context),
		TypeNode::SizePrefix(SizePrefixTypeNode { r#type, .. })
			if matches!(r#type.as_ref(), TypeNode::String(StringTypeNode { .. })) =>
		{
			Ok(FieldKind::String)
		}
		TypeNode::Number(number) => {
			little_endian(number, context).and_then(|()| {
				number_kind(number.format)
					.map(|rust_ty| {
						FieldKind::Number {
							plain: matches!(rust_ty, "u8" | "i8"),
						}
					})
					.ok_or_else(|| unsupported_number(number.format, context))
			})
		}
		TypeNode::Boolean(_) => Ok(FieldKind::Bool),
		TypeNode::PublicKey(_) => Ok(FieldKind::Pubkey),
		TypeNode::FixedSize(fixed_size) => {
			match fixed_size.r#type.as_ref() {
				TypeNode::Bytes(_) => Ok(FieldKind::FixedBytes),
				TypeNode::SizePrefix(SizePrefixTypeNode { r#type, .. }) => {
					match r#type.as_ref() {
						TypeNode::String(StringTypeNode { .. }) => Ok(FieldKind::String),
						other => {
							Err(unsupported(
								other,
								context,
								"only UTF-8 size-prefixed strings",
							))
						}
					}
				}
				TypeNode::Array(array) => {
					let TypeNode::Number(NumberTypeNode { format, endian, .. }) =
						array.item.as_ref()
					else {
						return Err(unsupported(
							array.item.as_ref(),
							context,
							"only number elements",
						));
					};
					little_endian_endian(*endian, context)?;
					number_kind(*format).ok_or_else(|| unsupported_number(*format, context))?;
					let rust_ty =
						number_kind(*format).ok_or_else(|| unsupported_number(*format, context))?;
					match array.count.as_ref() {
						CountNode::Fixed(_) | CountNode::Prefixed(_) => {
							Ok(FieldKind::Vec {
								plain_element: matches!(rust_ty, "u8" | "i8"),
							})
						}
						CountNode::Remainder(_) => {
							Err(unsupported(type_node, context, "a bounded item count"))
						}
					}
				}
				other => {
					Err(unsupported(
						other,
						context,
						"only strings, bytes, or vectors",
					))
				}
			}
		}
		TypeNode::Option(option) => {
			match option.item.as_ref() {
				TypeNode::Number(NumberTypeNode { format, endian, .. }) => {
					little_endian_endian(*endian, context)?;
					number_kind(*format)
						.map(|_| FieldKind::Option)
						.ok_or_else(|| unsupported_number(*format, context))
				}
				TypeNode::SizePrefix(SizePrefixTypeNode { r#type, .. })
					if matches!(r#type.as_ref(), TypeNode::String(StringTypeNode { .. })) =>
				{
					Ok(FieldKind::OptionString)
				}
				TypeNode::PreOffset(offset)
					if matches!(
						offset.r#type.as_ref(),
						TypeNode::SizePrefix(SizePrefixTypeNode {
							r#type: prefix_type,
							..
						}) if matches!(prefix_type.as_ref(), TypeNode::String(StringTypeNode { .. }))
					) =>
				{
					Ok(FieldKind::OptionString)
				}
				other => {
					Err(unsupported(
						other,
						context,
						"only optional numbers and strings are supported",
					))
				}
			}
		}
		TypeNode::Array(array) => {
			let TypeNode::Number(NumberTypeNode { format, endian, .. }) = array.item.as_ref()
			else {
				return Err(unsupported(
					array.item.as_ref(),
					context,
					"only number elements",
				));
			};
			little_endian_endian(*endian, context)?;
			let rust_ty =
				number_kind(*format).ok_or_else(|| unsupported_number(*format, context))?;
			if !matches!(
				array.count.as_ref(),
				CountNode::Fixed(_) | CountNode::Prefixed(_)
			) {
				return Err(unsupported(type_node, context, "a bounded item count"));
			}
			Ok(FieldKind::Vec {
				plain_element: matches!(rust_ty, "u8" | "i8"),
			})
		}
		other => {
			Err(unsupported(
				other,
				context,
				"numbers, booleans, pubkeys, strings, options, or vectors",
			))
		}
	}
}

fn error_model(error: &ErrorNode) -> ErrorModel {
	ErrorModel {
		pascal: error.name.as_ref().to_upper_camel_case(),
		code: error.code,
		message: error.message.clone(),
	}
}
