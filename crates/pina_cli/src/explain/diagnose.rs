//! Candidate causes for a failed instruction.
//!
//! The evaluator replays what `#[derive(Accounts)]` generates, in the order it
//! runs: the cursor parses each field (account count, `&mut` writability,
//! duplicate mutable accounts), then validation checks every field's header,
//! identity, data, and relationship rules, then nested structs, then the
//! processor's own account calls. Each check whose error matches the observed
//! one becomes a candidate, unless the transaction proves it passed.

use std::collections::HashMap;

use serde::Serialize;

use super::decode::DUPLICATE_MUTABLE_ACCOUNT;
use super::decode::ErrorKey;
use super::decode::INVALID_ACCOUNT_SIZE;
use super::decode::INVALID_DISCRIMINATOR;
use super::decode::PINA_PROGRAM_ERRORS;
use super::decode::TOO_MANY_ACCOUNT_KEYS;
use super::rpc::AccountState;
use super::source::AccountsLayout;
use super::source::DeclaredRule;
use super::source::FieldKind;
use super::source::Flow;
use super::source::LayoutField;
use super::source::Location;
use super::source::Phase;
use super::source::RuleKind;
use super::source::SourceIndex;
use super::source::compact_tokens;
use super::transaction::InstructionError;
use super::transaction::MessageInstruction;
use super::transaction::Transaction;
use crate::ir::InstructionIr;
use crate::ir::ProgramIr;
use crate::parse::validation::known_address_from_expr;

const WRITABLE_LOG: &str = "account has not been marked as writable";
const EXECUTABLE_LOG: &str = "account is not executable";
const ADDRESS_LOG: &str = "account address is invalid";
const DATA_LEN_LOG: &str = "account has an incorrect length";
const SYSVAR_OWNER: &str = "Sysvar1111111111111111111111111111111111111";

/// How strongly the transaction supports a candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
	/// Provable from the transaction alone.
	Confirmed,
	/// The check fails against account state read after the transaction, which
	/// may have changed since.
	CheckedAgainstCurrentState,
	/// The check can return this error but needs runtime values to evaluate.
	Possible,
}

/// What a candidate constrains.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CandidateScope {
	/// An account slot of the instruction.
	Account,
	/// A decoded instruction argument.
	Argument,
	/// The instruction as a whole, such as its discriminator.
	Instruction,
}

/// One check that may have produced the error.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
	/// What the check constrains.
	pub scope: CandidateScope,
	/// Accounts-struct field (`parent.child` when nested) or argument name.
	pub field: Option<String>,
	/// Position in the instruction's account list.
	pub account_index: Option<usize>,
	/// The rule as written, such as `signer`, `owner = ID`, or `assert_signer()`.
	pub rule: String,
	/// `path:line` of the rule, relative to the project root.
	pub location: Option<String>,
	/// How strongly the transaction supports the candidate.
	pub confidence: Confidence,
	/// Whether the failing program logged the message this check logs.
	pub log_confirmed: bool,
	/// Why the candidate fits, in one sentence.
	pub reason: String,
	#[serde(skip)]
	order: usize,
}

/// One account of the failing instruction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountRow {
	/// Position in the instruction's account list.
	pub index: usize,
	/// Accounts-struct field bound to the slot, when known.
	pub field: Option<String>,
	pub address: String,
	pub signer: bool,
	pub writable: bool,
	/// Whether the slot holds the program's own address, which marks an
	/// optional account as absent.
	pub absent: bool,
}

/// The instruction a transaction invoked, as the source names it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstructionSummary {
	/// IDL name, such as `check_policy`.
	pub name: String,
	/// Instruction-data struct, such as `CheckPolicyInstruction`.
	pub rust_name: String,
	/// The `#[derive(Accounts)]` struct the dispatch parses, when known.
	pub accounts_struct: Option<String>,
}

/// Current account state available to a diagnosis.
#[derive(Clone, Copy, Debug)]
pub(crate) enum CurrentState<'a> {
	/// No endpoint was given, so state-dependent checks stay possible.
	NotRequested,
	/// The request failed; the reason is reported as a note.
	Unavailable,
	Fetched(&'a HashMap<String, AccountState>),
}

/// The result of diagnosing one failed instruction of the explained program.
pub(crate) struct Diagnosis {
	pub(crate) instruction: Option<InstructionSummary>,
	pub(crate) candidates: Vec<Candidate>,
	pub(crate) accounts: Vec<AccountRow>,
	pub(crate) flow: Flow,
	pub(crate) notes: Vec<String>,
}

/// Everything the evaluator reads about the failure.
pub(crate) struct Failure<'a> {
	pub(crate) transaction: &'a Transaction,
	pub(crate) instruction: &'a MessageInstruction,
	pub(crate) error: &'a InstructionError,
	/// `Program log:` messages of the failing program.
	pub(crate) messages: &'a [String],
	pub(crate) state: CurrentState<'a>,
}

