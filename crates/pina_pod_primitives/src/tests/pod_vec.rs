use core::mem::size_of;

use super::*;

#[test]
fn pod_vec_empty() {
	let v = PodVec::<PodU64, 10>::default();
	assert!(v.is_empty());
	assert_eq!(v.len(), 0);
	assert_eq!(v.capacity(), 10);
	assert_eq!(v.as_slice(), &[] as &[PodU64]);
}

#[test]
fn pod_vec_push_and_get() {
	let mut v = PodVec::<PodU64, 10>::default();
	assert!(v.try_push(PodU64::from(1u64)).is_ok());
	assert!(v.try_push(PodU64::from(2u64)).is_ok());
	assert_eq!(v.len(), 2);
	assert_eq!(v.get(0), Some(&PodU64::from(1u64)));
	assert_eq!(v.get(1), Some(&PodU64::from(2u64)));
	assert_eq!(v.get(2), None);
}

#[test]
fn pod_vec_as_slice() {
	let mut v = PodVec::<PodU64, 10>::default();
	v.push(PodU64::from(100u64));
	v.push(PodU64::from(200u64));
	let slice: Vec<u64> = v.as_slice().iter().map(|x| x.get()).collect();
	assert_eq!(slice, vec![100, 200]);
}

#[test]
fn pod_vec_overflow_rejected() {
	let mut v = PodVec::<PodU64, 2>::default();
	assert!(v.try_push(PodU64::from(1u64)).is_ok());
	assert!(v.try_push(PodU64::from(2u64)).is_ok());
	assert!(v.try_push(PodU64::from(3u64)).is_err()); // at capacity
	assert_eq!(v.len(), 2);
}

#[test]
fn pod_vec_pop() {
	let mut v = PodVec::<PodU64, 10>::default();
	v.push(PodU64::from(42u64));
	v.push(PodU64::from(99u64));
	assert_eq!(v.pop(), Some(PodU64::from(99u64)));
	assert_eq!(v.pop(), Some(PodU64::from(42u64)));
	assert_eq!(v.pop(), None);
}

#[test]
fn pod_vec_clear() {
	let mut v = PodVec::<PodU64, 10>::default();
	v.push(PodU64::from(1u64));
	v.push(PodU64::from(2u64));
	v.clear();
	assert!(v.is_empty());
	assert_eq!(v.len(), 0);
}

#[test]
fn pod_vec_bytemuck_roundtrip() {
	let mut v = PodVec::<PodU64, 10>::default();
	v.push(PodU64::from(100u64));
	v.push(PodU64::from(200u64));
	let bytes: &[u8] = unsafe {
		core::slice::from_raw_parts(&v as *const _ as *const u8, size_of::<PodVec<PodU64, 10>>())
	};
	assert_eq!(bytes[0..2], [2, 0]); // len = 2 in LE
	let restored = unsafe { &*(bytes.as_ptr() as *const PodVec<PodU64, 10>) };
	assert_eq!(restored.len(), 2);
	assert_eq!(restored.get(0), Some(&PodU64::from(100u64)));
	assert_eq!(restored.get(1), Some(&PodU64::from(200u64)));
}
