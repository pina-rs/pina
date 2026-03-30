use codama_nodes::CountNode;
use codama_nodes::Endian;
use codama_nodes::HasKind;
use codama_nodes::NestedTypeNodeTrait;
use codama_nodes::Number;
use codama_nodes::NumberFormat;
use codama_nodes::TypeNode;
use codama_nodes::ValueNode;

use super::helpers::cast_signed;
use super::helpers::cast_unsigned;
use super::types::render_type_for_pod;
use crate::error::RenderError;
use crate::error::Result;

pub(crate) fn render_variable_seed_parameter(
	seed_name: &str,
	r#type: &TypeNode,
	context: &str,
) -> Result<(String, String)> {
	match r#type {
		TypeNode::PublicKey(_) => {
			Ok((
				"&solana_pubkey::Pubkey".to_string(),
				format!("{seed_name}.as_ref()"),
			))
		}
		TypeNode::Boolean(boolean_type) => {
			let number_type = boolean_type.size.get_nested_type_node();
			if !matches!(number_type.format, NumberFormat::U8)
				|| !matches!(number_type.endian, Endian::Little)
			{
				return Err(RenderError::UnsupportedType {
					context: context.to_string(),
					kind: "booleanTypeNode",
					reason: "booleans must use little-endian u8 for PDA seeds".to_string(),
				});
			}
			Ok(("bool".to_string(), format!("&[u8::from({seed_name})]")))
		}
		TypeNode::Number(number_type) => {
			if !matches!(number_type.endian, Endian::Little) {
				return Err(RenderError::UnsupportedType {
					context: context.to_string(),
					kind: "numberTypeNode",
					reason: "only little-endian numbers are supported for PDA seeds".to_string(),
				});
			}
			let number_ty = match number_type.format {
				NumberFormat::U8 => "u8",
				NumberFormat::I8 => "i8",
				NumberFormat::U16 => "u16",
				NumberFormat::I16 => "i16",
				NumberFormat::U32 => "u32",
				NumberFormat::I32 => "i32",
				NumberFormat::U64 => "u64",
				NumberFormat::I64 => "i64",
				NumberFormat::U128 => "u128",
				NumberFormat::I128 => "i128",
				NumberFormat::F32 | NumberFormat::F64 | NumberFormat::ShortU16 => {
					return Err(RenderError::UnsupportedType {
						context: context.to_string(),
						kind: "numberTypeNode",
						reason: "float/shortU16 seed numbers are unsupported".to_string(),
					});
				}
			};
			let bytes_expr = format!("&{seed_name}.to_le_bytes()");
			Ok((number_ty.to_string(), bytes_expr))
		}
		TypeNode::FixedSize(fixed_size) => {
			if matches!(fixed_size.r#type.as_ref(), TypeNode::Bytes(_)) {
				Ok((
					format!("&[u8; {}]", fixed_size.size),
					format!("&{seed_name}[..]"),
				))
			} else {
				let inner = render_type_for_pod(&fixed_size.r#type, context)?;
				Ok((
					format!("&[{inner}; {}]", fixed_size.size),
					format!("&{seed_name}[..]"),
				))
			}
		}
		TypeNode::Array(array) => {
			match &array.count {
				CountNode::Fixed(count) => {
					let inner = render_type_for_pod(&array.item, context)?;
					Ok((
						format!("&[{inner}; {}]", count.value),
						format!("&{seed_name}[..]"),
					))
				}
				CountNode::Prefixed(_) | CountNode::Remainder(_) => {
					Err(RenderError::UnsupportedType {
						context: context.to_string(),
						kind: "arrayTypeNode",
						reason: "only fixed arrays are supported for PDA seeds".to_string(),
					})
				}
			}
		}
		other => {
			Err(RenderError::UnsupportedType {
				context: context.to_string(),
				kind: other.kind(),
				reason: "unsupported variable PDA seed type".to_string(),
			})
		}
	}
}

