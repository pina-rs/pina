//! Adjacent ABI document converters.
//!
//! Each function reshapes one document body from the version it names to the
//! next. They run in memory on the decoded JSON value before the current typed
//! model sees it, and they never write a file. A converter checks every fact it
//! drops against the facts it keeps first, so a document that was already
//! inconsistent fails here instead of being normalized into a valid one.

use serde_json::Map;
use serde_json::Value;

use crate::SCHEMA_CODEC;

/// `0.20 → 0.21` manifest: drop the stored copies of derived facts.
///
/// - `identity` repeated the contract key. It is dropped once it is proven to
///   compose to exactly that key.
/// - `schema.codec` repeated the wire format every 0.20 document was derived
///   under. It is dropped once it is proven to be that format.
/// - Event transitions described conversions no reader ever runs: an event is
///   decoded with the schema of the version that emitted it. They are dropped.
///
/// Every 0.20 instruction history was enveloped, which is the 0.21 default, so
/// instructions carry forward unchanged.
pub(crate) fn manifest_0_20_to_0_21(mut value: Value) -> Result<Value, String> {
	let contracts = value
		.get_mut("contracts")
		.and_then(Value::as_object_mut)
		.ok_or_else(|| "the manifest has no `contracts` object".to_owned())?;

	for (key, history) in contracts.iter_mut() {
		let history = history
			.as_object_mut()
			.ok_or_else(|| format!("contract `{key}` is not an object"))?;
		let kind = take_identity(key, history)?;
		let versions = history
			.get_mut("versions")
			.and_then(Value::as_array_mut)
			.ok_or_else(|| format!("contract `{key}` has no `versions` array"))?;

		for (number, version) in versions.iter_mut().enumerate() {
			let version = version
				.as_object_mut()
				.ok_or_else(|| format!("contract `{key}` version {number} is not an object"))?;
			let schema = version
				.get_mut("schema")
				.and_then(Value::as_object_mut)
				.ok_or_else(|| format!("contract `{key}` version {number} has no `schema`"))?;
			match schema.remove("codec") {
				Some(Value::String(codec)) if codec == SCHEMA_CODEC => {}
				Some(other) => {
					return Err(format!(
						"contract `{key}` version {number} records codec {other}, but every 0.20 \
						 schema is `{SCHEMA_CODEC}`"
					));
				}
				None => {
					return Err(format!(
						"contract `{key}` version {number} is missing its `codec`"
					));
				}
			}
			if kind == "event" {
				version.remove("transition");
			}
		}
	}

	Ok(value)
}

/// Remove a 0.20 `identity` object after proving it spells `key`.
///
/// Returns the identity's kind so the caller can apply kind-specific rules.
fn take_identity(key: &str, history: &mut Map<String, Value>) -> Result<String, String> {
	let identity = history
		.remove("identity")
		.ok_or_else(|| format!("contract `{key}` has no `identity`"))?;
	let kind = identity.get("kind").and_then(Value::as_str);
	let bytes = identity.get("discriminatorBytes").and_then(Value::as_u64);
	let hex = identity.get("discriminatorHex").and_then(Value::as_str);
	let (Some(kind), Some(bytes), Some(hex)) = (kind, bytes, hex) else {
		return Err(format!("contract `{key}` has a malformed `identity`"));
	};
	if format!("{kind}:{bytes}:{hex}") != key {
		return Err(format!(
			"contract `{key}` records identity `{kind}:{bytes}:{hex}`; the key and its identity \
			 must agree"
		));
	}
	Ok(kind.to_owned())
}

