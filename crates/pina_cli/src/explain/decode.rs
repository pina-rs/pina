//! Error-code decoding and program-log analysis.

use serde::Serialize;

use super::transaction::InstructionError;
use crate::parse::error_enum::DeclaredError;

/// First custom code of the range Pina reserves for `PinaProgramError`.
///
/// Mirrors `pina::RESERVED_ERROR_CODE_START`; a test keeps the two equal.
pub(crate) const RESERVED_ERROR_CODE_START: u32 = 0xFFFF_0000;

/// `PinaProgramError::DuplicateMutableAccount`.
pub(crate) const DUPLICATE_MUTABLE_ACCOUNT: u32 = 0xFFFF_FFF9;
/// `PinaProgramError::InvalidAccountSize`.
pub(crate) const INVALID_ACCOUNT_SIZE: u32 = 0xFFFF_FFFB;
/// `PinaProgramError::TooManyAccountKeys`.
pub(crate) const TOO_MANY_ACCOUNT_KEYS: u32 = 0xFFFF_FFFE;
/// `PinaProgramError::InvalidDiscriminator`.
pub(crate) const INVALID_DISCRIMINATOR: u32 = 0xFFFF_FFFF;

/// Every `PinaProgramError` variant: name, code, and the first line of its
/// documentation.
///
/// The CLI decodes transactions without linking the on-chain crate, so the table
/// is a copy. `pina_program_errors_match_the_framework` checks each entry
/// against `pina::PinaProgramError` and checks that the table lists every
/// variant declared in `crates/pina/src/error.rs`.
pub(crate) const PINA_PROGRAM_ERRORS: &[(&str, u32, &str)] = &[
	(
		"StoredBumpMismatch",
		0xFFFF_FFF0,
		"A compact creation patch stores a PDA bump that disagrees with the canonical bump the \
		 creation validated.",
	),
	(
		"UnverifiedTransfer",
		0xFFFF_FFF1,
		"A verified CPI did not move the balance it was asked to move.",
	),
	(
		"MigrationLamportBudgetExceeded",
		0xFFFF_FFF2,
		"The rent funding a migration needs exceeds this invocation's lamport budget.",
	),
	(
		"MigrationAccountGrowthExceeded",
		0xFFFF_FFF3,
		"One inline migration step would grow the account past the runtime realloc limit.",
	),
	(
		"MigrationWorkspaceExceeded",
		0xFFFF_FFF4,
		"The caller-owned migration workspace cannot hold the transition.",
	),
	(
		"MigrationBudgetExceeded",
		0xFFFF_FFF5,
		"A generated migration exceeds its configured step, growth, or rent budget.",
	),
	(
		"MigrationUnavailable",
		0xFFFF_FFF6,
		"No generated transition can safely satisfy the requested historical contract.",
	),
	(
		"InvalidMigrationVersion",
		0xFFFF_FFF7,
		"A stored version is malformed, unknown, or newer than this program.",
	),
	(
		"MigrationRequired",
		0xFFFF_FFF8,
		"The operation needs a dedicated migration or authorized funding first.",
	),
	(
		"DuplicateMutableAccount",
		DUPLICATE_MUTABLE_ACCOUNT,
		"Two mutable account fields point at the same runtime account.",
	),
	(
		"DataTooShort",
		0xFFFF_FFFA,
		"Account or instruction data is shorter than the expected minimum.",
	),
	(
		"InvalidAccountSize",
		INVALID_ACCOUNT_SIZE,
		"Account size does not match the expected type size.",
	),
	(
		"InvalidTokenOwner",
		0xFFFF_FFFC,
		"Account is not owned by the expected token program.",
	),
	(
		"SeedsTooMany",
		0xFFFF_FFFD,
		"Too many PDA seeds were provided.",
	),
	(
		"TooManyAccountKeys",
		TOO_MANY_ACCOUNT_KEYS,
		"More account keys were provided than the instruction expects.",
	),
	(
		"InvalidDiscriminator",
		INVALID_DISCRIMINATOR,
		"The discriminator bytes do not match any known variant.",
	),
];

