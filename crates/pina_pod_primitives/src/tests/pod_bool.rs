use super::*;

#[test]
fn pod_bool_roundtrip() {
	for i in 0..=u8::MAX {
		let value = *try_from_bytes::<PodBool>(&[i]).unwrap();
		assert_eq!(i != 0, bool::from(value));
	}
}

#[test]
fn pod_bool_non_canonical_equality_mismatch() {
	let canonical_true = PodBool::from_bool(true);
	let non_canonical_true = *try_from_bytes::<PodBool>(&[2]).unwrap();

	assert!(bool::from(canonical_true));
	assert!(bool::from(non_canonical_true));

	assert_ne!(canonical_true, non_canonical_true);

	assert!(canonical_true.is_canonical());
	assert!(!non_canonical_true.is_canonical());
}

#[test]
fn pod_bool_is_canonical_boundary_values() {
	assert!(PodBool(0).is_canonical());
	assert!(PodBool(1).is_canonical());
	assert!(!PodBool(2).is_canonical());
	assert!(!PodBool(127).is_canonical());
	assert!(!PodBool(255).is_canonical());
}

#[test]
fn pod_bool_from_bool_produces_canonical() {
	assert!(PodBool::from_bool(false).is_canonical());
	assert!(PodBool::from_bool(true).is_canonical());
	assert!(PodBool::from(false).is_canonical());
	assert!(PodBool::from(true).is_canonical());
}

#[test]
fn pod_bool_from_ref() {
	let t = true;
	let f = false;
	assert_eq!(PodBool::from(&t), PodBool(1));
	assert_eq!(PodBool::from(&f), PodBool(0));
}

#[test]
fn pod_bool_from_ref_roundtrip() {
	let pod = PodBool(1);
	assert!(bool::from(&pod));
	let pod = PodBool(0);
	assert!(!bool::from(&pod));
}

#[test]
fn pod_bool_default_is_false() {
	let default = PodBool::default();
	assert_eq!(default.0, 0);
	assert!(!bool::from(default));
	assert!(default.is_canonical());
}

#[test]
fn pod_bool_not() {
	assert_eq!(!PodBool::from_bool(true), PodBool::from_bool(false));
	assert_eq!(!PodBool::from_bool(false), PodBool::from_bool(true));
	assert_eq!(!PodBool(42), PodBool::from_bool(false));
}

#[test]
fn pod_bool_display() {
	assert_eq!(std::format!("{}", PodBool::from_bool(true)), "true");
	assert_eq!(std::format!("{}", PodBool::from_bool(false)), "false");
}
