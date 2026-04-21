use core::mem::size_of;

use super::*;

#[test]
fn pod_option_none() {
	let opt = PodOption::<PodU64>::none();
	assert!(opt.is_none());
	assert!(!opt.is_some());
	assert_eq!(opt.get(), None);
}

#[test]
fn pod_option_some() {
	let opt = PodOption::some(PodU64::from(42u64));
	assert!(!opt.is_none());
	assert!(opt.is_some());
	assert_eq!(opt.get(), Some(PodU64::from(42u64)));
}

#[test]
fn pod_option_set_and_clear() {
	let mut opt = PodOption::<PodU64>::none();
	opt.set(PodU64::from(100u64));
	assert!(opt.is_some());
	assert_eq!(opt.get(), Some(PodU64::from(100u64)));
	opt.clear();
	assert!(opt.is_none());
}

#[test]
fn pod_option_default_is_none() {
	let opt = PodOption::<PodU64>::default();
	assert!(opt.is_none());
}

#[test]
fn pod_option_bytemuck_roundtrip() {
	let opt = PodOption::some(PodU64::from(0xDEAD_BEEF_u64));
	let bytes: &[u8] = unsafe {
		core::slice::from_raw_parts(
			&opt as *const _ as *const u8,
			size_of::<PodOption<PodU64>>(),
		)
	};
	assert_eq!(bytes[0], 1); // Some tag
	assert_eq!(bytes[1..9], 0xDEAD_BEEF_u64.to_le_bytes());
	let restored = unsafe { &*(bytes.as_ptr() as *const PodOption<PodU64>) };
	assert_eq!(restored.get(), Some(PodU64::from(0xDEAD_BEEF_u64)));
}