/// Which family an instruction error belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ErrorKind {
	/// A built-in runtime error, such as `MissingRequiredSignature`.
	Builtin,
	/// A `PinaProgramError` from the reserved range.
	Pina,
	/// A variant of one of the program's `#[error]` enums.
	Program,
	/// A custom code no known table declares.
	Unknown,
	/// A failure the runtime did not attribute to one instruction.
	Transaction,
}

/// A decoded instruction or transaction error.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecodedError {
	/// Family of the error.
	pub kind: ErrorKind,
	/// Display name, such as `InvalidAccountData` or `ValidationError::InvalidAmount`.
	pub name: String,
	/// The custom code, for custom errors.
	pub code: Option<u32>,
	/// First documentation line of the variant, when known.
	pub description: Option<String>,
}

/// Decode an instruction error against Pina's reserved codes and the program's
/// `#[error]` enums.
pub(crate) fn decode_error(error: &InstructionError, declared: &[DeclaredError]) -> DecodedError {
	let code = match error {
		InstructionError::Builtin(name) => {
			return DecodedError {
				kind: ErrorKind::Builtin,
				name: name.clone(),
				code: None,
				description: None,
			};
		}
		InstructionError::Custom(code) => *code,
	};

	if code >= RESERVED_ERROR_CODE_START {
		let known = PINA_PROGRAM_ERRORS
			.iter()
			.find(|(_, candidate, _)| *candidate == code);

		return DecodedError {
			kind: ErrorKind::Pina,
			name: known.map_or_else(
				|| format!("reserved Pina code {code:#X}"),
				|(name, ..)| format!("PinaProgramError::{name}"),
			),
			code: Some(code),
			description: Some(known.map_or_else(
				|| {
					"This CLI does not know the code; the program may use a newer pina release."
						.to_owned()
				},
				|(_, _, description)| (*description).to_owned(),
			)),
		};
	}

	let matches: Vec<_> = declared
		.iter()
		.filter(|declared| declared.error.code == code)
		.collect();

	if matches.is_empty() {
		return DecodedError {
			kind: ErrorKind::Unknown,
			name: format!("Custom({code})"),
			code: Some(code),
			description: Some(
				"No `#[error]` variant of this program declares the code; it may come from a \
				 program this instruction invoked."
					.to_owned(),
			),
		};
	}

	DecodedError {
		kind: ErrorKind::Program,
		name: matches
			.iter()
			.map(|declared| format!("{}::{}", declared.enum_name, declared.error.name))
			.collect::<Vec<_>>()
			.join(" or "),
		code: Some(code),
		description: matches
			.iter()
			.find_map(|declared| declared.error.docs.first().cloned()),
	}
}

/// The error a check returns, in the form the transaction reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ErrorKey {
	Builtin(String),
	Custom(u32),
	/// A custom error expression that could not be resolved to a code.
	AnyCustom,
}

impl ErrorKey {
	pub(crate) fn builtin(name: &str) -> Self {
		Self::Builtin(name.to_owned())
	}

	pub(crate) fn matches(&self, error: &InstructionError) -> bool {
		match (self, error) {
			(Self::Builtin(expected), InstructionError::Builtin(actual)) => expected == actual,
			(Self::Custom(expected), InstructionError::Custom(actual)) => expected == actual,
			(Self::AnyCustom, InstructionError::Custom(_)) => true,
			_ => false,
		}
	}
}

/// A failure the logs attribute to a program invoked through CPI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpiFailure {
	/// The callee that failed first.
	pub program_id: String,
	/// Invocation depth of the callee; the top-level instruction is depth 1.
	pub depth: usize,
	/// The runtime's failure message for the callee.
	pub message: String,
}

