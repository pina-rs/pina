//! Adjacent ABI document converters.
//!
//! Each function reshapes one document body from the version it names to the
//! next. They run in memory on the decoded JSON value before the current typed
//! model sees it, and they never write a file. A converter checks every fact it
//! drops against the facts it keeps first, so a document that was already
//! inconsistent fails here instead of being normalized into a valid one.

use serde_json::Map;
use serde_json::Value;

use crate::PublicationReceipt;
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

/// `0.20 → 0.21` publication ledger: drop pins for event transitions.
///
/// Events no longer carry transitions, so a pinned event transition hash names
/// nothing a reader can compare. Removing one changes its receipt's hash, so the
/// chain is proven as written first and then sealed again over the converted
/// receipts: a ledger whose chain was already broken fails here instead of
/// being re-sealed into a valid one.
pub(crate) fn publications_0_20_to_0_21(mut value: Value) -> Result<Value, String> {
	let ledger = value
		.as_object_mut()
		.ok_or_else(|| "the publication ledger is not an object".to_owned())?;
	let receipts = ledger
		.get_mut("receipts")
		.and_then(Value::as_array_mut)
		.ok_or_else(|| "the publication ledger has no `receipts` array".to_owned())?;

	let mut written = None::<String>;
	for (sequence, receipt) in receipts.iter().enumerate() {
		if previous_hash(receipt) != written {
			return Err(format!(
				"receipt {sequence} does not extend the previous receipt hash; restore \
				 migrations/publications.json from version control"
			));
		}
		written = Some(receipt_sha256(sequence, receipt)?);
	}

	let mut sealed = None::<String>;
	for (sequence, receipt) in receipts.iter_mut().enumerate() {
		strip_event_transition_pins(receipt);
		set_previous_hash(receipt, sealed.as_deref());
		sealed = Some(receipt_sha256(sequence, receipt)?);
	}

	if let Some(pending) = ledger
		.get_mut("pending")
		.filter(|pending| !pending.is_null())
	{
		if previous_hash(pending) != written {
			return Err(
				"the pending publication does not extend the last receipt hash; restore \
				 migrations/publications.json from version control"
					.to_owned(),
			);
		}
		strip_event_transition_pins(pending);
		set_previous_hash(pending, sealed.as_deref());
	}

	Ok(value)
}

/// The `previousReceiptSha256` a receipt or pending record carries.
fn previous_hash(record: &Value) -> Option<String> {
	record
		.get("previousReceiptSha256")
		.and_then(Value::as_str)
		.map(str::to_owned)
}

fn set_previous_hash(record: &mut Value, previous: Option<&str>) {
	if let Some(object) = record.as_object_mut() {
		object.insert(
			"previousReceiptSha256".to_owned(),
			previous.map_or(Value::Null, Value::from),
		);
	}
}

/// Hash one receipt exactly as the typed ledger does.
///
/// The receipt shape is unchanged between 0.20 and 0.21, so the typed model
/// reproduces the hash the writer recorded, including its skipped defaults.
fn receipt_sha256(sequence: usize, receipt: &Value) -> Result<String, String> {
	serde_json::from_value::<PublicationReceipt>(receipt.clone())
		.map(|receipt| receipt.sha256())
		.map_err(|error| format!("receipt {sequence} is malformed: {error}"))
}