pub(crate) fn render_constant_seed_expression(
	r#type: &TypeNode,
	value: &ValueNode,
	context: &str,
	primary_program_const: &str,
) -> Result<String> {
	match value {
		ValueNode::String(string_value) => Ok(format!("{:?}.as_bytes()", string_value.string)),
		ValueNode::Number(number_value) => {
			render_number_seed_expression(r#type, &number_value.number, context)
		}
		ValueNode::PublicKey(public_key_value) => {
			Ok(format!(
				"solana_pubkey::pubkey!(\"{}\").as_ref()",
				public_key_value.public_key
			))
		}
		ValueNode::Constant(constant_value) => {
			match constant_value.value.as_ref() {
				ValueNode::String(string_value) => {
					Ok(format!("{:?}.as_bytes()", string_value.string))
				}
				ValueNode::Number(number_value) => {
					render_number_seed_expression(
						constant_value.r#type.as_ref(),
						&number_value.number,
						context,
					)
				}
				other => {
					Err(RenderError::UnsupportedValue {
						context: context.to_string(),
						kind: other.kind(),
						reason: "unsupported nested constant seed value".to_string(),
					})
				}
			}
		}
		ValueNode::Bytes(bytes_value) => {
			if matches!(bytes_value.encoding, codama_nodes::BytesEncoding::Utf8) {
				Ok(format!("{:?}.as_bytes()", bytes_value.data))
			} else {
				Err(RenderError::UnsupportedValue {
					context: context.to_string(),
					kind: value.kind(),
					reason: "non-utf8 bytes seeds are unsupported".to_string(),
				})
			}
		}
		other => {
			Err(RenderError::UnsupportedValue {
				context: context.to_string(),
				kind: other.kind(),
				reason: format!(
					"supported constant seed values are string/number/publicKey (program const: \
					 {primary_program_const})"
				),
			})
		}
	}
}

pub(crate) fn render_number_seed_expression(
	r#type: &TypeNode,
	value: &Number,
	context: &str,
) -> Result<String> {
	let TypeNode::Number(number_type) = r#type else {
		return Err(RenderError::UnsupportedType {
			context: context.to_string(),
			kind: r#type.kind(),
			reason: "numeric seed value requires number type".to_string(),
		});
	};
	if !matches!(number_type.endian, Endian::Little) {
		return Err(RenderError::UnsupportedType {
			context: context.to_string(),
			kind: "numberTypeNode",
			reason: "only little-endian numeric seeds are supported".to_string(),
		});
	}
	let literal = match number_type.format {
		NumberFormat::U8 => format!("{}u8", cast_unsigned(value, u128::from(u8::MAX), context)?),
		NumberFormat::I8 => {
			format!(
				"{}i8",
				cast_signed(value, i128::from(i8::MIN), i128::from(i8::MAX), context)?
			)
		}
		NumberFormat::U16 => {
			format!(
				"{}u16",
				cast_unsigned(value, u128::from(u16::MAX), context)?
			)
		}
		NumberFormat::I16 => {
			format!(
				"{}i16",
				cast_signed(value, i128::from(i16::MIN), i128::from(i16::MAX), context)?
			)
		}
		NumberFormat::U32 => {
			format!(
				"{}u32",
				cast_unsigned(value, u128::from(u32::MAX), context)?
			)
		}
		NumberFormat::I32 => {
			format!(
				"{}i32",
				cast_signed(value, i128::from(i32::MIN), i128::from(i32::MAX), context)?
			)
		}
		NumberFormat::U64 => {
			format!(
				"{}u64",
				cast_unsigned(value, u128::from(u64::MAX), context)?
			)
		}
		NumberFormat::I64 => {
			format!(
				"{}i64",
				cast_signed(value, i128::from(i64::MIN), i128::from(i64::MAX), context)?
			)
		}
		NumberFormat::U128 => format!("{}u128", cast_unsigned(value, u128::MAX, context)?),
		NumberFormat::I128 => {
			format!("{}i128", cast_signed(value, i128::MIN, i128::MAX, context)?)
		}
		NumberFormat::F32 | NumberFormat::F64 | NumberFormat::ShortU16 => {
			return Err(RenderError::UnsupportedType {
				context: context.to_string(),
				kind: "numberTypeNode",
				reason: "float/shortU16 numeric seeds are unsupported".to_string(),
			});
		}
	};
	Ok(format!("&{literal}.to_le_bytes()"))
}