/// What the logs of the failing top-level instruction show.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct LogAnalysis {
	/// Every line the failing instruction logged, in order.
	pub(crate) segment: Vec<String>,
	/// `Program log:` messages the failing program logged itself, outside any
	/// CPI.
	pub(crate) program_messages: Vec<String>,
	pub(crate) cpi_failure: Option<CpiFailure>,
	/// Whether the node cut the logs short.
	pub(crate) truncated: bool,
}

/// Find the failing instruction's logs and the first program that failed.
///
/// Execution stops at the first failing instruction, so its logs start at the
/// last depth-1 invocation of its program. The innermost failure is logged
/// first; each caller then logs its own failure with the same error.
pub(crate) fn analyze_logs(logs: &[String], program_id: &str) -> LogAnalysis {
	let truncated = logs.iter().any(|line| line == "Log truncated");
	let Some(start) = logs
		.iter()
		.rposition(|line| invocation(line) == Some((program_id, 1)))
	else {
		return LogAnalysis {
			truncated,
			..LogAnalysis::default()
		};
	};
	let segment = logs[start..].to_vec();
	let mut stack: Vec<&str> = Vec::new();
	let mut program_messages = Vec::new();
	let mut cpi_failure = None;

	for line in &segment {
		if let Some((invoked, _)) = invocation(line) {
			stack.push(invoked);
			continue;
		}

		if let Some(message) = line.strip_prefix("Program log: ") {
			if stack.len() == 1 {
				program_messages.push(message.to_owned());
			}
			continue;
		}

		if let Some((failed, message)) = failure(line) {
			if stack.len() > 1 && cpi_failure.is_none() {
				cpi_failure = Some(CpiFailure {
					program_id: failed.to_owned(),
					depth: stack.len(),
					message: message.to_owned(),
				});
			}
			stack.pop();
			continue;
		}

		if is_success(line) {
			stack.pop();
		}
	}

	LogAnalysis {
		segment,
		program_messages,
		cpi_failure,
		truncated,
	}
}

/// `Program <id> invoke [<depth>]`.
fn invocation(line: &str) -> Option<(&str, usize)> {
	let (program, depth) = line.strip_prefix("Program ")?.split_once(" invoke [")?;

	Some((program, depth.strip_suffix(']')?.parse().ok()?))
}

/// `Program <id> failed: <message>`.
fn failure(line: &str) -> Option<(&str, &str)> {
	line.strip_prefix("Program ")?.split_once(" failed: ")
}