/// Diagnose a failed instruction that targets the explained program.
pub(crate) fn diagnose(
	program: &ProgramIr,
	source: &SourceIndex,
	failure: &Failure<'_>,
) -> Diagnosis {
	let mut evaluator = Evaluator {
		program,
		source,
		failure,
		candidates: Vec::new(),
		names: vec![None; failure.instruction.accounts.len()],
		absent: vec![false; failure.instruction.accounts.len()],
		opaque: false,
		flow: Flow::default(),
		notes: Vec::new(),
	};
	let instruction = identify(program, &failure.instruction.data);

	let summary = if let Some(instruction) = instruction {
		Some(evaluator.instruction(instruction))
	} else {
		evaluator.unknown_discriminator();
		None
	};

	let Evaluator {
		mut candidates,
		names,
		absent,
		flow,
		notes,
		..
	} = evaluator;

	// The program stops at its first failing check, so a check that runs after
	// one the transaction proves failing never ran. When no remaining candidate
	// explains the program's log, later log-confirmed checks are kept: the log
	// outranks the model.
	if let Some(first_proven) = candidates
		.iter()
		.filter(|candidate| candidate.confidence == Confidence::Confirmed)
		.map(|candidate| candidate.order)
		.min()
	{
		let log_explained = candidates
			.iter()
			.any(|candidate| candidate.order <= first_proven && candidate.log_confirmed);
		candidates.retain(|candidate| {
			candidate.order <= first_proven || (!log_explained && candidate.log_confirmed)
		});
	}

	candidates.sort_by_key(|candidate| {
		(
			!candidate.log_confirmed,
			candidate.confidence,
			candidate.order,
		)
	});
	let accounts = failure
		.instruction
		.accounts
		.iter()
		.enumerate()
		.map(|(index, key)| {
			let account = &failure.transaction.accounts[*key];

			AccountRow {
				index,
				field: names[index].clone(),
				address: account.address.clone(),
				signer: account.signer,
				writable: account.writable,
				absent: absent[index],
			}
		})
		.collect();

	Diagnosis {
		instruction: summary,
		candidates,
		accounts,
		flow,
		notes,
	}
}

/// Find the instruction whose discriminator starts `data`.
fn identify<'a>(program: &'a ProgramIr, data: &[u8]) -> Option<&'a InstructionIr> {
	program.instructions.iter().find(|instruction| {
		let width = instruction.discriminator.repr_size;

		data.len() >= width && little_endian(&data[..width]) == instruction.discriminator.value
	})
}

fn little_endian(bytes: &[u8]) -> u64 {
	bytes
		.iter()
		.rev()
		.fold(0, |value, byte| (value << 8) | u64::from(*byte))
}

/// A field bound to the slot the cursor gave it.
struct BoundField<'a> {
	path: String,
	field: &'a LayoutField,
	/// Position of a present account; `None` for a missing slot, an absent
	/// optional, or a nested struct or slice.
	position: Option<usize>,
	nested: Option<BoundStruct<'a>>,
}

struct BoundStruct<'a> {
	fields: Vec<BoundField<'a>>,
}

/// What the transaction or current state says about one check.
enum Evidence {
	/// The transaction proves the check fails.
	Fails(String),
	/// The transaction proves the check passes.
	Passes,
	/// Current state says the check fails.
	StateFails(String),
	/// Current state says the check passes.
	StatePasses,
	/// The check needs values pina cannot read offline.
	Unknown(String),
}

/// The parts of a candidate that do not depend on its evidence.
struct Subject<'a> {
	scope: CandidateScope,
	field: Option<&'a str>,
	position: Option<usize>,
	rule: String,
	location: Option<&'a Location>,
	/// Message the check logs on failure in default builds.
	log: Option<&'static str>,
}

struct Evaluator<'a> {
	program: &'a ProgramIr,
	source: &'a SourceIndex,
	failure: &'a Failure<'a>,
	candidates: Vec<Candidate>,
	names: Vec<Option<String>>,
	absent: Vec<bool>,
	/// Set when a nested struct's layout is unknown, so later slots are
	/// unattributed.
	opaque: bool,
	flow: Flow,
	notes: Vec<String>,
}

impl<'a> Evaluator<'a> {
	fn instruction(&mut self, instruction: &InstructionIr) -> InstructionSummary {
		let source = self.source;
		let accounts_struct = source.dispatch.get(&instruction.name).cloned();
		let layout = accounts_struct
			.as_ref()
			.and_then(|name| source.accounts.get(name));
		self.flow.types.insert(instruction.rust_name.clone());

		match layout {
			Some(layout) => self.accounts(layout),
			None => {
				self.notes.push(format!(
					"No `#[derive(Accounts)]` struct is known for `{}`, so its accounts are not \
					 named.",
					instruction.name
				));
			}
		}

		self.arguments(&instruction.rust_name);

		if let Some(layout) = layout {
			self.process_sites(layout);
		}

		InstructionSummary {
			name: instruction.name.clone(),
			rust_name: instruction.rust_name.clone(),
			accounts_struct,
		}
	}

