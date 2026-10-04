//! Parsing of `getTransaction` results requested with `"encoding": "json"`.
//!
//! The JSON encoding lists the message's account keys and a header instead of
//! per-key flags, so signer and writable privileges are derived here the same
//! way the runtime derives them.

use serde::Deserialize;
use serde_json::Value;

use super::ExplainError;

/// One account key of the transaction message with its privileges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MessageAccount {
	pub(crate) address: String,
	pub(crate) signer: bool,
	pub(crate) writable: bool,
}

/// A top-level instruction with its account indices resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MessageInstruction {
	pub(crate) program_id: String,
	/// Indices into [`Transaction::accounts`], in instruction order.
	pub(crate) accounts: Vec<usize>,
	pub(crate) data: Vec<u8>,
}

/// The error an instruction returned, as the RPC serializes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum InstructionError {
	/// A built-in error such as `MissingRequiredSignature`.
	Builtin(String),
	/// `{"Custom": code}`.
	Custom(u32),
}

/// How the transaction ended.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Outcome {
	Succeeded,
	InstructionFailed {
		index: usize,
		error: InstructionError,
	},
	/// A failure the runtime did not attribute to one instruction, such as
	/// `InsufficientFundsForFee`.
	TransactionFailed(Value),
}

/// A landed transaction reduced to what a diagnosis reads.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Transaction {
	pub(crate) signature: String,
	pub(crate) slot: u64,
	/// Static account keys followed by keys loaded from lookup tables.
	pub(crate) accounts: Vec<MessageAccount>,
	pub(crate) instructions: Vec<MessageInstruction>,
	pub(crate) outcome: Outcome,
	/// `None` when the node did not record logs.
	pub(crate) logs: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawResult {
	slot: u64,
	meta: Option<RawMeta>,
	transaction: RawTransaction,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMeta {
	err: Option<Value>,
	log_messages: Option<Vec<String>>,
	loaded_addresses: Option<RawLoadedAddresses>,
}

#[derive(Deserialize)]
struct RawLoadedAddresses {
	writable: Vec<String>,
	readonly: Vec<String>,
}

#[derive(Deserialize)]
struct RawTransaction {
	signatures: Vec<String>,
	message: RawMessage,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMessage {
	header: RawHeader,
	account_keys: Vec<String>,
	instructions: Vec<RawInstruction>,
}

#[derive(Deserialize)]
struct RawHeader {
	#[serde(rename = "numRequiredSignatures")]
	required_signatures: usize,
	#[serde(rename = "numReadonlySignedAccounts")]
	readonly_signed: usize,
	#[serde(rename = "numReadonlyUnsignedAccounts")]
	readonly_unsigned: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawInstruction {
	program_id_index: usize,
	accounts: Vec<usize>,
	data: String,
}

/// Parse a `getTransaction` result object.
///
/// # Errors
///
/// Returns [`ExplainError::InvalidTransaction`] when the value is not a
/// JSON-encoded transaction with status metadata, or its indices and header do
/// not describe a valid message.
pub(crate) fn parse_transaction(value: Value) -> Result<Transaction, ExplainError> {
	let raw: RawResult = serde_json::from_value(value)
		.map_err(|error| invalid(format!("not a JSON-encoded getTransaction result: {error}")))?;
	let meta = raw
		.meta
		.ok_or_else(|| invalid("the transaction has no status metadata".to_owned()))?;
	let signature = raw
		.transaction
		.signatures
		.first()
		.cloned()
		.ok_or_else(|| invalid("the transaction has no signatures".to_owned()))?;
	let accounts = message_accounts(&raw.transaction.message, meta.loaded_addresses.as_ref())?;
	let instructions: Vec<_> = raw
		.transaction
		.message
		.instructions
		.iter()
		.map(|instruction| resolve_instruction(instruction, &accounts))
		.collect::<Result<_, _>>()?;
	let outcome = parse_outcome(meta.err.as_ref());

	if let Outcome::InstructionFailed { index, .. } = &outcome
		&& *index >= instructions.len()
	{
		return Err(invalid(format!(
			"the error names instruction #{index}, but the message has {} instruction(s)",
			instructions.len()
		)));
	}

	Ok(Transaction {
		signature,
		slot: raw.slot,
		accounts,
		instructions,
		outcome,
		logs: meta.log_messages,
	})
}

/// Derive each key's privileges from the message header.
///
/// Static keys are ordered writable signers, read-only signers, writable
/// non-signers, then read-only non-signers. Keys loaded from address lookup
/// tables follow, writable before read-only, and are never signers.
fn message_accounts(
	message: &RawMessage,
	loaded: Option<&RawLoadedAddresses>,
) -> Result<Vec<MessageAccount>, ExplainError> {
	let header = &message.header;
	let total = message.account_keys.len();
	let signed = header.required_signatures;

	if signed > total
		|| header.readonly_signed > signed
		|| header.readonly_unsigned > total - signed
	{
		return Err(invalid(format!(
			"the message header does not fit its {total} account keys"
		)));
	}

	let writable_signed = signed - header.readonly_signed;
	let writable_unsigned = total - header.readonly_unsigned;
	let mut accounts: Vec<MessageAccount> = message
		.account_keys
		.iter()
		.enumerate()
		.map(|(index, address)| {
			MessageAccount {
				address: address.clone(),
				signer: index < signed,
				writable: if index < signed {
					index < writable_signed
				} else {
					index < writable_unsigned
				},
			}
		})
		.collect();

	if let Some(loaded) = loaded {
		for (addresses, writable) in [(&loaded.writable, true), (&loaded.readonly, false)] {
			accounts.extend(addresses.iter().map(|address| {
				MessageAccount {
					address: address.clone(),
					signer: false,
					writable,
				}
			}));
		}
	}

	Ok(accounts)
}

fn resolve_instruction(
	instruction: &RawInstruction,
	accounts: &[MessageAccount],
) -> Result<MessageInstruction, ExplainError> {
	let program_id = accounts
		.get(instruction.program_id_index)
		.map(|account| account.address.clone())
		.ok_or_else(|| {
			invalid("an instruction names a program index past the account keys".to_owned())
		})?;

	if instruction
		.accounts
		.iter()
		.any(|index| *index >= accounts.len())
	{
		return Err(invalid(
			"an instruction names an account index past the account keys".to_owned(),
		));
	}

	let data = bs58::decode(&instruction.data)
		.into_vec()
		.map_err(|error| invalid(format!("instruction data is not base58: {error}")))?;

	Ok(MessageInstruction {
		program_id,
		accounts: instruction.accounts.clone(),
		data,
	})
}

fn parse_outcome(error: Option<&Value>) -> Outcome {
	let Some(error) = error.filter(|error| !error.is_null()) else {
		return Outcome::Succeeded;
	};

	if let Some(Value::Array(parts)) = error.get("InstructionError")
		&& let [index, instruction_error] = parts.as_slice()
		&& let Some(index) = index.as_u64().and_then(|index| usize::try_from(index).ok())
	{
		return Outcome::InstructionFailed {
			index,
			error: parse_instruction_error(instruction_error),
		};
	}

	Outcome::TransactionFailed(error.clone())
}

/// Read an `InstructionError` as serialized by the RPC: a unit variant is a
/// string, and a variant with data is a single-key object.
fn parse_instruction_error(value: &Value) -> InstructionError {
	if let Some(name) = value.as_str() {
		return InstructionError::Builtin(name.to_owned());
	}

	if let Some(code) = value
		.get("Custom")
		.and_then(Value::as_u64)
		.and_then(|code| u32::try_from(code).ok())
	{
		return InstructionError::Custom(code);
	}

	match value
		.as_object()
		.map(|object| object.keys().collect::<Vec<_>>())
	{
		Some(keys) if keys.len() == 1 => InstructionError::Builtin(keys[0].clone()),
		_ => InstructionError::Builtin(value.to_string()),
	}
}

fn invalid(reason: String) -> ExplainError {
	ExplainError::InvalidTransaction { reason }
}

#[cfg(test)]
mod tests {
	use serde_json::json;

	use super::*;

	fn transaction(header: Value, keys: &[&str], meta: Value) -> Value {
		json!({
			"slot": 7,
			"blockTime": null,
			"meta": meta,
			"transaction": {
				"signatures": ["sig"],
				"message": {
					"header": header,
					"accountKeys": keys,
					"recentBlockhash": "11111111111111111111111111111111",
					"instructions": [
						{ "programIdIndex": keys.len() - 1, "accounts": [0, 1], "data": "Ldp", "stackHeight": null }
					]
				}
			},
			"version": "legacy"
		})
	}

	fn header(signed: usize, readonly_signed: usize, readonly_unsigned: usize) -> Value {
		json!({
			"numRequiredSignatures": signed,
			"numReadonlySignedAccounts": readonly_signed,
			"numReadonlyUnsignedAccounts": readonly_unsigned
		})
	}

	fn flags(transaction: &Transaction) -> Vec<(bool, bool)> {
		transaction
			.accounts
			.iter()
			.map(|account| (account.signer, account.writable))
			.collect()
	}

	#[test]
	fn derives_legacy_privileges_from_the_header() {
		let value = transaction(
			header(2, 1, 2),
			&["payer", "cosigner", "vault", "mint", "program"],
			json!({ "err": null, "logMessages": ["Program log: ok"] }),
		);
		let parsed = parse_transaction(value).expect("valid legacy transaction");

		assert_eq!(parsed.signature, "sig");
		assert_eq!(parsed.slot, 7);
		assert_eq!(
			flags(&parsed),
			[
				(true, true),
				(true, false),
				(false, true),
				(false, false),
				(false, false)
			]
		);
		assert_eq!(parsed.instructions[0].program_id, "program");
		assert_eq!(parsed.instructions[0].accounts, [0, 1]);
		assert_eq!(parsed.instructions[0].data, [1, 2, 3]);
		assert_eq!(parsed.outcome, Outcome::Succeeded);
		assert_eq!(parsed.logs, Some(vec!["Program log: ok".to_owned()]));
	}

	#[test]
	fn appends_loaded_addresses_as_non_signers() {
		let value = transaction(
			header(1, 0, 1),
			&["payer", "program"],
			json!({
				"err": { "InstructionError": [0, { "Custom": 42 }] },
				"logMessages": null,
				"loadedAddresses": { "writable": ["table-writable"], "readonly": ["table-readonly"] }
			}),
		);
		let parsed = parse_transaction(value).expect("valid v0 transaction");
		let addresses: Vec<_> = parsed
			.accounts
			.iter()
			.map(|account| account.address.as_str())
			.collect();

		assert_eq!(
			addresses,
			["payer", "program", "table-writable", "table-readonly"]
		);
		assert_eq!(
			flags(&parsed),
			[(true, true), (false, false), (false, true), (false, false)]
		);
		assert_eq!(
			parsed.outcome,
			Outcome::InstructionFailed {
				index: 0,
				error: InstructionError::Custom(42),
			}
		);
		assert_eq!(parsed.logs, None);
	}

	#[test]
	fn reads_every_instruction_error_shape() {
		assert_eq!(
			parse_instruction_error(&json!("MissingRequiredSignature")),
			InstructionError::Builtin("MissingRequiredSignature".to_owned())
		);
		assert_eq!(
			parse_instruction_error(&json!({ "Custom": 4_294_967_294_u64 })),
			InstructionError::Custom(0xFFFF_FFFE)
		);
		assert_eq!(
			parse_instruction_error(&json!({ "BorshIoError": "unexpected length" })),
			InstructionError::Builtin("BorshIoError".to_owned())
		);
		assert_eq!(
			parse_instruction_error(&json!(17)),
			InstructionError::Builtin("17".to_owned())
		);
		assert_eq!(
			parse_outcome(Some(&json!("AccountNotFound"))),
			Outcome::TransactionFailed(json!("AccountNotFound"))
		);
		assert_eq!(parse_outcome(Some(&Value::Null)), Outcome::Succeeded);
	}

	#[test]
	fn rejects_malformed_transactions() {
		let cases = [
			(
				json!({ "slot": 1 }),
				"not a JSON-encoded getTransaction result",
			),
			(
				transaction(header(1, 0, 0), &["payer", "program"], Value::Null),
				"no status metadata",
			),
			(
				transaction(
					header(3, 0, 0),
					&["payer", "program"],
					json!({ "err": null }),
				),
				"does not fit its 2 account keys",
			),
			(
				transaction(
					header(1, 2, 0),
					&["payer", "program"],
					json!({ "err": null }),
				),
				"does not fit its 2 account keys",
			),
			(
				transaction(
					header(1, 0, 2),
					&["payer", "program"],
					json!({ "err": null }),
				),
				"does not fit its 2 account keys",
			),
		];

		for (value, message) in cases {
			let error = parse_transaction(value).expect_err("malformed transaction");
			assert!(error.to_string().contains(message), "{error}");
		}

		let mut value = transaction(
			header(1, 0, 0),
			&["payer", "program"],
			json!({ "err": null }),
		);
		value["transaction"]["signatures"] = json!([]);
		let error = parse_transaction(value).expect_err("no signature");
		assert!(error.to_string().contains("no signatures"));

		let mut value = transaction(
			header(1, 0, 0),
			&["payer", "program"],
			json!({ "err": null }),
		);
		value["transaction"]["message"]["instructions"][0]["programIdIndex"] = json!(9);
		let error = parse_transaction(value).expect_err("program index out of range");
		assert!(
			error
				.to_string()
				.contains("program index past the account keys")
		);

		let mut value = transaction(
			header(1, 0, 0),
			&["payer", "program"],
			json!({ "err": null }),
		);
		value["transaction"]["message"]["instructions"][0]["accounts"] = json!([0, 5]);
		let error = parse_transaction(value).expect_err("account index out of range");
		assert!(
			error
				.to_string()
				.contains("account index past the account keys")
		);

		let mut value = transaction(
			header(1, 0, 0),
			&["payer", "program"],
			json!({ "err": null }),
		);
		value["transaction"]["message"]["instructions"][0]["data"] = json!("0OIl");
		let error = parse_transaction(value).expect_err("data is not base58");
		assert!(error.to_string().contains("instruction data is not base58"));

		let value = transaction(
			header(1, 0, 0),
			&["payer", "program"],
			json!({ "err": { "InstructionError": [3, "InvalidArgument"] } }),
		);
		let error = parse_transaction(value).expect_err("error index out of range");
		assert!(error.to_string().contains("names instruction #3"));
	}
}