/// `Program <id> success`.
fn is_success(line: &str) -> bool {
	line.strip_prefix("Program ")
		.and_then(|rest| rest.strip_suffix(" success"))
		.is_some_and(|program| !program.contains([' ', ':']))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::ir::ErrorIr;

	fn declared(enum_name: &str, name: &str, code: u32, docs: &[&str]) -> DeclaredError {
		DeclaredError {
			enum_name: enum_name.to_owned(),
			error: ErrorIr {
				name: name.to_owned(),
				code,
				docs: docs.iter().map(|doc| (*doc).to_owned()).collect(),
			},
		}
	}

	#[test]
	fn decodes_builtin_reserved_program_and_unknown_errors() {
		let program = [
			declared("AppError", "Denied", 6000, &["The caller may not do this."]),
			declared("AppError", "Undocumented", 6001, &[]),
			declared("OtherError", "Shared", 6001, &["Shared code."]),
		];

		let builtin = decode_error(
			&InstructionError::Builtin("InvalidAccountData".to_owned()),
			&program,
		);
		assert_eq!(builtin.kind, ErrorKind::Builtin);
		assert_eq!(builtin.name, "InvalidAccountData");
		assert_eq!(builtin.code, None);

		let reserved = decode_error(&InstructionError::Custom(TOO_MANY_ACCOUNT_KEYS), &program);
		assert_eq!(reserved.kind, ErrorKind::Pina);
		assert_eq!(reserved.name, "PinaProgramError::TooManyAccountKeys");
		assert_eq!(
			reserved.description.as_deref(),
			Some("More account keys were provided than the instruction expects.")
		);

		let future = decode_error(&InstructionError::Custom(0xFFFF_0001), &program);
		assert_eq!(future.kind, ErrorKind::Pina);
		assert_eq!(future.name, "reserved Pina code 0xFFFF0001");
		assert!(
			future
				.description
				.is_some_and(|description| description.contains("newer pina"))
		);

		let user = decode_error(&InstructionError::Custom(6000), &program);
		assert_eq!(user.kind, ErrorKind::Program);
		assert_eq!(user.name, "AppError::Denied");
		assert_eq!(user.code, Some(6000));
		assert_eq!(
			user.description.as_deref(),
			Some("The caller may not do this.")
		);

		let shared = decode_error(&InstructionError::Custom(6001), &program);
		assert_eq!(shared.name, "AppError::Undocumented or OtherError::Shared");
		assert_eq!(shared.description.as_deref(), Some("Shared code."));

		let unknown = decode_error(&InstructionError::Custom(1), &program);
		assert_eq!(unknown.kind, ErrorKind::Unknown);
		assert_eq!(unknown.name, "Custom(1)");
	}

	#[test]
	fn error_keys_match_only_their_own_error() {
		let builtin = InstructionError::Builtin("InvalidSeeds".to_owned());
		let custom = InstructionError::Custom(3);

		assert!(ErrorKey::builtin("InvalidSeeds").matches(&builtin));
		assert!(!ErrorKey::builtin("InvalidAccountData").matches(&builtin));
		assert!(!ErrorKey::builtin("InvalidSeeds").matches(&custom));
		assert!(ErrorKey::Custom(3).matches(&custom));
		assert!(!ErrorKey::Custom(4).matches(&custom));
		assert!(ErrorKey::AnyCustom.matches(&custom));
		assert!(!ErrorKey::AnyCustom.matches(&builtin));
	}

	fn lines(lines: &[&str]) -> Vec<String> {
		lines.iter().map(|line| (*line).to_owned()).collect()
	}

	#[test]
	fn isolates_the_failing_instruction_and_its_own_messages() {
		let logs = lines(&[
			"Program Prog111 invoke [1]",
			"Program log: first instruction",
			"Program Prog111 success",
			"Program Prog111 invoke [1]",
			"Program log: account has not been marked as writable",
			"Program Prog111 consumed 120 of 200000 compute units",
			"Program Prog111 failed: invalid account data for instruction",
		]);
		let analysis = analyze_logs(&logs, "Prog111");

		assert_eq!(analysis.segment.len(), 4);
		assert_eq!(
			analysis.program_messages,
			["account has not been marked as writable"]
		);
		assert_eq!(analysis.cpi_failure, None);
		assert!(!analysis.truncated);
	}

	#[test]
	fn attributes_a_failure_inside_a_cpi_to_the_callee() {
		let logs = lines(&[
			"Program Prog111 invoke [1]",
			"Program log: creating account",
			"Program 11111111111111111111111111111111 invoke [2]",
			"Program log: callee message",
			"Program 11111111111111111111111111111111 success",
			"Program 11111111111111111111111111111111 invoke [2]",
			"Allocate: account Address { address: Abc, base: None } already in use",
			"Program 11111111111111111111111111111111 failed: custom program error: 0x0",
			"Program Prog111 failed: custom program error: 0x0",
		]);
		let analysis = analyze_logs(&logs, "Prog111");

		assert_eq!(analysis.program_messages, ["creating account"]);
		assert_eq!(
			analysis.cpi_failure,
			Some(CpiFailure {
				program_id: "11111111111111111111111111111111".to_owned(),
				depth: 2,
				message: "custom program error: 0x0".to_owned(),
			})
		);
	}

	#[test]
	fn reports_missing_and_truncated_logs() {
		let analysis = analyze_logs(
			&lines(&["Program Other invoke [1]", "Log truncated"]),
			"Prog111",
		);

		assert!(analysis.segment.is_empty());
		assert!(analysis.truncated);
		assert_eq!(invocation("Program Prog111 invoke [x]"), None);
		assert_eq!(invocation("Program Prog111 invoke [1"), None);
		assert!(!is_success("Program log: success"));
	}

	#[test]
	fn pina_program_errors_match_the_framework() {
		let linked: &[(&str, u32)] = &[
			(
				"StoredBumpMismatch",
				pina::PinaProgramError::StoredBumpMismatch as u32,
			),
			(
				"UnverifiedTransfer",
				pina::PinaProgramError::UnverifiedTransfer as u32,
			),
			(
				"MigrationLamportBudgetExceeded",
				pina::PinaProgramError::MigrationLamportBudgetExceeded as u32,
			),
			(
				"MigrationAccountGrowthExceeded",
				pina::PinaProgramError::MigrationAccountGrowthExceeded as u32,
			),
			(
				"MigrationWorkspaceExceeded",
				pina::PinaProgramError::MigrationWorkspaceExceeded as u32,
			),
			(
				"MigrationBudgetExceeded",
				pina::PinaProgramError::MigrationBudgetExceeded as u32,
			),
			(
				"MigrationUnavailable",
				pina::PinaProgramError::MigrationUnavailable as u32,
			),
			(
				"InvalidMigrationVersion",
				pina::PinaProgramError::InvalidMigrationVersion as u32,
			),
			(
				"MigrationRequired",
				pina::PinaProgramError::MigrationRequired as u32,
			),
			(
				"DuplicateMutableAccount",
				pina::PinaProgramError::DuplicateMutableAccount as u32,
			),
			("DataTooShort", pina::PinaProgramError::DataTooShort as u32),
			(
				"InvalidAccountSize",
				pina::PinaProgramError::InvalidAccountSize as u32,
			),
			(
				"InvalidTokenOwner",
				pina::PinaProgramError::InvalidTokenOwner as u32,
			),
			("SeedsTooMany", pina::PinaProgramError::SeedsTooMany as u32),
			(
				"TooManyAccountKeys",
				pina::PinaProgramError::TooManyAccountKeys as u32,
			),
			(
				"InvalidDiscriminator",
				pina::PinaProgramError::InvalidDiscriminator as u32,
			),
		];
		let table: Vec<_> = PINA_PROGRAM_ERRORS
			.iter()
			.map(|(name, code, _)| (*name, *code))
			.collect();

		assert_eq!(table, linked);
		assert_eq!(RESERVED_ERROR_CODE_START, pina::RESERVED_ERROR_CODE_START);

		// `PinaProgramError` is `#[non_exhaustive]`, so no match outside the crate
		// can prove the table complete. Read the enum from its source instead: a
		// variant added there without a table entry fails here, and the doc check
		// keeps the descriptions current.
		let source_path =
			std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../pina/src/error.rs");
		let source = std::fs::read_to_string(&source_path).expect("pina's error.rs is readable");
		let file =
			syn::parse_file(&source).unwrap_or_else(|error| panic!("parse error.rs: {error}"));
		let declared: Vec<_> = file
			.items
			.iter()
			.find_map(|item| {
				match item {
					syn::Item::Enum(item) if item.ident == "PinaProgramError" => Some(item),
					_ => None,
				}
			})
			.expect("PinaProgramError is declared in error.rs")
			.variants
			.iter()
			.map(|variant| {
				let docs = crate::parse::doc_comments::extract_docs(&variant.attrs);
				let summary = docs
					.iter()
					.take_while(|line| !line.is_empty())
					.cloned()
					.collect::<Vec<_>>()
					.join(" ");
				(variant.ident.to_string(), summary)
			})
			.collect();
		let mut table_docs: Vec<_> = PINA_PROGRAM_ERRORS
			.iter()
			.map(|(name, _, description)| ((*name).to_owned(), (*description).to_owned()))
			.collect();
		let mut declared = declared;
		declared.sort();
		table_docs.sort();

		assert_eq!(table_docs, declared);
	}
}
