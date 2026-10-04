//! The `pinaComputeUnits` instruction plugin.
//!
//! `pina test --record-compute-units` measures what each instruction consumes
//! in its Surfpool suite, and IDL generation turns the measurement into a
//! compute unit limit. Both numbers travel to every client generator as one
//! plugin node on the instruction:
//!
//! ```json
//! { "kind": "pluginNode", "name": "pinaComputeUnits", "payload": { "measured": 1234, "limit": 1800 } }
//! ```
//!
//! The limit is computed once, by `pina_cli`; generators copy it and never
//! recompute a margin. An instruction without the plugin has no measurement,
//! and its clients request no limit, so the runtime default applies.

use codama_nodes::InstructionNode;
use codama_nodes::PluginNode;
use serde_json::Value;

use crate::error::RenderError;
use crate::error::Result;

/// Name of the instruction plugin that carries a compute unit budget.
pub const COMPUTE_UNITS_PLUGIN: &str = "pinaComputeUnits";

/// The recorded compute unit budget for one instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComputeUnitBudget {
	/// Compute units the instruction consumed in its most expensive recorded
	/// simulation.
	pub measured: u32,
	/// Compute unit limit a transaction carrying the instruction requests:
	/// the measurement plus the project's margin and the compute budget
	/// instructions' own cost.
	pub limit: u32,
}

impl ComputeUnitBudget {
	/// Encode the budget as the plugin node attached to an instruction.
	#[must_use]
	pub fn to_plugin(self) -> PluginNode {
		PluginNode::with_payload(
			COMPUTE_UNITS_PLUGIN,
			serde_json::json!({ "measured": self.measured, "limit": self.limit }),
		)
	}

	/// Read the budget an instruction node carries, if it carries one.
	///
	/// # Errors
	///
	/// Returns [`RenderError::UnsupportedValue`] when the instruction carries
	/// the plugin more than once, or a payload that is not exactly
	/// `{ "measured": u32, "limit": u32 }` with a limit that covers the
	/// measurement.
	pub fn from_instruction(instruction: &InstructionNode) -> Result<Option<Self>> {
		let context = format!("instruction `{}`", instruction.name.as_ref());
		let mut plugins = instruction
			.plugins
			.iter()
			.filter(|plugin| plugin.name.as_ref() == COMPUTE_UNITS_PLUGIN);
		let Some(plugin) = plugins.next() else {
			return Ok(None);
		};

		if plugins.next().is_some() {
			return Err(invalid(&context, "the plugin appears more than once"));
		}

		let Some(Value::Object(payload)) = &plugin.payload else {
			return Err(invalid(&context, "the payload must be an object"));
		};

		if let Some(key) = payload
			.keys()
			.find(|key| !matches!(key.as_str(), "measured" | "limit"))
		{
			return Err(invalid(&context, &format!("unknown payload key `{key}`")));
		}

		let measured = payload_units(payload.get("measured"), "measured", &context)?;
		let limit = payload_units(payload.get("limit"), "limit", &context)?;

		if limit < measured {
			return Err(invalid(
				&context,
				&format!("limit {limit} does not cover the measured {measured} compute units"),
			));
		}

		Ok(Some(Self { measured, limit }))
	}
}

fn payload_units(value: Option<&Value>, key: &str, context: &str) -> Result<u32> {
	value
		.and_then(Value::as_u64)
		.and_then(|units| u32::try_from(units).ok())
		.ok_or_else(|| {
			invalid(
				context,
				&format!("`{key}` must be an unsigned 32-bit integer"),
			)
		})
}

fn invalid(context: &str, reason: &str) -> RenderError {
	RenderError::UnsupportedValue {
		context: context.to_string(),
		kind: "pluginNode",
		reason: format!("invalid `{COMPUTE_UNITS_PLUGIN}` plugin: {reason}"),
	}
}

#[cfg(test)]
mod tests {
	use std::error::Error;

	use codama_nodes::InstructionNode;
	use codama_nodes::PluginNode;
	use serde_json::json;

	use super::*;

	type TestResult = std::result::Result<(), Box<dyn Error>>;

	fn instruction_with(plugins: Vec<PluginNode>) -> InstructionNode {
		InstructionNode {
			name: "increment".into(),
			plugins,
			..InstructionNode::default()
		}
	}

	/// Whether reading `plugins` fails with a reason containing `expected`.
	fn rejects(plugins: Vec<PluginNode>, expected: &str) -> bool {
		matches!(
			ComputeUnitBudget::from_instruction(&instruction_with(plugins)),
			Err(RenderError::UnsupportedValue { reason, .. }) if reason.contains(expected)
		)
	}

	#[test]
	fn round_trips_through_the_plugin_node() -> TestResult {
		let budget = ComputeUnitBudget {
			measured: 1_234,
			limit: 1_800,
		};
		let plugin = budget.to_plugin();

		assert_eq!(
			serde_json::to_value(&plugin)?,
			json!({
				"kind": "pluginNode",
				"name": "pinaComputeUnits",
				"payload": { "measured": 1_234, "limit": 1_800 },
			})
		);
		assert_eq!(
			ComputeUnitBudget::from_instruction(&instruction_with(vec![plugin]))?,
			Some(budget)
		);

		Ok(())
	}

	#[test]
	fn an_instruction_without_the_plugin_has_no_budget() -> TestResult {
		let unrelated = PluginNode::with_payload("anchor", json!({ "version": "0.30.0" }));

		assert_eq!(
			ComputeUnitBudget::from_instruction(&instruction_with(vec![unrelated]))?,
			None
		);

		Ok(())
	}

	#[test]
	fn rejects_malformed_payloads() {
		let plugin = |payload: Value| PluginNode::with_payload(COMPUTE_UNITS_PLUGIN, payload);
		let budget = ComputeUnitBudget {
			measured: 10,
			limit: 20,
		};

		assert!(rejects(
			vec![budget.to_plugin(), budget.to_plugin()],
			"more than once"
		));
		assert!(rejects(
			vec![PluginNode::new(COMPUTE_UNITS_PLUGIN)],
			"must be an object"
		));
		assert!(rejects(
			vec![plugin(json!({ "measured": 1, "limit": 2, "docs": [] }))],
			"unknown payload key `docs`"
		));
		assert!(rejects(
			vec![plugin(json!({ "measured": -1, "limit": 2 }))],
			"`measured` must be an unsigned 32-bit integer"
		));
		assert!(rejects(
			vec![plugin(json!({ "measured": 1, "limit": 5_000_000_000_u64 }))],
			"`limit` must be an unsigned 32-bit integer"
		));
		assert!(rejects(
			vec![plugin(json!({ "measured": 30, "limit": 20 }))],
			"does not cover"
		));
	}
}