/// `0.20 → 0.21` publication ledger: drop every stored copy of a fact the
/// ledger or its manifest already records.
///
/// - `sequence` repeated each receipt's position. It is dropped once it is
///   proven to equal that position.
/// - `previousReceiptSha256` chained each record to the receipt before it.
///   Anyone able to edit the file could recompute the chain, so it proved
///   nothing version control does not, and it is dropped unchecked: that is
///   what lets `pina migrations reconcile --pin-legacy` fill in missing pins
///   without re-sealing a chain no reader keeps.
/// - `programId` repeated the manifest's program on every record. It is
///   dropped once every record is proven to name the same program.
/// - `manifestSha256` hashed a manifest that later versions rewrite, so no
///   reader could compare it with anything. It is dropped.
/// - Each contract's `version` repeated the position of its last pin. It is
///   dropped once it is proven to be that position, and the entry becomes the
///   bare list of pins. A 0.20 entry that pinned nothing cannot be converted
///   until [`pin_legacy_publications`](crate::pin_legacy_publications) pins it.
/// - Event transition pins name transitions events no longer carry. They are
///   dropped.
pub(crate) fn publications_0_20_to_0_21(mut value: Value) -> Result<Value, String> {
	let ledger = value
		.as_object_mut()
		.ok_or_else(|| "the publication ledger is not an object".to_owned())?;
	let receipts = ledger
		.get_mut("receipts")
		.and_then(Value::as_array_mut)
		.ok_or_else(|| "the publication ledger has no `receipts` array".to_owned())?;

	let mut program = None::<String>;
	for (position, receipt) in receipts.iter_mut().enumerate() {
		let label = format!("receipt {position}");
		let record = record_object(&label, receipt)?;
		if record
			.remove("sequence")
			.and_then(|sequence| sequence.as_u64())
			!= u64::try_from(position).ok()
		{
			return Err(format!(
				"{label} records a sequence other than its position; restore \
				 migrations/publications.json from version control"
			));
		}
		reshape_record(&label, record, &mut program)?;
	}

	if let Some(pending) = ledger
		.get_mut("pending")
		.filter(|pending| !pending.is_null())
	{
		let label = "the pending publication";
		reshape_record(label, record_object(label, pending)?, &mut program)?;
	}

	Ok(value)
}

fn record_object<'record>(
	label: &str,
	record: &'record mut Value,
) -> Result<&'record mut Map<String, Value>, String> {
	record
		.as_object_mut()
		.ok_or_else(|| format!("{label} is not an object"))
}

