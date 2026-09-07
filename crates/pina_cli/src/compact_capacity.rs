//! Machine-readable compact collection limits for Codama 0.13.2.
//!
//! Codama 0.13.2 cannot attach extension metadata or a maximum count to a
//! prefixed collection. Pina carries the bound in a reserved defined type
//! whose supported `fixedSizeTypeNode(bytes)` size is the collection
//! capacity. The account field retains its real variable wire type.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use codama_nodes::CamelCaseString;
use codama_nodes::CountNode;
use codama_nodes::NestedTypeNode;
use codama_nodes::NestedTypeNodeTrait;
use codama_nodes::NumberFormat;
use codama_nodes::NumberTypeNode;
use codama_nodes::PostOffsetStrategy;
use codama_nodes::PreOffsetStrategy;
use codama_nodes::ProgramNode;
use codama_nodes::TypeNode;

pub(crate) const COMPACT_CAPACITY_MARKER_PREFIX: &str = "pinaPodV1CompactCapacity";

#[derive(Debug, Default)]
pub(crate) struct CompactCapacityIndex {
	capacities: BTreeMap<(String, String), CompactCapacity>,
	marker_names: BTreeSet<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CompactCapacityKind {
	String,
	Vec,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompactCapacity {
	pub(crate) capacity: usize,
	pub(crate) kind: CompactCapacityKind,
	pub(crate) option_tag_offset: Option<usize>,
}

impl CompactCapacityIndex {
	pub(crate) fn read(program: &ProgramNode) -> Result<Self, String> {
		let mut expected = BTreeMap::new();

		for account in &program.accounts {
			for field in &account.data.get_nested_type_node().fields {
				let Some((kind, option_tag_offset)) = compact_tail_metadata(&field.r#type)? else {
					continue;
				};

				let account_name = account.name.as_ref().to_owned();
				let field_name = field.name.as_ref().to_owned();
				let marker_name = compact_capacity_marker_name(&account_name, &field_name);
				let previous = expected.insert(
					marker_name.clone(),
					(account_name, field_name, kind, option_tag_offset),
				);

				if previous.is_some() {
					return Err(format!(
						"capacity marker `{marker_name}` resolves to more than one compact field"
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
			let Some((account_name, field_name, kind, option_tag_offset)) =
				expected.remove(&marker_name)
			else {
				return Err(format!(
					"capacity marker `{marker_name}` does not resolve to one compact account field"
				));
			};

			if !marker_names.insert(marker_name.clone()) {
				return Err(format!("duplicate capacity marker `{marker_name}`"));
			}

			if !marker.docs.is_empty() {
				return Err(format!(
					"capacity marker `{marker_name}` cannot contain documentation"
				));
			}

			let TypeNode::FixedSize(fixed) = marker.r#type.as_ref() else {
				return Err(format!(
					"capacity marker `{marker_name}` must be a fixedSizeTypeNode"
				));
			};
			if !matches!(fixed.r#type.as_ref(), TypeNode::Bytes(_)) {
				return Err(format!(
					"capacity marker `{marker_name}` must wrap a bytesTypeNode"
				));
			}

			capacities.insert(
				(account_name, field_name),
				CompactCapacity {
					capacity: fixed.size,
					kind,
					option_tag_offset,
				},
			);
		}

		if let Some((marker_name, _)) = expected.into_iter().next() {
			return Err(format!(
				"compact field is missing capacity marker `{marker_name}`"
			));
		}

		Ok(Self {
			capacities,
			marker_names,
		})
	}

	pub(crate) fn capacities(&self) -> &BTreeMap<(String, String), CompactCapacity> {
		&self.capacities
	}

	pub(crate) fn marker_names(&self) -> &BTreeSet<String> {
		&self.marker_names
	}
}

pub(crate) fn compact_capacity_marker_name(account: &str, field: &str) -> String {
	let account = CamelCaseString::new(account);
	let field = CamelCaseString::new(field);
	let source = format!(
		"{COMPACT_CAPACITY_MARKER_PREFIX}_a{}_{}_f{}_{}",
		account.len(),
		account.as_ref(),
		field.len(),
		field.as_ref(),
	);
	CamelCaseString::new(source).as_ref().to_owned()
}

pub(crate) fn is_compact_capacity_marker(name: &str) -> bool {
	name.starts_with(COMPACT_CAPACITY_MARKER_PREFIX)
}

fn compact_tail_metadata(
	r#type: &TypeNode,
) -> Result<Option<(CompactCapacityKind, Option<usize>)>, String> {
	match r#type {
		TypeNode::Array(array) if matches!(array.count.as_ref(), CountNode::Prefixed(_)) => {
			Ok(Some((CompactCapacityKind::Vec, None)))
		}
		TypeNode::SizePrefix(prefix) if matches!(prefix.r#type.as_ref(), TypeNode::String(_)) => {
			Ok(Some((CompactCapacityKind::String, None)))
		}
		TypeNode::Option(option) if option.fixed != Some(true) => {
			if option.prefix.get_nested_type_node().format != NumberFormat::U8 {
				return Err("compact option tags must use a one-byte u8 prefix".to_owned());
			}
			let Some((kind, _)) = compact_tail_metadata(&option.item)? else {
				return Ok(None);
			};
			let option_tag_offset = compact_option_tag_offset(&option.prefix)?;
			Ok(Some((kind, option_tag_offset)))
		}
		TypeNode::PreOffset(offset) => compact_tail_metadata(&offset.r#type),
		TypeNode::PostOffset(offset) => compact_tail_metadata(&offset.r#type),
		_ => Ok(None),
	}
}

fn compact_option_tag_offset(
	prefix: &NestedTypeNode<NumberTypeNode>,
) -> Result<Option<usize>, String> {
	match prefix {
		NestedTypeNode::Value(_) => Ok(None),
		NestedTypeNode::PostOffset(post)
			if post.strategy == PostOffsetStrategy::PreOffset && post.offset == 0 =>
		{
			let NestedTypeNode::PreOffset(pre) = post.r#type.as_ref() else {
				return Err(
					"compact option prefix must restore its payload cursor after reading the \
					 header tag"
						.to_owned(),
				);
			};
			if pre.strategy != PreOffsetStrategy::Absolute || pre.offset < 0 {
				return Err(
					"compact option prefix must use a non-negative absolute header offset"
						.to_owned(),
				);
			}
			usize::try_from(pre.offset)
				.map(Some)
				.map_err(|_| "compact option header offset does not fit usize".to_owned())
		}
		_ => {
			Err(
				"compact option prefix must be a u8 tag or a canonical shared-header offset"
					.to_owned(),
			)
		}
	}
}