/// Remove every pinned transition hash from the event contracts of one record.
fn strip_event_transition_pins(record: &mut Value) {
	let Some(versions) = record.get_mut("versions").and_then(Value::as_object_mut) else {
		return;
	};
	for (key, published) in versions.iter_mut() {
		if !key.starts_with("event:") {
			continue;
		}
		let Some(history) = published.get_mut("history").and_then(Value::as_array_mut) else {
			continue;
		};
		for pin in history.iter_mut().filter_map(Value::as_object_mut) {
			pin.remove("transitionSha256");
		}
	}
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
			"event:1:05": { "version": 0, "history": [] },
		})
	}

	/// A two-receipt ledger with a pending record, chained as 0.20 wrote it.
	fn chained_ledger() -> Value {
		let first = receipt(None, published_versions());
		let first_hash = receipt_sha256(0, &first).unwrap();
		let mut second = receipt(Some(&first_hash), published_versions());
		second["sequence"] = json!(1);
		let second_hash = receipt_sha256(1, &second).unwrap();
		let mut pending = receipt(Some(&second_hash), published_versions());
		pending.as_object_mut().unwrap().remove("sequence");
		pending["cluster"] = json!("devnet");
		json!({ "abiVersion": "0.20", "receipts": [first, second], "pending": pending })
	}

	#[test]
	fn the_ledger_step_drops_event_transition_pins_and_reseals_the_chain() {
		let converted = publications_0_20_to_0_21(chained_ledger()).unwrap();
		let receipts = converted["receipts"].as_array().unwrap();

		for record in [&receipts[0], &receipts[1], &converted["pending"]] {
			let event = &record["versions"]["event:1:04"]["history"];
			assert!(event[1].get("transitionSha256").is_none());
			// Accounts keep their transition pins.
			let account = &record["versions"]["account:1:01"]["history"];
			assert!(account[1].get("transitionSha256").is_some());
		}
		assert_eq!(receipts[0]["previousReceiptSha256"], Value::Null);
		let first = receipt_sha256(0, &receipts[0]).unwrap();
		assert_eq!(receipts[1]["previousReceiptSha256"], json!(first));
		let second = receipt_sha256(1, &receipts[1]).unwrap();
		assert_eq!(converted["pending"]["previousReceiptSha256"], json!(second));
	}

	#[test]
	fn the_ledger_step_without_pending_or_event_pins_keeps_its_chain() {
		let first = receipt(
			None,
			json!({ "account:1:01": { "version": 0, "history": [] } }),
		);
		let ledger = json!({ "receipts": [first.clone()], "pending": null });

		let converted = publications_0_20_to_0_21(ledger).unwrap();
		assert_eq!(converted["receipts"][0], first);
		assert_eq!(converted["pending"], Value::Null);
	}

	#[test]
	fn the_ledger_step_refuses_to_reseal_a_broken_chain() {
		let mut broken = chained_ledger();
		broken["receipts"][1]["previousReceiptSha256"] = json!("e".repeat(64));
		let error = publications_0_20_to_0_21(broken).unwrap_err();
		assert!(error.contains("receipt 1 does not extend"), "{error}");

		let mut stale_pending = chained_ledger();
		stale_pending["pending"]["previousReceiptSha256"] = json!("e".repeat(64));
		let error = publications_0_20_to_0_21(stale_pending).unwrap_err();
		assert!(
			error.contains("pending publication does not extend"),
			"{error}"
		);

		let mut malformed = chained_ledger();
		malformed["receipts"][0]["unexpected"] = json!(true);
		let error = publications_0_20_to_0_21(malformed).unwrap_err();
		assert!(error.contains("receipt 0 is malformed"), "{error}");

		let error = publications_0_20_to_0_21(json!([])).unwrap_err();
		assert!(error.contains("is not an object"), "{error}");
		let error = publications_0_20_to_0_21(json!({})).unwrap_err();
		assert!(error.contains("no `receipts` array"), "{error}");
	}

	#[test]
	fn stripping_tolerates_records_without_versions_or_history() {
		let mut record = json!({ "previousReceiptSha256": null });
		strip_event_transition_pins(&mut record);
		assert_eq!(record, json!({ "previousReceiptSha256": null }));

		let unpinned = json!({ "versions": { "event:1:04": { "version": 0 } } });
		let mut record = unpinned.clone();
		strip_event_transition_pins(&mut record);
		assert_eq!(record, unpinned);

		let mut scalar = json!(1);
		set_previous_hash(&mut scalar, Some("hash"));
		assert_eq!(scalar, json!(1));
	}
}
