use codama_nodes::ConstantDiscriminatorNode;
use codama_nodes::DiscriminatorNode;
use codama_nodes::Endian;
use codama_nodes::HasKind;
use codama_nodes::Number;
use codama_nodes::NumberFormat;
use codama_nodes::NumberTypeNode;
use codama_nodes::TypeNode;
use codama_nodes::ValueNode;

use super::helpers::cast_signed;
use super::helpers::cast_unsigned;
use super::helpers::shouty;
use crate::error::RenderError;
use crate::error::Result;

#[derive(Clone, Debug)]
pub(crate) struct DiscriminatorInfo {
	pub(crate) name: String,
	pub(crate) ty: String,
	pub(crate) value: String,
}

pub(crate) fn render_constant_discriminator(
	prefix: &str,
	discriminators: &[DiscriminatorNode],
	context: &str,
) -> Result<Option<DiscriminatorInfo>> {
	let Some(constant_discriminator) = discriminators.iter().find_map(|discriminator| {
		match discriminator {
			DiscriminatorNode::Constant(node) if node.offset == 0 => Some(node),
			_ => None,
		}
	}) else {
		return Ok(None);
	};

	let (ty, value) = render_constant_discriminator_value(constant_discriminator, context)?;
	Ok(Some(DiscriminatorInfo {
		name: format!("{}_DISCRIMINATOR", shouty(prefix)),
		ty,
		value,
	}))
}

fn render_constant_discriminator_value(
	discriminator: &ConstantDiscriminatorNode,
	context: &str,
) -> Result<(String, String)> {
	let number_type = match discriminator.constant.r#type.as_ref() {
		TypeNode::Number(number_type) => number_type,
		other => {
			return Err(RenderError::UnsupportedDiscriminator {
				context: context.to_string(),
				reason: format!("expected number type, found `{}`", other.kind()),
			});
		}
	};

	let number_value = match discriminator.constant.value.as_ref() {
		ValueNode::Number(number_value) => &number_value.number,
		other => {
			return Err(RenderError::UnsupportedDiscriminator {
				context: context.to_string(),
				reason: format!("expected numeric value, found `{}`", other.kind()),
			});
		}
	};

	let ty = render_discriminator_type(number_type, context)?;
	let value = render_discriminator_literal(number_type, number_value, context)?;
	Ok((ty, value))
}

fn render_discriminator_type(number_type: &NumberTypeNode, context: &str) -> Result<String> {
	if !matches!(number_type.endian, Endian::Little) {
		return Err(RenderError::UnsupportedDiscriminator {
			context: context.to_string(),
			reason: "only little-endian discriminators are supported".to_string(),
		});
	}

	match number_type.format {
		NumberFormat::U8 => Ok("u8".to_string()),
		NumberFormat::I8 => Ok("i8".to_string()),
		NumberFormat::U16 => Ok("pina_pod_primitives::PodU16".to_string()),
		NumberFormat::I16 => Ok("pina_pod_primitives::PodI16".to_string()),
		NumberFormat::U32 => Ok("pina_pod_primitives::PodU32".to_string()),
		NumberFormat::I32 => Ok("pina_pod_primitives::PodI32".to_string()),
		NumberFormat::U64 => Ok("pina_pod_primitives::PodU64".to_string()),
		NumberFormat::I64 => Ok("pina_pod_primitives::PodI64".to_string()),
		NumberFormat::U128 => Ok("pina_pod_primitives::PodU128".to_string()),
		NumberFormat::I128 => Ok("pina_pod_primitives::PodI128".to_string()),
		NumberFormat::F32 | NumberFormat::F64 | NumberFormat::ShortU16 => {
			Err(RenderError::UnsupportedDiscriminator {
				context: context.to_string(),
				reason: format!(
					"unsupported discriminator format `{:?}`",
					number_type.format
				),
			})
		}
	}
}

fn render_discriminator_literal(
	number_type: &NumberTypeNode,
	value: &Number,
	context: &str,
) -> Result<String> {
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
			return Err(RenderError::UnsupportedDiscriminator {
				context: context.to_string(),
				reason: "float/shortU16 discriminators are unsupported".to_string(),
			});
		}
	};

	Ok(match number_type.format {
		NumberFormat::U8 | NumberFormat::I8 => literal,
		NumberFormat::U16 => format!("pina_pod_primitives::PodU16::from({literal})"),
		NumberFormat::I16 => format!("pina_pod_primitives::PodI16::from({literal})"),
		NumberFormat::U32 => format!("pina_pod_primitives::PodU32::from({literal})"),
		NumberFormat::I32 => format!("pina_pod_primitives::PodI32::from({literal})"),
		NumberFormat::U64 => format!("pina_pod_primitives::PodU64::from({literal})"),
		NumberFormat::I64 => format!("pina_pod_primitives::PodI64::from({literal})"),
		NumberFormat::U128 => format!("pina_pod_primitives::PodU128::from({literal})"),
		NumberFormat::I128 => format!("pina_pod_primitives::PodI128::from({literal})"),
		NumberFormat::F32 | NumberFormat::F64 | NumberFormat::ShortU16 => unreachable!(),
	})
}