/// Drop the program, manifest hash, chain link, and per-contract versions of
/// one record.
fn reshape_record(
	label: &str,
	record: &mut Map<String, Value>,
	program: &mut Option<String>,
) -> Result<(), String> {
	let named = record
		.remove("programId")
		.and_then(|program| program.as_str().map(str::to_owned))
		.ok_or_else(|| format!("{label} names no `programId`"))?;
	if program.get_or_insert_with(|| named.clone()) != &named {
		return Err(format!(
			"{label} names program {named}, but an earlier record names {}; one ledger \
			 belongs to one program",
			program.as_deref().unwrap_or_default()
		));
	}
	record.remove("manifestSha256");
	record.remove("previousReceiptSha256");

	let versions = record
		.get_mut("versions")
		.and_then(Value::as_object_mut)
		.ok_or_else(|| format!("{label} has no `versions` object"))?;
	for (key, published) in versions.iter_mut() {
		let version = published.get("version").and_then(Value::as_u64);
		let mut history = published
			.get_mut("history")
			.map(Value::take)
			.and_then(|history| {
				match history {
					Value::Array(history) => Some(history),
					_ => None,
				}
			})
			.ok_or_else(|| format!("{label} has no pinned `history` for `{key}`"))?;
		if history.is_empty() {
			return Err(format!(
				"{label} names `{key}` without pinning its published schemas. Confirm with \
				 version control that migrations/manifest.json still records exactly what was \
				 deployed, then run `pina migrations reconcile --pin-legacy` to pin it"
			));
		}
		if version != u64::try_from(history.len() - 1).ok() {
			return Err(format!(
				"{label} records a version for `{key}` other than the position of its last pin"
			));
		}
		if key.starts_with("event:") {
			for pin in history.iter_mut().filter_map(Value::as_object_mut) {
				pin.remove("transitionSha256");
			}
		}
		*published = Value::Array(history);
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use serde_json::json;

	use super::*;

	fn manifest(contracts: Value) -> Value {
		json!({
			"abiVersion": "0.20",
			"programId": "program",
			"versionType": "u8",
			"contracts": contracts,
		})
	}

	fn history(kind: &str, hex: &str, versions: Value) -> Value {
		json!({
			"identity": { "kind": kind, "discriminatorBytes": 1, "discriminatorHex": hex },
			"rustName": "Contract",
			"versions": versions,
		})
	}

	fn schema_version(codec: Value, transition: Value) -> Value {
		let mut schema = json!({ "layout": "fixed", "fields": [] });
		if !codec.is_null() {
			schema["codec"] = codec;
		}
		json!({ "schema": schema, "transition": transition })
	}

	#[test]
	fn the_manifest_step_drops_identity_codec_and_event_transitions() {
		let automatic = json!({ "mode": "automatic", "implementationSha256": null });
		let value = manifest(json!({
			"account:1:01": history("account", "01", json!([
				schema_version(json!("pinaPodV2"), Value::Null),
				schema_version(json!("pinaPodV2"), automatic.clone()),
			])),
			"event:1:04": history("event", "04", json!([
				schema_version(json!("pinaPodV2"), Value::Null),
				schema_version(json!("pinaPodV2"), automatic.clone()),
			])),
		}));

		let converted = manifest_0_20_to_0_21(value).unwrap();
		let account = &converted["contracts"]["account:1:01"];
		assert!(account.get("identity").is_none());
		assert!(account["versions"][0]["schema"].get("codec").is_none());
		assert_eq!(account["versions"][1]["transition"], automatic);
		let event = &converted["contracts"]["event:1:04"];
		assert!(event["versions"][1].get("transition").is_none());
	}

	#[test]
	fn the_manifest_step_rejects_facts_it_cannot_prove() {
		let valid = || schema_version(json!("pinaPodV2"), Value::Null);
		let cases = [
			(json!({ "abiVersion": "0.20" }), "no `contracts` object"),
			(manifest(json!({ "account:1:01": 1 })), "is not an object"),
			(
				manifest(json!({ "account:1:01": { "rustName": "A", "versions": [] } })),
				"has no `identity`",
			),
			(
				manifest(json!({ "account:1:01": {
					"identity": { "kind": "account" }, "rustName": "A", "versions": []
				} })),
				"malformed `identity`",
			),
			(
				manifest(json!({ "account:1:01": history("account", "02", json!([valid()])) })),
				"must agree",
			),
			(
				manifest(json!({ "account:1:01": {
					"identity": { "kind": "account", "discriminatorBytes": 1, "discriminatorHex": "01" },
					"rustName": "A",
				} })),
				"no `versions` array",
			),
			(
				manifest(json!({ "account:1:01": history("account", "01", json!([1])) })),
				"version 0 is not an object",
			),
			(
				manifest(json!({ "account:1:01": history("account", "01", json!([{}])) })),
				"has no `schema`",
			),
			(
				manifest(json!({ "account:1:01": history("account", "01", json!([
					schema_version(json!("pinaPodV3"), Value::Null)
				])) })),
				"records codec",
			),
			(
				manifest(json!({ "account:1:01": history("account", "01", json!([
					schema_version(Value::Null, Value::Null)
				])) })),
				"missing its `codec`",
			),
		];
		for (value, expected) in cases {
			let error = manifest_0_20_to_0_21(value).unwrap_err();
			assert!(
				error.contains(expected),
				"expected `{expected}`, got: {error}"
			);
		}
	}

	fn receipt(previous: Option<&str>, versions: Value) -> Value {
		json!({
			"sequence": 0,
			"rpcUrl": "fixture",
			"programId": "program",
			"executableSha256": "a".repeat(64),
			"manifestSha256": "b".repeat(64),
			"versions": versions,
			"previousReceiptSha256": previous,
		})
	}

	fn pinned(transition: Option<&str>) -> Value {
		let mut pin = json!({ "schemaSha256": "c".repeat(64) });
		if let Some(transition) = transition {
			pin["transitionSha256"] = json!(transition);
		}
		pin
	}

	fn published_versions() -> Value {
		let transition = "d".repeat(64);
		json!({
			"account:1:01": {
				"version": 1,
				"history": [pinned(None), pinned(Some(&transition))],
			},
			"event:1:04": {
				"version": 1,
				"history": [pinned(None), pinned(Some(&transition))],
			},
		})
	}

	/// A two-receipt ledger with a pending record, shaped as 0.20 wrote it.
	/// The chain links are never checked, so any value stands in for them.
	fn chained_ledger() -> Value {
		let first = receipt(None, published_versions());
		let mut second = receipt(Some(&"e".repeat(64)), published_versions());
		second["sequence"] = json!(1);
		let mut pending = receipt(Some(&"f".repeat(64)), published_versions());
		pending.as_object_mut().unwrap().remove("sequence");
		pending["cluster"] = json!("devnet");
		json!({ "abiVersion": "0.20", "receipts": [first, second], "pending": pending })
	}

	#[test]
	fn the_ledger_step_keeps_only_facts_nothing_else_records() {
		let converted = publications_0_20_to_0_21(chained_ledger()).unwrap();
		let receipts = converted["receipts"].as_array().unwrap();

		for record in [&receipts[0], &receipts[1], &converted["pending"]] {
			for dropped in [
				"sequence",
				"programId",
				"manifestSha256",
				"previousReceiptSha256",
			] {
				assert!(
					record.get(dropped).is_none(),
					"{dropped} survived: {record}"
				);
			}
			assert_eq!(record["executableSha256"], json!("a".repeat(64)));
			// A contract entry is the bare list of its pins.
			let account = &record["versions"]["account:1:01"];
			assert_eq!(account.as_array().map(Vec::len), Some(2));
			assert!(account[1].get("transitionSha256").is_some());
			// Event transition pins are gone; accounts keep theirs.
			let event = &record["versions"]["event:1:04"];
			assert!(event[1].get("transitionSha256").is_none());
		}
		assert_eq!(converted["pending"]["cluster"], json!("devnet"));
	}

	#[test]
	fn the_ledger_step_without_pending_keeps_its_receipts() {
		let first = receipt(
			None,
			json!({ "account:1:01": { "version": 0, "history": [pinned(None)] } }),
		);
		let ledger = json!({ "receipts": [first], "pending": null });

		let converted = publications_0_20_to_0_21(ledger).unwrap();
		assert_eq!(
			converted["receipts"][0],
			json!({
				"rpcUrl": "fixture",
				"executableSha256": "a".repeat(64),
				"versions": { "account:1:01": [pinned(None)] },
			})
		);
		assert_eq!(converted["pending"], Value::Null);
	}

	#[test]
	fn the_ledger_step_refuses_facts_it_cannot_prove() {
		let expect = |ledger: Value, expected: &str| {
			let error = publications_0_20_to_0_21(ledger).unwrap_err();
			assert!(
				error.contains(expected),
				"expected `{expected}`, got: {error}"
			);
		};

		let mut reordered = chained_ledger();
		reordered["receipts"][0]["sequence"] = json!(5);
		expect(
			reordered,
			"receipt 0 records a sequence other than its position",
		);

		let mut foreign = chained_ledger();
		foreign["pending"]["programId"] = json!("other");
		expect(foreign, "one ledger belongs to one program");

		let mut nameless = chained_ledger();
		nameless["pending"]
			.as_object_mut()
			.unwrap()
			.remove("programId");
		expect(nameless, "names no `programId`");

		let mut unversioned = chained_ledger();
		unversioned["pending"]
			.as_object_mut()
			.unwrap()
			.remove("versions");
		expect(unversioned, "has no `versions` object");

		let mut unpinned = chained_ledger();
		unpinned["pending"]["versions"]["account:1:01"]["history"] = json!([]);
		expect(unpinned, "run `pina migrations reconcile --pin-legacy`");

		let mut historyless = chained_ledger();
		historyless["pending"]["versions"]["account:1:01"] = json!({ "version": 0 });
		expect(historyless, "has no pinned `history`");

		let mut scalar_history = chained_ledger();
		scalar_history["pending"]["versions"]["account:1:01"]["history"] = json!(1);
		expect(scalar_history, "has no pinned `history`");

		let mut mismatched = chained_ledger();
		mismatched["pending"]["versions"]["account:1:01"]["version"] = json!(3);
		expect(mismatched, "other than the position of its last pin");

		let mut scalar = chained_ledger();
		scalar["pending"] = json!(1);
		expect(scalar, "the pending publication is not an object");

		let mut scalar_receipt = chained_ledger();
		scalar_receipt["receipts"][0] = json!(1);
		expect(scalar_receipt, "receipt 0 is not an object");

		expect(json!([]), "is not an object");
		expect(json!({}), "no `receipts` array");
	}
}
