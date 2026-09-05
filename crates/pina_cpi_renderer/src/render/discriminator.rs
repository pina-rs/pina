//! Constant discriminator extraction for CPI instruction data.

use codama_nodes::BytesEncoding;
use codama_nodes::ConstantDiscriminatorNode;
use codama_nodes::DiscriminatorNode;
use codama_nodes::Endianness;
use codama_nodes::HasKind;
use codama_nodes::NumberFormat;
use codama_nodes::TypeNode;
use codama_nodes::ValueNode;
use heck::ToShoutySnakeCase;

use super::helpers::cast_unsigned;
use super::helpers::decode_base16;
use crate::error::RenderError;
use crate::error::Result;

/// Byte array of a resolved constant discriminator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DiscriminatorBytes {
	pub(crate) name: String,
	pub(crate) bytes: Vec<u8>,
}

/// Resolves the offset-zero constant discriminator of an instruction.
///
/// Only little-endian numeric constants and base16 byte constants are
/// supported; any other discriminator kind is refused instead of generating
/// instruction data that would never dispatch.
pub(crate) fn render_constant_discriminator(
	prefix: &str,
	discriminators: &[DiscriminatorNode],
	context: &str,
) -> Result<DiscriminatorBytes> {
	let constant_discriminator = discriminators
		.iter()
		.find_map(|discriminator| {
			match discriminator {
				DiscriminatorNode::Constant(node) if node.offset == 0 => Some(node),
				_ => None,
			}
		})
		.ok_or(RenderError::MissingDiscriminator {
			context: context.to_string(),
		})?;

	let bytes = discriminator_bytes(constant_discriminator, context)?;
	Ok(DiscriminatorBytes {
		name: format!("{}_DISCRIMINATOR", prefix.to_shouty_snake_case()),
		bytes,
	})
}

fn discriminator_bytes(
	discriminator: &ConstantDiscriminatorNode,
	context: &str,
) -> Result<Vec<u8>> {
	match (
		discriminator.constant.r#type.as_ref(),
		discriminator.constant.value.as_ref(),
	) {
		(TypeNode::Number(number_type), ValueNode::Number(number_value)) => {
			number_discriminator_bytes(number_type, &number_value.number, context)
		}
		(TypeNode::Array(_), ValueNode::Bytes(bytes_value)) => {
			bytes_discriminator_bytes(bytes_value, context)
		}
		(other_type, other_value) => {
			Err(RenderError::UnsupportedDiscriminator {
				context: context.to_string(),
				reason: format!(
					"expected a little-endian number or base16 byte constant, found type `{}` and \
					 value `{}`",
					other_type.kind(),
					other_value.kind(),
				),
			})
		}
	}
}

fn number_discriminator_bytes(
	number_type: &codama_nodes::NumberTypeNode,
	value: &codama_nodes::Number,
	context: &str,
) -> Result<Vec<u8>> {
	if !matches!(number_type.endian, Endianness::Le) {
		return Err(RenderError::UnsupportedDiscriminator {
			context: context.to_string(),
			reason: "only little-endian discriminators are supported".to_string(),
		});
	}

	let value = cast_unsigned(value, u128::from(u64::MAX), context)?;
	let bytes = match number_type.format {
		NumberFormat::U8 => (value as u8).to_le_bytes().to_vec(),
		NumberFormat::U16 => (value as u16).to_le_bytes().to_vec(),
		NumberFormat::U32 => (value as u32).to_le_bytes().to_vec(),
		NumberFormat::U64 => (value as u64).to_le_bytes().to_vec(),
		NumberFormat::I8
		| NumberFormat::I16
		| NumberFormat::I32
		| NumberFormat::I64
		| NumberFormat::U128
		| NumberFormat::I128
		| NumberFormat::F32
		| NumberFormat::F64 => {
			return Err(RenderError::UnsupportedDiscriminator {
				context: context.to_string(),
				reason: format!(
					"unsupported discriminator format `{:?}`",
					number_type.format
				),
			});
		}
		NumberFormat::ShortU16 => {
			return Err(RenderError::UnsupportedDiscriminator {
				context: context.to_string(),
				reason: "short-u16 discriminators are not supported".to_string(),
			});
		}
	};

	Ok(bytes)
}

fn bytes_discriminator_bytes(
	bytes_value: &codama_nodes::BytesValueNode,
	context: &str,
) -> Result<Vec<u8>> {
	match bytes_value.encoding {
		BytesEncoding::Base16 => decode_base16(&bytes_value.data, context),
		BytesEncoding::Base58 | BytesEncoding::Base64 | BytesEncoding::Utf8 => {
			Err(RenderError::UnsupportedDiscriminator {
				context: context.to_string(),
				reason: format!("unsupported byte encoding `{:?}`", bytes_value.encoding),
			})
		}
	}
}
