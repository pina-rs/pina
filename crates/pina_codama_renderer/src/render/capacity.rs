//! `PinaPod` compact-capacity metadata carried through Codama 0.13.2.
//!
//! Codama 0.13.2 has no extension map or maximum-count property for a
//! prefixed collection. Pina therefore emits reserved defined-type markers.
//! The marker identity names one account field, while a supported
//! `fixedSizeTypeNode(bytes)` stores the numeric capacity. The marker never
//! changes the account field's variable wire type and is omitted from Rust
//! client output. A future native Codama extension can replace this carrier
//! without changing account bytes.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use codama_nodes::CamelCaseString;
use codama_nodes::NestedTypeNodeTrait;
use codama_nodes::ProgramNode;
use codama_nodes::TypeNode;

use super::types::is_compact_tail;
use crate::error::RenderError;
use crate::error::Result;

pub(crate) const COMPACT_CAPACITY_MARKER_PREFIX: &str = "pinaPodV1CompactCapacity";

#[derive(Debug, Default)]
pub(crate) struct CompactCapacityIndex {
	capacities: BTreeMap<(String, String), usize>,
	marker_names: BTreeSet<String>,
}

impl CompactCapacityIndex {
	pub(crate) fn read(program: &ProgramNode) -> Result<Self> {
		let mut expected = BTreeMap::new();

		for account in &program.accounts {
			for field in &account.data.get_nested_type_node().fields {
				if !is_compact_tail(&field.r#type) {
					continue;
				}

				let account_name = account.name.as_ref().to_owned();
				let field_name = field.name.as_ref().to_owned();
				let marker_name = compact_capacity_marker_name(&account_name, &field_name);
				let previous = expected.insert(marker_name.clone(), (account_name, field_name));

				if previous.is_some() {
					return Err(metadata_error(
						&marker_name,
						"two compact fields resolve to the same reserved marker name".to_string(),
					));
				}
			}
		}

		let mut capacities = BTreeMap::new();
		let mut marker_names = BTreeSet::new();

		for marker in program
			.defined_types
			.iter()
			.filter(|node| is_compact_capacity_marker(node.name.as_ref()))
		{
			let marker_name = marker.name.as_ref().to_owned();
			let Some((account_name, field_name)) = expected.remove(&marker_name) else {
				return Err(metadata_error(
					&marker_name,
					"marker does not resolve to one compact account field".to_string(),
				));
			};

			if !marker_names.insert(marker_name.clone()) {
				return Err(metadata_error(
					&marker_name,
					"duplicate marker names are not allowed".to_string(),
				));
			}

			if !marker.docs.is_empty() {
				return Err(metadata_error(
					&marker_name,
					"capacity markers cannot contain documentation".to_string(),
				));
			}

			let TypeNode::FixedSize(fixed) = marker.r#type.as_ref() else {
				return Err(metadata_error(
					&marker_name,
					"capacity marker must be a fixedSizeTypeNode".to_string(),
				));
			};
			if !matches!(fixed.r#type.as_ref(), TypeNode::Bytes(_)) {
				return Err(metadata_error(
					&marker_name,
					"capacity marker must wrap a bytesTypeNode".to_string(),
				));
			}

			capacities.insert((account_name, field_name), fixed.size);
		}

		if let Some((marker_name, _)) = expected.into_iter().next() {
			return Err(metadata_error(
				&marker_name,
				"compact account field is missing its capacity marker".to_string(),
			));
		}

		Ok(Self {
			capacities,
			marker_names,
		})
	}

	pub(crate) fn capacity(&self, account: &str, field: &str) -> Result<usize> {
		self.capacities
			.get(&(account.to_owned(), field.to_owned()))
			.copied()
			.ok_or_else(|| {
				metadata_error(
					&compact_capacity_marker_name(account, field),
					"compact account field is missing its capacity marker".to_string(),
				)
			})
	}

	pub(crate) fn is_marker(&self, name: &str) -> bool {
		self.marker_names.contains(name)
	}
}

pub(crate) fn compact_capacity_marker_name(account: &str, field: &str) -> String {
	let source = format!(
		"{COMPACT_CAPACITY_MARKER_PREFIX}_a{}_{}_f{}_{}",
		account.len(),
		account,
		field.len(),
		field,
	);
	CamelCaseString::new(source).as_ref().to_owned()
}

pub(crate) fn is_compact_capacity_marker(name: &str) -> bool {
	name.starts_with(COMPACT_CAPACITY_MARKER_PREFIX)
}

fn metadata_error(marker: &str, reason: String) -> RenderError {
	RenderError::UnsupportedValue {
		context: format!("defined type `{marker}`"),
		kind: "PinaPod compact capacity marker",
		reason,
	}
}
