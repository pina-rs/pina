use super::*;

#[test]
fn pod_u16_roundtrip() {
	assert_eq!(1u16, u16::from(PodU16::from_primitive(1)));
}

#[test]
fn pod_i16_roundtrip() {
	assert_eq!(-1i16, i16::from(PodI16::from_primitive(-1)));
}

#[test]
fn pod_u32_roundtrip() {
	assert_eq!(7u32, u32::from(PodU32::from_primitive(7)));
}

#[test]
fn pod_i32_roundtrip() {
	assert_eq!(-7i32, i32::from(PodI32::from_primitive(-7)));
}

#[test]
fn pod_u64_roundtrip() {
	assert_eq!(9u64, u64::from(PodU64::from_primitive(9)));
}

#[test]
fn pod_i64_roundtrip() {
	assert_eq!(-9i64, i64::from(PodI64::from_primitive(-9)));
}

#[test]
fn pod_u128_roundtrip() {
	assert_eq!(11u128, u128::from(PodU128::from_primitive(11)));
}

#[test]
fn pod_i128_roundtrip() {
	assert_eq!(-11i128, i128::from(PodI128::from_primitive(-11)));
}

#[test]
fn pod_u16_boundary_values() {
	assert_eq!(0u16, u16::from(PodU16::from_primitive(0)));
	assert_eq!(u16::MAX, u16::from(PodU16::from_primitive(u16::MAX)));
}

#[test]
fn pod_i16_boundary_values() {
	assert_eq!(i16::MIN, i16::from(PodI16::from_primitive(i16::MIN)));
	assert_eq!(i16::MAX, i16::from(PodI16::from_primitive(i16::MAX)));
	assert_eq!(0i16, i16::from(PodI16::from_primitive(0)));
}

#[test]
fn pod_u32_boundary_values() {
	assert_eq!(0u32, u32::from(PodU32::from_primitive(0)));
	assert_eq!(u32::MAX, u32::from(PodU32::from_primitive(u32::MAX)));
}

#[test]
fn pod_i32_boundary_values() {
	assert_eq!(i32::MIN, i32::from(PodI32::from_primitive(i32::MIN)));
	assert_eq!(i32::MAX, i32::from(PodI32::from_primitive(i32::MAX)));
}

#[test]
fn pod_u64_boundary_values() {
	assert_eq!(0u64, u64::from(PodU64::from_primitive(0)));
	assert_eq!(u64::MAX, u64::from(PodU64::from_primitive(u64::MAX)));
}

#[test]
fn pod_i64_boundary_values() {
	assert_eq!(i64::MIN, i64::from(PodI64::from_primitive(i64::MIN)));
	assert_eq!(i64::MAX, i64::from(PodI64::from_primitive(i64::MAX)));
}

#[test]
fn pod_u128_boundary_values() {
	assert_eq!(0u128, u128::from(PodU128::from_primitive(0)));
	assert_eq!(u128::MAX, u128::from(PodU128::from_primitive(u128::MAX)));
}

#[test]
fn pod_i128_boundary_values() {
	assert_eq!(i128::MIN, i128::from(PodI128::from_primitive(i128::MIN)));
	assert_eq!(i128::MAX, i128::from(PodI128::from_primitive(i128::MAX)));
}

#[test]
fn pod_types_use_little_endian_byte_order() {
	let u16_val = PodU16::from_primitive(0x0102);
	assert_eq!(u16_val.0, [0x02, 0x01]);

	let u32_val = PodU32::from_primitive(0x01020304);
	assert_eq!(u32_val.0, [0x04, 0x03, 0x02, 0x01]);

	let u64_val = PodU64::from_primitive(0x0102030405060708);
	assert_eq!(u64_val.0, [0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]);
}

