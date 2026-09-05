//! Constant discriminator extraction for CPI instruction data.

use codama_nodes::BytesEncoding;
use codama_nodes::ConstantDiscriminatorNode;
use codama_nodes::DiscriminatorNode;
use codama_nodes::Endianness;
use codama_nodes::HasKind;
use codama_nodes::InstructionArgumentNode;
use codama_nodes::InstructionInputValueNode;
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

/// Resolves the offset-zero discriminator of an instruction.
///
/// Native Codama roots typically use constant discriminators. Anchor roots use
/// an omitted instruction argument whose default value carries the same
/// bytes. Both forms are resolved before CPI arguments are rendered.
pub(crate) fn render_constant_discriminator(
	prefix: &str,
	discriminators: &[DiscriminatorNode],
	arguments: &[InstructionArgumentNode],
	context: &str,
) -> Result<DiscriminatorBytes> {
	let bytes = if let Some(constant) = discriminators.iter().find_map(|discriminator| {
		match discriminator {
			DiscriminatorNode::Constant(node) if node.offset == 0 => Some(node),
			_ => None,
		}
	}) {
		discriminator_bytes(constant, context)?
	} else if let Some(field) = discriminators.iter().find_map(|discriminator| {
		match discriminator {
			DiscriminatorNode::Field(node) if node.offset == 0 => Some(node),
			_ => None,
		}
	}) {
		let argument = arguments
			.iter()
			.find(|argument| argument.name.as_ref() == field.name.as_ref())
			.ok_or_else(|| {
				unsupported_discriminator(
					context,
					format!("field `{}` has no matching argument", field.name.as_ref()),
				)
			})?;
		field_discriminator_bytes(argument, context)?
	} else {
		return Err(RenderError::MissingDiscriminator {
			context: context.to_string(),
		});
	};

	Ok(DiscriminatorBytes {
		name: format!("{}_DISCRIMINATOR", prefix.to_shouty_snake_case()),
		bytes,
	})
}

fn field_discriminator_bytes(argument: &InstructionArgumentNode, context: &str) -> Result<Vec<u8>> {
	if !matches!(
		argument.default_value_strategy,
		Some(codama_nodes::DefaultValueStrategy::Omitted)
	) {
		return Err(unsupported_discriminator(
			context,
			format!("field argument `{}` is not omitted", argument.name.as_ref()),
		));
	}

	match (
		argument.r#type.as_ref(),
		argument.default_value.as_ref().as_ref(),
	) {
		(TypeNode::Number(number_type), Some(InstructionInputValueNode::NumberValue(value))) => {
			number_discriminator_bytes(number_type, &value.number, context)
		}
		(
			TypeNode::Array(_) | TypeNode::Bytes(_) | TypeNode::FixedSize(_),
			Some(InstructionInputValueNode::BytesValue(value)),
		) => bytes_discriminator_bytes(value, context),
		(r#type, Some(value)) => {
			Err(unsupported_discriminator(
				context,
				format!(
					"field argument `{}` has type `{}` and default value `{}`",
					argument.name.as_ref(),
					r#type.kind(),
					value.kind(),
				),
			))
		}
		(_, None) => {
			Err(unsupported_discriminator(
				context,
				format!(
					"field argument `{}` has no default value",
					argument.name.as_ref()
				),
			))
		}
	}
}