	fn unknown_discriminator(&mut self) {
		let data = &self.failure.instruction.data;
		let Some(width) = self
			.program
			.instructions
			.first()
			.map(|instruction| instruction.discriminator.repr_size)
		else {
			return;
		};

		let reason = if data.len() < width {
			format!(
				"the instruction data holds {} byte(s), fewer than the {width}-byte discriminator",
				data.len()
			)
		} else {
			format!(
				"the data starts with discriminator {}, which no instruction of `{}` declares",
				little_endian(&data[..width]),
				self.program.name
			)
		};

		self.consider(
			&ErrorKey::builtin("InvalidInstructionData"),
			Evidence::Fails(reason),
			&Subject {
				scope: CandidateScope::Instruction,
				field: None,
				position: None,
				rule: "instruction discriminator".to_owned(),
				location: None,
				log: None,
			},
		);
	}

	/// Replay the generated parser and validator of the top-level struct.
	fn accounts(&mut self, layout: &'a AccountsLayout) {
		let mut cursor = 0;
		let mut missing_reported = false;
		let bound = self.bind(layout, "", &mut cursor, &mut missing_reported);
		let len = self.failure.instruction.accounts.len();
		let has_remaining = layout
			.fields
			.iter()
			.any(|field| matches!(field.kind, FieldKind::Remaining { .. }));

		if !has_remaining && !self.opaque && cursor < len {
			let extra = (cursor..len)
				.map(|position| format!("#{position}"))
				.collect::<Vec<_>>()
				.join(", ");
			self.consider(
				&ErrorKey::Custom(TOO_MANY_ACCOUNT_KEYS),
				Evidence::Fails(format!(
					"the instruction passed {len} accounts, but `{}` reads at most {cursor} (extra: \
					 {extra})",
					layout.name
				)),
				&Subject {
					scope: CandidateScope::Instruction,
					field: None,
					position: Some(cursor),
					rule: "no extra accounts".to_owned(),
					location: Some(&layout.location),
					log: None,
				},
			);
		}

		self.validate(&bound);
	}

	/// Bind each field to its slot as the cursor would, recording parse-time
	/// failures. Binding stops at a nested struct whose layout is unknown,
	/// because the slots after it cannot be attributed.
	fn bind(
		&mut self,
		layout: &'a AccountsLayout,
		prefix: &str,
		cursor: &mut usize,
		missing_reported: &mut bool,
	) -> BoundStruct<'a> {
		let source = self.source;
		let len = self.failure.instruction.accounts.len();
		let mut fields = Vec::with_capacity(layout.fields.len());
		self.flow.types.insert(layout.name.clone());
		self.flow.functions.extend(layout.hook.clone());

		for field in &layout.fields {
			if self.opaque {
				break;
			}

			let path = format!("{prefix}{}", field.name);

			match &field.kind {
				FieldKind::Account { mutable, optional } => {
					let position = (*cursor < len).then_some(*cursor);
					*cursor = (*cursor + 1).min(len);
					let present = position
						.filter(|position| !(*optional && self.is_program_address(*position)));

					if let Some(position) = position {
						self.names[position] = Some(path.clone());
						self.absent[position] = present.is_none();
					}

					if position.is_none() && !optional && !*missing_reported {
						*missing_reported = true;
						self.missing(field, &path, layout, len);
					}

					if *mutable && let Some(position) = present {
						self.mutable_slot(field, &path, position);
					}

					fields.push(BoundField {
						path,
						field,
						position: present,
						nested: None,
					});
				}
				FieldKind::Nested(name) => {
					let Some(nested) = source.accounts.get(name) else {
						self.notes.push(format!(
							"`{name}` is not a `#[derive(Accounts)]` struct of this program, so \
							 the accounts from `{path}` on are not named."
						));
						self.opaque = true;
						break;
					};
					let bound = self.bind(nested, &format!("{path}."), cursor, missing_reported);

					fields.push(BoundField {
						path,
						field,
						position: None,
						nested: Some(bound),
					});
				}
				FieldKind::Remaining { mutable, distinct } => {
					let start = *cursor;
					*cursor = len;
					self.remaining(field, &path, start, *mutable, *distinct);

					fields.push(BoundField {
						path,
						field,
						position: None,
						nested: None,
					});
				}
			}
		}