#[test]
fn pod_types_bytemuck_from_bytes() {
	let bytes_u16 = [0x39, 0x05];
	let val = try_from_bytes::<PodU16>(&bytes_u16).unwrap();
	assert_eq!(u16::from(*val), 1337);

	let bytes_u32 = [0xEF, 0xBE, 0xAD, 0xDE];
	let val = try_from_bytes::<PodU32>(&bytes_u32).unwrap();
	assert_eq!(u32::from(*val), 0xDEAD_BEEF);

	let bytes_i16 = [0xFF, 0xFF];
	let val = try_from_bytes::<PodI16>(&bytes_i16).unwrap();
	assert_eq!(i16::from(*val), -1);
}

#[test]
fn pod_default_is_zero() {
	assert_eq!(u16::from(PodU16::default()), 0);
	assert_eq!(i16::from(PodI16::default()), 0);
	assert_eq!(u32::from(PodU32::default()), 0);
	assert_eq!(i32::from(PodI32::default()), 0);
	assert_eq!(u64::from(PodU64::default()), 0);
	assert_eq!(i64::from(PodI64::default()), 0);
	assert_eq!(u128::from(PodU128::default()), 0);
	assert_eq!(i128::from(PodI128::default()), 0);
}

#[test]
fn pod_constants_zero() {
	assert!(PodU16::ZERO.is_zero());
	assert!(PodU32::ZERO.is_zero());
	assert!(PodU64::ZERO.is_zero());
	assert!(PodU128::ZERO.is_zero());
	assert!(PodI16::ZERO.is_zero());
	assert!(PodI32::ZERO.is_zero());
	assert!(PodI64::ZERO.is_zero());
	assert!(PodI128::ZERO.is_zero());
}

#[test]
fn pod_constants_min_max() {
	assert_eq!(PodU16::MIN.get(), u16::MIN);
	assert_eq!(PodU16::MAX.get(), u16::MAX);
	assert_eq!(PodU32::MIN.get(), u32::MIN);
	assert_eq!(PodU32::MAX.get(), u32::MAX);
	assert_eq!(PodU64::MIN.get(), u64::MIN);
	assert_eq!(PodU64::MAX.get(), u64::MAX);
	assert_eq!(PodU128::MIN.get(), u128::MIN);
	assert_eq!(PodU128::MAX.get(), u128::MAX);
	assert_eq!(PodI16::MIN.get(), i16::MIN);
	assert_eq!(PodI16::MAX.get(), i16::MAX);
	assert_eq!(PodI32::MIN.get(), i32::MIN);
	assert_eq!(PodI32::MAX.get(), i32::MAX);
	assert_eq!(PodI64::MIN.get(), i64::MIN);
	assert_eq!(PodI64::MAX.get(), i64::MAX);
	assert_eq!(PodI128::MIN.get(), i128::MIN);
	assert_eq!(PodI128::MAX.get(), i128::MAX);
}

#[test]
fn pod_is_zero_false_for_nonzero() {
	assert!(!PodU64::from_primitive(1).is_zero());
	assert!(!PodI64::from_primitive(-1).is_zero());
	assert!(!PodU128::MAX.is_zero());
}

#[test]
fn pod_add_native() {
	assert_eq!((PodU64::from(10u64) + 5u64).get(), 15);
	assert_eq!((PodI32::from(10i32) + 5i32).get(), 15);
	assert_eq!((PodI32::from(-10i32) + 5i32).get(), -5);
}

#[test]
fn pod_add_pod() {
	let a = PodU64::from(10u64);
	let b = PodU64::from(20u64);
	assert_eq!((a + b).get(), 30);
}

#[test]
fn pod_sub_native() {
	assert_eq!((PodU64::from(10u64) - 5u64).get(), 5);
	assert_eq!((PodI32::from(-10i32) - 5i32).get(), -15);
}

#[test]
fn pod_sub_pod() {
	let a = PodU64::from(20u64);
	let b = PodU64::from(5u64);
	assert_eq!((a - b).get(), 15);
}

#[test]
fn pod_mul_native() {
	assert_eq!((PodU64::from(6u64) * 7u64).get(), 42);
	assert_eq!((PodI32::from(-3i32) * 4i32).get(), -12);
}