fn unsupported_discriminator(context: &str, reason: String) -> RenderError {
	RenderError::UnsupportedDiscriminator {
		context: context.to_string(),
		reason,
	}
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

#[cfg(test)]
mod tests {
	use codama_nodes::BooleanTypeNode;
	use codama_nodes::BooleanValueNode;
	use codama_nodes::BytesTypeNode;
	use codama_nodes::F32;
	use codama_nodes::F64;
	use codama_nodes::FieldDiscriminatorNode;
	use codama_nodes::I8;
	use codama_nodes::I16;
	use codama_nodes::I32;
	use codama_nodes::I64;
	use codama_nodes::I128;
	use codama_nodes::Number;
	use codama_nodes::NumberTypeNode;
	use codama_nodes::NumberValueNode;
	use codama_nodes::ShortU16;
	use codama_nodes::SizeDiscriminatorNode;
	use codama_nodes::U8;
	use codama_nodes::U16;
	use codama_nodes::U32;
	use codama_nodes::U64;
	use codama_nodes::U128;

	use super::*;

	fn field_argument() -> InstructionArgumentNode {
		let mut argument = InstructionArgumentNode::new(
			"discriminator",
			codama_nodes::FixedSizeTypeNode::new(BytesTypeNode {}, 8),
		);
		argument.default_value_strategy = Some(codama_nodes::DefaultValueStrategy::Omitted);
		argument.default_value = Box::new(Some(
			codama_nodes::BytesValueNode::new(BytesEncoding::Base16, "0011223344556677").into(),
		));
		argument
	}

	#[test]
	fn field_discriminators_require_an_omitted_constant_value() {
		let mut argument = field_argument();
		argument.default_value_strategy = None;
		assert!(field_discriminator_bytes(&argument, "test").is_err());

		argument.default_value_strategy = Some(codama_nodes::DefaultValueStrategy::Omitted);
		*argument.default_value = None;
		assert!(field_discriminator_bytes(&argument, "test").is_err());

		*argument.default_value = Some(NumberValueNode::new(1u8).into());
		assert!(field_discriminator_bytes(&argument, "test").is_err());

		let mut numeric = InstructionArgumentNode::new("discriminator", NumberTypeNode::le(U16));
		numeric.default_value_strategy = Some(codama_nodes::DefaultValueStrategy::Omitted);
		*numeric.default_value = Some(NumberValueNode::new(0x1234u16).into());
		assert_eq!(
			field_discriminator_bytes(&numeric, "test")
				.unwrap_or_else(|error| panic!("numeric field should render: {error}")),
			[0x34, 0x12]
		);
	}

	#[test]
	fn reports_missing_and_nonzero_discriminators() {
		assert!(matches!(
			render_constant_discriminator("test", &[], &[], "test"),
			Err(RenderError::MissingDiscriminator { .. })
		));
		let size = DiscriminatorNode::Size(SizeDiscriminatorNode::new(1));
		assert!(matches!(
			render_constant_discriminator("test", &[size], &[], "test"),
			Err(RenderError::MissingDiscriminator { .. })
		));
		let field = DiscriminatorNode::Field(FieldDiscriminatorNode::new("discriminator", 1));
		assert!(matches!(
			render_constant_discriminator("test", &[field], &[field_argument()], "test"),
			Err(RenderError::MissingDiscriminator { .. })
		));
	}

	#[test]
	fn rejects_mismatched_constant_discriminators() {
		let constant = ConstantDiscriminatorNode::new(
			codama_nodes::ConstantValueNode::new(
				BooleanTypeNode::default(),
				BooleanValueNode::new(true),
			),
			0,
		);
		assert!(discriminator_bytes(&constant, "test").is_err());
	}

	#[test]
	fn renders_supported_numeric_discriminators() {
		for (format, expected) in [
			(U8, vec![1]),
			(U16, vec![1, 0]),
			(U32, vec![1, 0, 0, 0]),
			(U64, vec![1, 0, 0, 0, 0, 0, 0, 0]),
		] {
			assert_eq!(
				number_discriminator_bytes(&NumberTypeNode::le(format), &Number::from(1u8), "test")
					.unwrap_or_else(|error| panic!("number should render: {error}")),
				expected
			);
		}
	}

	#[test]
	fn rejects_unsupported_numeric_discriminators() {
		assert!(
			number_discriminator_bytes(&NumberTypeNode::be(U16), &Number::from(1u8), "test")
				.is_err()
		);
		for format in [I8, I16, I32, I64, U128, I128, F32, F64, ShortU16] {
			assert!(
				number_discriminator_bytes(&NumberTypeNode::le(format), &Number::from(1u8), "test")
					.is_err()
			);
		}
	}

	#[test]
	fn rejects_non_base16_byte_discriminators() {
		for encoding in [
			BytesEncoding::Base58,
			BytesEncoding::Base64,
			BytesEncoding::Utf8,
		] {
			assert!(
				bytes_discriminator_bytes(
					&codama_nodes::BytesValueNode::new(encoding, "value"),
					"test"
				)
				.is_err()
			);
		}
	}
}