		BoundStruct { fields }
	}

	fn missing(&mut self, field: &'a LayoutField, path: &str, layout: &AccountsLayout, len: usize) {
		self.consider(
			&ErrorKey::builtin("NotEnoughAccountKeys"),
			Evidence::Fails(format!(
				"the instruction passed {len} account(s), so `{}` has no slot #{len} for `{path}`",
				layout.name
			)),
			&Subject {
				scope: CandidateScope::Account,
				field: Some(path),
				position: Some(len),
				rule: "required account".to_owned(),
				location: Some(&field.location),
				log: None,
			},
		);
	}

	/// The cursor's checks for a `&mut AccountView` slot: writable, then not
	/// repeated in any later slot.
	fn mutable_slot(&mut self, field: &'a LayoutField, path: &str, position: usize) {
		let subject = |rule: &str, log| {
			Subject {
				scope: CandidateScope::Account,
				field: Some(path),
				position: Some(position),
				rule: rule.to_owned(),
				location: Some(&field.location),
				log,
			}
		};
		let writable = self.writable_evidence(position);
		self.consider(
			&ErrorKey::builtin("InvalidAccountData"),
			writable,
			&subject("writable (`&mut` field)", Some(WRITABLE_LOG)),
		);

		let key = self.failure.instruction.accounts[position];
		let repeated = self.failure.instruction.accounts[position + 1..]
			.iter()
			.position(|later| *later == key)
			.map(|offset| position + 1 + offset);
		let evidence = repeated.map_or(Evidence::Passes, |repeated| {
			Evidence::Fails(format!(
				"account #{position} ({}) is bound to a mutable field and appears again at \
				 #{repeated}",
				self.address(position)
			))
		});
		self.consider(
			&ErrorKey::Custom(DUPLICATE_MUTABLE_ACCOUNT),
			evidence,
			&subject("distinct mutable account", None),
		);
	}

	fn remaining(
		&mut self,
		field: &'a LayoutField,
		path: &str,
		start: usize,
		mutable: bool,
		distinct: bool,
	) {
		let len = self.failure.instruction.accounts.len();

		for position in start..len {
			let name = format!("{path}[{}]", position - start);
			self.names[position] = Some(name.clone());

			let subject = |rule: &str, log| {
				Subject {
					scope: CandidateScope::Account,
					field: Some(&name),
					position: Some(position),
					rule: rule.to_owned(),
					location: Some(&field.location),
					log,
				}
			};

			if mutable {
				let writable = self.writable_evidence(position);
				self.consider(
					&ErrorKey::builtin("InvalidAccountData"),
					writable,
					&subject(
						"writable (`&mut [AccountView]` remaining slice)",
						Some(WRITABLE_LOG),
					),
				);
			}

			let key = self.failure.instruction.accounts[position];
			let earlier = self.failure.instruction.accounts[start..position]
				.iter()
				.position(|earlier| *earlier == key);
			if distinct && let Some(earlier) = earlier {
				self.consider(
					&ErrorKey::Custom(DUPLICATE_MUTABLE_ACCOUNT),
					Evidence::Fails(format!(
						"remaining account #{position} ({}) repeats #{}",
						self.address(position),
						start + earlier
					)),
					&subject("distinct remaining accounts", None),
				);
			}
		}
	}

	/// Replay the generated validator: every field's checks of one phase, then
	/// the next phase, then nested structs.
	fn validate(&mut self, bound: &BoundStruct<'a>) {
		for phase in [
			Phase::Header,
			Phase::Identity,
			Phase::Data,
			Phase::Relationship,
		] {
			for field in &bound.fields {
				let mut rules: Vec<_> = field
					.field
					.rules
					.iter()
					.filter(|rule| rule.kind.phase() == phase)
					.collect();
				rules.sort_by_key(|rule| {
					(
						rule.group,
						RuleKind::ALL.iter().position(|kind| *kind == rule.kind),
					)
				});

				for rule in rules {
					self.rule(bound, field, rule);
				}
			}
		}

		for field in &bound.fields {
			if let Some(nested) = &field.nested {
				self.validate(nested);
			}
		}
	}

	fn rule(&mut self, bound: &BoundStruct<'a>, field: &BoundField<'a>, rule: &DeclaredRule) {
		let Some(position) = field.position else {
			return;
		};
		let subject = |log| {
			Subject {
				scope: CandidateScope::Account,
				field: Some(field.path.as_str()),
				position: Some(position),
				rule: rule.describe(),
				location: Some(&rule.location),
				log,
			}
		};
		let error = rule.error.as_deref();
		let address = self.address(position).to_owned();

		match rule.kind {
			RuleKind::Signer => {
				let evidence = if self.account(position).signer {
					Evidence::Passes
				} else {
					Evidence::Fails(format!(
						"account #{position} ({address}) is not a signer of this transaction"
					))
				};
				self.consider(
					&self.override_key(error, "MissingRequiredSignature"),
					evidence,
					&subject(None),
				);
			}
			RuleKind::Writable => {
				let evidence = self.writable_evidence(position);
				self.consider(
					&self.override_key(error, "InvalidAccountData"),
					evidence,
					&subject(Some(WRITABLE_LOG)),
				);
			}
			RuleKind::Executable => {
				let evidence = self.executable_evidence(position);
				self.consider(
					&self.override_key(error, "InvalidAccountData"),
					evidence,
					&subject(Some(EXECUTABLE_LOG)),
				);
			}
			RuleKind::Address | RuleKind::Addresses => {
				let evidence = self.address_evidence(position, rule.value.as_ref());
				self.consider(
					&self.override_key(error, "InvalidAccountData"),
					evidence,
					&subject(Some(ADDRESS_LOG)),
				);
			}
			RuleKind::Owner | RuleKind::Owners => {
				let evidence = self.owner_evidence(position, rule.value.as_ref());
				self.consider(
					&self.override_key(error, "InvalidAccountOwner"),
					evidence,
					&subject(None),
				);
			}
			RuleKind::Program => {
				// `program` checks the address first, then that the account is
				// executable.
				let evidence = self.address_evidence(position, rule.value.as_ref());
				let address_failed = matches!(evidence, Evidence::Fails(_));
				self.consider(
					&self.override_key(error, "InvalidAccountData"),
					evidence,
					&subject(Some(ADDRESS_LOG)),
				);

				if !address_failed {
					let evidence = self.executable_evidence(position);
					self.consider(
						&self.override_key(error, "InvalidAccountData"),
						evidence,
						&subject(Some(EXECUTABLE_LOG)),
					);
				}
			}
			RuleKind::Sysvar => {
				// `sysvar` checks the sysvar owner first, then the address.
				let owner = self.owner_is(position, &[SYSVAR_OWNER.to_owned()]);
				self.consider(
					&self.override_key(error, "InvalidAccountOwner"),
					owner,
					&subject(None),
				);
				let evidence = self.address_evidence(position, rule.value.as_ref());
				self.consider(
					&self.override_key(error, "InvalidAccountData"),
					evidence,
					&subject(Some(ADDRESS_LOG)),
				);
			}
			RuleKind::Empty => {
				let evidence = self.data_len_evidence(position, |len| {
					(len != 0).then(|| {
						format!("account #{position} ({address}) currently holds {len} bytes")
					})
				});
				self.consider(
					&self.override_key(error, "AccountAlreadyInitialized"),
					evidence,
					&subject(None),
				);
			}
			RuleKind::NotEmpty => {
				let evidence = self.data_len_evidence(position, |len| {
					(len == 0)
						.then(|| format!("account #{position} ({address}) currently holds no data"))
				});
				self.consider(
					&self.override_key(error, "UninitializedAccount"),
					evidence,
					&subject(None),
				);
			}
			RuleKind::DataLen => {
				let expected = rule.value.as_ref().and_then(integer_literal);
				let evidence = match expected {
					Some(expected) => {
						self.data_len_evidence(position, |len| {
							(len != expected).then(|| {
								format!(
									"account #{position} ({address}) currently holds {len} bytes, \
									 expected {expected}"
								)
							})
						})
					}
					None => {
						Evidence::Unknown(format!(
							"the expected length `{}` is not a literal pina can evaluate",
							rule.value.as_ref().map(compact_tokens).unwrap_or_default()
						))
					}
				};
				self.consider(
					&self.override_key(error, "InvalidAccountData"),
					evidence,
					&subject(Some(DATA_LEN_LOG)),
				);
			}
			RuleKind::DistinctFrom => {
				let target = rule.value.as_ref().map(compact_tokens).unwrap_or_default();
				let other = bound
					.fields
					.iter()
					.find(|candidate| candidate.field.name == target)
					.and_then(|candidate| candidate.position);
				let accounts = &self.failure.instruction.accounts;
				let evidence = match other {
					Some(other) if accounts[other] == accounts[position] => {
						Evidence::Fails(format!(
							"`{}` (#{position}) and `{target}` (#{other}) are both {address}",
							field.path
						))
					}
					_ => Evidence::Passes,
				};
				self.consider(
					&self.override_key(error, "InvalidAccountData"),
					evidence,
					&subject(None),
				);
			}
		}
	}

	/// Value rules on the instruction's arguments run when the processor
	/// decodes its data, and need the decoded values.
	fn arguments(&mut self, rust_name: &str) {
		let source = self.source;
		let Some(rules) = source.instructions.get(rust_name) else {
			return;
		};
		self.flow.functions.extend(rules.hook.clone());

		// `try_from_bytes` reports a data length or discriminator mismatch with
		// `InvalidAccountData`, the same code as several account checks.
		self.consider(
			&ErrorKey::builtin("InvalidAccountData"),
			Evidence::Unknown(format!(
				"`{rust_name}::try_from_bytes` rejects data whose length or discriminator does not \
				 match the struct; this instruction carried {} byte(s)",
				self.failure.instruction.data.len()
			)),
			&Subject {
				scope: CandidateScope::Instruction,
				field: None,
				position: None,
				rule: format!("`{rust_name}` data layout"),
				location: Some(&rules.location),
				log: None,
			},
		);

		for rule in &rules.rules {
			let key = self.override_key(rule.error.as_deref(), "InvalidInstructionData");
			self.consider(
				&key,
				Evidence::Unknown("value rules read the decoded instruction data".to_owned()),
				&Subject {
					scope: CandidateScope::Argument,
					field: Some(&rule.field),
					position: None,
					rule: rule.rule.clone(),
					location: Some(&rule.location),
					log: None,
				},
			);
		}
	}

	/// Account calls in the processor body run after the generated validation.
	fn process_sites(&mut self, layout: &AccountsLayout) {
		let source = self.source;
		let Some(sites) = source.process_sites.get(&layout.name) else {
			return;
		};

		for site in sites {
			let Some((keys, log)) = assertion_errors(&site.method) else {
				continue;
			};
			let Some(position) = self.position_of(&site.field) else {
				continue;
			};
			let address = self.address(position).to_owned();
			let evidence = match site.method.as_str() {
				"assert_signer" if self.account(position).signer => Evidence::Passes,
				"assert_signer" => {
					Evidence::Fails(format!(
						"account #{position} ({address}) is not a signer of this transaction"
					))
				}
				"assert_writable" => self.writable_evidence(position),
				"assert_executable" => self.executable_evidence(position),
				"assert_empty" => {
					self.data_len_evidence(position, |len| {
						(len != 0).then(|| {
							format!("account #{position} ({address}) currently holds {len} bytes")
						})
					})
				}
				"assert_not_empty" => {
					self.data_len_evidence(position, |len| {
						(len == 0).then(|| {
							format!("account #{position} ({address}) currently holds no data")
						})
					})
				}
				_ => Evidence::Unknown(unknown_call_reason(&site.method).to_owned()),
			};
			let Some(key) = keys.into_iter().find(|key| key.matches(self.failure.error)) else {
				continue;
			};

			self.consider(
				&key,
				evidence,
				&Subject {
					scope: CandidateScope::Account,
					field: Some(&site.field),
					position: Some(position),
					rule: format!("{}()", site.method),
					location: Some(&site.location),
					log,
				},
			);
		}
	}

	/// Record a candidate when `key` matches the observed error and the
	/// evidence does not prove the check passed.
	fn consider(&mut self, key: &ErrorKey, evidence: Evidence, subject: &Subject<'_>) {
		if !key.matches(self.failure.error) {
			return;
		}

		let (confidence, reason) =
			match evidence {
				Evidence::Passes => return,
				Evidence::Fails(reason) => (Confidence::Confirmed, reason),
				Evidence::StateFails(reason) => (Confidence::CheckedAgainstCurrentState, reason),
				Evidence::StatePasses => (
					Confidence::Possible,
					"the check passes against the current account state, which may have changed \
					 since the transaction"
						.to_owned(),
				),
				Evidence::Unknown(reason) => (Confidence::Possible, reason),
			};
		let log_confirmed = subject
			.log
			.is_some_and(|log| self.failure.messages.iter().any(|message| message == log));

		self.candidates.push(Candidate {
			scope: subject.scope,
			field: subject.field.map(ToOwned::to_owned),
			account_index: subject.position,
			rule: subject.rule.clone(),
			location: subject.location.map(ToString::to_string),
			confidence,
			log_confirmed,
			reason,
			order: self.candidates.len(),
		});
	}

	/// The error a check returns: its `error = ...` override, or `default`.
	fn override_key(&self, error: Option<&[String]>, default: &str) -> ErrorKey {
		let Some(path) = error else {
			return ErrorKey::builtin(default);
		};
		let [.., enum_name, variant] = path else {
			return ErrorKey::AnyCustom;
		};

		if enum_name == "ProgramError" {
			return ErrorKey::Builtin(variant.clone());
		}

		if enum_name == "PinaProgramError"
			&& let Some((_, code, _)) = PINA_PROGRAM_ERRORS
				.iter()
				.find(|(name, ..)| name == variant)
		{
			return ErrorKey::Custom(*code);
		}

		self.source
			.errors
			.iter()
			.find(|declared| declared.enum_name == *enum_name && declared.error.name == *variant)
			.map_or(ErrorKey::AnyCustom, |declared| {
				ErrorKey::Custom(declared.error.code)
			})
	}

	fn position_of(&self, field: &str) -> Option<usize> {
		self.names
			.iter()
			.enumerate()
			.find(|(position, name)| name.as_deref() == Some(field) && !self.absent[*position])
			.map(|(position, _)| position)
	}

	fn account(&self, position: usize) -> &super::transaction::MessageAccount {
		&self.failure.transaction.accounts[self.failure.instruction.accounts[position]]
	}

	fn address(&self, position: usize) -> &str {
		&self.account(position).address
	}

	fn is_program_address(&self, position: usize) -> bool {
		self.address(position) == self.program.public_key
	}

	fn writable_evidence(&self, position: usize) -> Evidence {
		if self.account(position).writable {
			return Evidence::Passes;
		}

		Evidence::Fails(format!(
			"account #{position} ({}) is read-only in this transaction",
			self.address(position)
		))
	}

	fn executable_evidence(&self, position: usize) -> Evidence {
		self.with_state(position, |state| {
			if state.executable {
				return Evidence::StatePasses;
			}

			Evidence::StateFails(format!(
				"account #{position} ({}) is currently not executable",
				self.address(position)
			))
		})
	}

	fn address_evidence(&self, position: usize, value: Option<&syn::Expr>) -> Evidence {
		let Some(expected) = value.and_then(|value| self.addresses(value)) else {
			return Evidence::Unknown(format!(
				"the expected address `{}` is not a constant pina can resolve",
				value.map(compact_tokens).unwrap_or_default()
			));
		};
		let address = self.address(position);

		if expected.iter().any(|expected| expected == address) {
			return Evidence::Passes;
		}

		Evidence::Fails(format!(
			"account #{position} is {address}, expected {}",
			expected.join(" or ")
		))
	}

	fn owner_evidence(&self, position: usize, value: Option<&syn::Expr>) -> Evidence {
		match value.and_then(|value| self.addresses(value)) {
			Some(expected) => self.owner_is(position, &expected),
			None => {
				Evidence::Unknown(format!(
					"the expected owner `{}` is not a constant pina can resolve",
					value.map(compact_tokens).unwrap_or_default()
				))
			}
		}
	}

	fn owner_is(&self, position: usize, expected: &[String]) -> Evidence {
		self.with_state(position, |state| {
			if expected.contains(&state.owner) {
				return Evidence::StatePasses;
			}

			Evidence::StateFails(format!(
				"account #{position} ({}) is currently owned by {}, expected {}",
				self.address(position),
				state.owner,
				expected.join(" or ")
			))
		})
	}

	fn data_len_evidence(
		&self,
		position: usize,
		fails: impl FnOnce(u64) -> Option<String>,
	) -> Evidence {
		self.with_state(position, |state| {
			let Some(len) = state.data_len else {
				return Evidence::Unknown(
					"the RPC did not report the account's data length".to_owned(),
				);
			};

			fails(len).map_or(Evidence::StatePasses, Evidence::StateFails)
		})
	}

	fn with_state(
		&self,
		position: usize,
		evaluate: impl FnOnce(&AccountState) -> Evidence,
	) -> Evidence {
		match self.failure.state {
			CurrentState::NotRequested => Evidence::Unknown(
				"the check reads account state; pass --network or --rpc-url to check it against \
					 the current state"
					.to_owned(),
			),
			CurrentState::Unavailable => {
				Evidence::Unknown(
					"the check reads account state, which could not be fetched".to_owned(),
				)
			}
			CurrentState::Fetched(states) => {
				match states.get(self.address(position)) {
					Some(state) => evaluate(state),
					None => {
						Evidence::Unknown(
							"the check reads account state, which was not fetched".to_owned(),
						)
					}
				}
			}
		}
	}

	/// Resolve an address expression, or an array of them, to base58 values.
	fn addresses(&self, value: &syn::Expr) -> Option<Vec<String>> {
		match value {
			syn::Expr::Reference(reference) => self.addresses(&reference.expr),
			syn::Expr::Array(array) => {
				array
					.elems
					.iter()
					.map(|element| self.address_constant(element))
					.collect()
			}
			_ => self.address_constant(value).map(|address| vec![address]),
		}
	}

	/// A known program or sysvar ID, or the program's own `ID`.
	fn address_constant(&self, value: &syn::Expr) -> Option<String> {
		if let Some(address) = known_address_from_expr(value) {
			return Some(address);
		}

		let path = compact_tokens(value);
		matches!(
			path.trim_start_matches('&'),
			"ID" | "crate::ID" | "self::ID" | "super::ID"
		)
		.then(|| self.program.public_key.clone())
	}
}