#[test]
fn pod_mul_pod() {
	let a = PodU32::from(6u32);
	let b = PodU32::from(7u32);
	assert_eq!((a * b).get(), 42);
}

#[test]
fn pod_div_native() {
	assert_eq!((PodU64::from(42u64) / 7u64).get(), 6);
	assert_eq!((PodI32::from(-12i32) / 4i32).get(), -3);
}

#[test]
fn pod_div_pod() {
	let a = PodU64::from(42u64);
	let b = PodU64::from(7u64);
	assert_eq!((a / b).get(), 6);
}

#[test]
fn pod_rem_native() {
	assert_eq!((PodU64::from(10u64) % 3u64).get(), 1);
	assert_eq!((PodI32::from(-10i32) % 3i32).get(), -1);
}

#[test]
fn pod_rem_pod() {
	let a = PodU64::from(10u64);
	let b = PodU64::from(3u64);
	assert_eq!((a % b).get(), 1);
}

#[test]
fn pod_add_assign_native() {
	let mut v = PodU64::from(10u64);
	v += 5u64;
	assert_eq!(v.get(), 15);
}

#[test]
fn pod_add_assign_pod() {
	let mut v = PodU64::from(10u64);
	v += PodU64::from(5u64);
	assert_eq!(v.get(), 15);
}

#[test]
fn pod_sub_assign_native() {
	let mut v = PodU64::from(10u64);
	v -= 3u64;
	assert_eq!(v.get(), 7);
}

#[test]
fn pod_sub_assign_pod() {
	let mut v = PodU64::from(10u64);
	v -= PodU64::from(3u64);
	assert_eq!(v.get(), 7);
}

#[test]
fn pod_mul_assign_native() {
	let mut v = PodU32::from(5u32);
	v *= 4u32;
	assert_eq!(v.get(), 20);
}

#[test]
fn pod_mul_assign_pod() {
	let mut v = PodU32::from(5u32);
	v *= PodU32::from(4u32);
	assert_eq!(v.get(), 20);
}

#[test]
fn pod_div_assign_native() {
	let mut v = PodU64::from(20u64);
	v /= 5u64;
	assert_eq!(v.get(), 4);
}

#[test]
fn pod_div_assign_pod() {
	let mut v = PodU64::from(20u64);
	v /= PodU64::from(5u64);
	assert_eq!(v.get(), 4);
}

#[test]
fn pod_rem_assign_native() {
	let mut v = PodU64::from(10u64);
	v %= 3u64;
	assert_eq!(v.get(), 1);
}

#[test]
fn pod_rem_assign_pod() {
	let mut v = PodU64::from(10u64);
	v %= PodU64::from(3u64);
	assert_eq!(v.get(), 1);
}

#[test]
fn pod_bitand_native() {
	assert_eq!((PodU32::from(0xFF00u32) & 0x0FF0u32).get(), 0x0F00);
}

#[test]
fn pod_bitand_pod() {
	let a = PodU32::from(0xFF00u32);
	let b = PodU32::from(0x0FF0u32);
	assert_eq!((a & b).get(), 0x0F00);
}

#[test]
fn pod_bitor_native() {
	assert_eq!((PodU32::from(0xFF00u32) | 0x00FFu32).get(), 0xFFFF);
}

#[test]
fn pod_bitor_pod() {
	let a = PodU32::from(0xFF00u32);
	let b = PodU32::from(0x00FFu32);
	assert_eq!((a | b).get(), 0xFFFF);
}

#[test]
fn pod_bitxor_native() {
	assert_eq!((PodU32::from(0xFFFFu32) ^ 0xFF00u32).get(), 0x00FF);
}

#[test]
fn pod_bitxor_pod() {
	let a = PodU32::from(0xFFFFu32);
	let b = PodU32::from(0xFF00u32);
	assert_eq!((a ^ b).get(), 0x00FF);
}

#[test]
fn pod_shl() {
	assert_eq!((PodU32::from(1u32) << 4).get(), 16);
}

#[test]
fn pod_shr() {
	assert_eq!((PodU32::from(16u32) >> 4).get(), 1);
}

#[test]
fn pod_not() {
	assert_eq!((!PodU16::from(0u16)).get(), u16::MAX);
	assert_eq!((!PodI16::from(0i16)).get(), -1i16);
}

#[test]
fn pod_bitand_assign_native() {
	let mut v = PodU32::from(0xFF00u32);
	v &= 0x0FF0u32;
	assert_eq!(v.get(), 0x0F00);
}

#[test]
fn pod_bitand_assign_pod() {
	let mut v = PodU32::from(0xFF00u32);
	v &= PodU32::from(0x0FF0u32);
	assert_eq!(v.get(), 0x0F00);
}

#[test]
fn pod_bitor_assign_native() {
	let mut v = PodU32::from(0xFF00u32);
	v |= 0x00FFu32;
	assert_eq!(v.get(), 0xFFFF);
}

#[test]
fn pod_bitor_assign_pod() {
	let mut v = PodU32::from(0xFF00u32);
	v |= PodU32::from(0x00FFu32);
	assert_eq!(v.get(), 0xFFFF);
}

#[test]
fn pod_bitxor_assign_native() {
	let mut v = PodU32::from(0xFFFFu32);
	v ^= 0xFF00u32;
	assert_eq!(v.get(), 0x00FF);
}

#[test]
fn pod_bitxor_assign_pod() {
	let mut v = PodU32::from(0xFFFFu32);
	v ^= PodU32::from(0xFF00u32);
	assert_eq!(v.get(), 0x00FF);
}

#[test]
fn pod_shl_assign() {
	let mut v = PodU32::from(1u32);
	v <<= 4;
	assert_eq!(v.get(), 16);
}

#[test]
fn pod_shr_assign() {
	let mut v = PodU32::from(16u32);
	v >>= 4;
	assert_eq!(v.get(), 1);
}

#[test]
fn pod_neg_i16() {
	assert_eq!((-PodI16::from(5i16)).get(), -5);
	assert_eq!((-PodI16::from(-5i16)).get(), 5);
	assert_eq!((-PodI16::from(0i16)).get(), 0);
}

#[test]
fn pod_neg_i32() {
	assert_eq!((-PodI32::from(42i32)).get(), -42);
}

#[test]
fn pod_neg_i64() {
	assert_eq!((-PodI64::from(100i64)).get(), -100);
}

#[test]
fn pod_neg_i128() {
	assert_eq!((-PodI128::from(999i128)).get(), -999);
}

#[test]
fn pod_checked_add_ok() {
	assert_eq!(
		PodU64::from(10u64).checked_add(5u64),
		Some(PodU64::from(15u64))
	);
}

#[test]
fn pod_checked_add_overflow() {
	assert_eq!(PodU64::MAX.checked_add(1u64), None);
}

#[test]
fn pod_checked_add_pod() {
	assert_eq!(
		PodU32::from(10u32).checked_add(PodU32::from(5u32)),
		Some(PodU32::from(15u32))
	);
}

#[test]
fn pod_checked_sub_ok() {
	assert_eq!(
		PodU64::from(10u64).checked_sub(5u64),
		Some(PodU64::from(5u64))
	);
}

#[test]
fn pod_checked_sub_underflow() {
	assert_eq!(PodU64::from(5u64).checked_sub(10u64), None);
}

#[test]
fn pod_checked_mul_ok() {
	assert_eq!(
		PodU64::from(6u64).checked_mul(7u64),
		Some(PodU64::from(42u64))
	);
}

#[test]
fn pod_checked_mul_overflow() {
	assert_eq!(PodU64::MAX.checked_mul(2u64), None);
}

#[test]
fn pod_checked_div_ok() {
	assert_eq!(
		PodU64::from(42u64).checked_div(7u64),
		Some(PodU64::from(6u64))
	);
}

#[test]
fn pod_checked_div_by_zero() {
	assert_eq!(PodU64::from(42u64).checked_div(0u64), None);
}