/// The errors an account call in a processor body can return, and the message
/// it logs in default builds.
fn assertion_errors(method: &str) -> Option<(Vec<ErrorKey>, Option<&'static str>)> {
	let builtin = |names: &[&str]| {
		names
			.iter()
			.map(|name| ErrorKey::builtin(name))
			.collect::<Vec<_>>()
	};
	let account_type = || {
		let mut keys = builtin(&["InvalidAccountOwner", "InvalidAccountData"]);
		keys.extend([
			ErrorKey::Custom(INVALID_ACCOUNT_SIZE),
			ErrorKey::Custom(INVALID_DISCRIMINATOR),
		]);
		keys
	};

	Some(match method {
		"assert_signer" => (builtin(&["MissingRequiredSignature"]), None),
		"assert_writable" => (builtin(&["InvalidAccountData"]), Some(WRITABLE_LOG)),
		"assert_executable" => (builtin(&["InvalidAccountData"]), Some(EXECUTABLE_LOG)),
		"assert_data_len" => (builtin(&["InvalidAccountData"]), Some(DATA_LEN_LOG)),
		"assert_empty" => (builtin(&["AccountAlreadyInitialized"]), None),
		"assert_not_empty" => (builtin(&["UninitializedAccount"]), None),
		"assert_address" | "assert_addresses" | "assert_program" => {
			(builtin(&["InvalidAccountData"]), Some(ADDRESS_LOG))
		}
		"assert_owner" | "assert_owners" => (builtin(&["InvalidAccountOwner"]), None),
		"assert_sysvar" => {
			(
				builtin(&["InvalidAccountOwner", "InvalidAccountData"]),
				Some(ADDRESS_LOG),
			)
		}
		"assert_seeds"
		| "assert_seeds_with_bump"
		| "assert_canonical_bump"
		| "assert_associated_token_address" => (builtin(&["InvalidSeeds"]), Some(ADDRESS_LOG)),
		"assert_type" | "as_account" | "as_account_mut" => (account_type(), None),
		"load_pda" | "load_pda_mut" | "with_pda" | "with_stored_bump_pda" | "with_checked_pda" => {
			let mut keys = account_type();
			keys.push(ErrorKey::builtin("InvalidSeeds"));
			(keys, None)
		}
		_ => return None,
	})
}

fn unknown_call_reason(method: &str) -> &'static str {
	match method {
		"assert_seeds"
		| "assert_seeds_with_bump"
		| "assert_canonical_bump"
		| "assert_associated_token_address" => "PDA seeds are computed at runtime",
		"assert_type"
		| "as_account"
		| "as_account_mut"
		| "load_pda"
		| "load_pda_mut"
		| "with_pda"
		| "with_stored_bump_pda"
		| "with_checked_pda" => "the account type check reads the account's data at runtime",
		_ => "the call's arguments are evaluated at runtime",
	}
}

fn integer_literal(value: &syn::Expr) -> Option<u64> {
	let syn::Expr::Lit(syn::ExprLit {
		lit: syn::Lit::Int(literal),
		..
	}) = value
	else {
		return None;
	};

	literal.base10_parse().ok()
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::ir::DiscriminatorIr;

	fn instruction(name: &str, value: u64, repr_size: usize) -> InstructionIr {
		InstructionIr {
			name: name.to_owned(),
			rust_name: format!("{name}Instruction"),
			accounts: Vec::new(),
			arguments: Vec::new(),
			discriminator: DiscriminatorIr { value, repr_size },
			docs: Vec::new(),
		}
	}

	#[test]
	fn identifies_instructions_by_their_little_endian_discriminator() {
		let program = ProgramIr {
			name: "wide".to_owned(),
			public_key: "Wide1111111111111111111111111111111111111111".to_owned(),
			pinapod_enums: Vec::new(),
			accounts: Vec::new(),
			instructions: vec![
				instruction("first", 0x0102, 2),
				instruction("second", 0x0201, 2),
			],
			events: Vec::new(),
			errors: Vec::new(),
			pdas: Vec::new(),
		};

		assert_eq!(little_endian(&[0x02, 0x01]), 0x0102);
		assert_eq!(
			identify(&program, &[0x02, 0x01, 0xFF]).map(|found| found.name.as_str()),
			Some("first")
		);
		assert_eq!(
			identify(&program, &[0x01, 0x02]).map(|found| found.name.as_str()),
			Some("second")
		);
		assert!(identify(&program, &[0x02]).is_none());
		assert!(identify(&program, &[0x03, 0x00]).is_none());
	}

	#[test]
	fn maps_processor_calls_to_the_errors_they_return() {
		let builtin = ErrorKey::builtin;

		assert_eq!(
			assertion_errors("assert_data_len"),
			Some((vec![builtin("InvalidAccountData")], Some(DATA_LEN_LOG)))
		);
		assert_eq!(
			assertion_errors("assert_program"),
			Some((vec![builtin("InvalidAccountData")], Some(ADDRESS_LOG)))
		);
		assert_eq!(
			assertion_errors("assert_sysvar"),
			Some((
				vec![
					builtin("InvalidAccountOwner"),
					builtin("InvalidAccountData")
				],
				Some(ADDRESS_LOG)
			))
		);
		assert_eq!(
			assertion_errors("assert_canonical_bump"),
			Some((vec![builtin("InvalidSeeds")], Some(ADDRESS_LOG)))
		);
		assert_eq!(
			assertion_errors("as_account"),
			Some((
				vec![
					builtin("InvalidAccountOwner"),
					builtin("InvalidAccountData"),
					ErrorKey::Custom(INVALID_ACCOUNT_SIZE),
					ErrorKey::Custom(INVALID_DISCRIMINATOR),
				],
				None
			))
		);
		assert_eq!(
			assertion_errors("load_pda_mut").and_then(|(keys, _)| keys.last().cloned()),
			Some(builtin("InvalidSeeds"))
		);
		assert_eq!(assertion_errors("address"), None);

		assert_eq!(
			unknown_call_reason("assert_seeds"),
			"PDA seeds are computed at runtime"
		);
		assert_eq!(
			unknown_call_reason("with_checked_pda"),
			"the account type check reads the account's data at runtime"
		);
		assert_eq!(
			unknown_call_reason("assert_address"),
			"the call's arguments are evaluated at runtime"
		);
	}

	#[test]
	fn reads_only_integer_literal_lengths() {
		assert_eq!(integer_literal(&syn::parse_quote!(165)), Some(165));
		assert_eq!(integer_literal(&syn::parse_quote!(SIZE)), None);
		assert_eq!(integer_literal(&syn::parse_quote!("8")), None);
	}
}