#[test]
fn pod_checked_signed_overflow() {
	assert_eq!(PodI64::MIN.checked_sub(1i64), None);
	assert_eq!(PodI64::MAX.checked_add(1i64), None);
}

#[test]
fn pod_saturating_add() {
	assert_eq!(PodU64::MAX.saturating_add(100u64), PodU64::MAX);
	assert_eq!(
		PodU64::from(10u64).saturating_add(5u64),
		PodU64::from(15u64)
	);
}

#[test]
fn pod_saturating_sub() {
	assert_eq!(PodU64::from(5u64).saturating_sub(10u64), PodU64::ZERO);
	assert_eq!(PodU64::from(10u64).saturating_sub(5u64), PodU64::from(5u64));
}

#[test]
fn pod_saturating_mul() {
	assert_eq!(PodU64::MAX.saturating_mul(2u64), PodU64::MAX);
	assert_eq!(PodU64::from(6u64).saturating_mul(7u64), PodU64::from(42u64));
}

#[test]
fn pod_saturating_signed() {
	assert_eq!(PodI64::MAX.saturating_add(100i64), PodI64::MAX);
	assert_eq!(PodI64::MIN.saturating_sub(100i64), PodI64::MIN);
	assert_eq!(PodI64::MAX.saturating_mul(2i64), PodI64::MAX);
	assert_eq!(PodI64::MIN.saturating_mul(2i64), PodI64::MIN);
}

#[test]
fn pod_ordering() {
	assert!(PodU64::from(10u64) > PodU64::from(5u64));
	assert!(PodU64::from(5u64) < PodU64::from(10u64));
	assert!(PodU64::from(5u64) == PodU64::from(5u64));

	assert!(PodI64::from(-10i64) < PodI64::from(5i64));
	assert!(PodI64::from(5i64) > PodI64::from(-10i64));
}

#[test]
fn pod_partial_eq_native() {
	assert!(PodU64::from(42u64) == 42u64);
	assert!(PodI32::from(-5i32) == -5i32);
	assert!(PodU64::from(42u64) != 43u64);
}

#[test]
fn pod_partial_ord_native() {
	assert!(PodU64::from(10u64) > 5u64);
	assert!(PodU64::from(5u64) < 10u64);
	assert!(PodI32::from(-10i32) < 0i32);
}

#[test]
fn pod_display() {
	assert_eq!(std::format!("{}", PodU64::from(42u64)), "42");
	assert_eq!(std::format!("{}", PodI32::from(-7i32)), "-7");
	assert_eq!(std::format!("{}", PodU128::from(0u128)), "0");
}

#[test]
fn pod_debug() {
	assert_eq!(std::format!("{:?}", PodU64::from(42u64)), "PodU64(42)");
	assert_eq!(std::format!("{:?}", PodI32::from(-7i32)), "PodI32(-7)");
}

#[test]
fn pod_get_method() {
	assert_eq!(PodU16::from(1337u16).get(), 1337);
	assert_eq!(PodI16::from(-42i16).get(), -42);
	assert_eq!(PodU32::from(0xDEAD_BEEFu32).get(), 0xDEAD_BEEF);
	assert_eq!(PodI32::from(i32::MIN).get(), i32::MIN);
	assert_eq!(PodU64::from(u64::MAX).get(), u64::MAX);
	assert_eq!(PodI64::from(i64::MAX).get(), i64::MAX);
	assert_eq!(PodU128::from(u128::MAX).get(), u128::MAX);
	assert_eq!(PodI128::from(i128::MIN).get(), i128::MIN);
}

#[test]
fn ergonomic_counter_increment() {
	let mut count = PodU64::from(0u64);
	count += 1u64;
	assert_eq!(count.get(), 1);
	count += 1u64;
	assert_eq!(count.get(), 2);
}

#[test]
fn ergonomic_balance_arithmetic() {
	let mut balance = PodU64::from(1000u64);
	let fee = PodU64::from(25u64);
	balance -= fee;
	assert_eq!(balance.get(), 975);
}
