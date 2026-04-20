use core::mem::size_of;

use super::*;

#[test]
fn pod_string_empty() {
	let s = PodString::<32>::default();
	assert!(s.is_empty());
	assert_eq!(s.len(), 0);
	assert_eq!(s.capacity(), 32);
	assert_eq!(s.as_bytes(), b"");
}

#[test]
fn pod_string_set_and_get() {
	let mut s = PodString::<32>::default();
	assert!(s.try_set("hello").is_ok());
	assert_eq!(s.len(), 5);
	assert_eq!(s.as_bytes(), b"hello");
	assert_eq!(s.try_as_str().unwrap(), "hello");
}

#[test]
fn pod_string_push_str() {
	let mut s = PodString::<32>::default();
	s.set("hello");
	assert!(s.try_push_str(" world").is_ok());
	assert_eq!(s.try_as_str().unwrap(), "hello world");
}

#[test]
fn pod_string_overflow_rejected() {
	let mut s = PodString::<4>::default();
	assert!(s.try_set("hello").is_err()); // 5 bytes > 4 capacity
	assert!(s.is_empty()); // unchanged
}

#[test]
fn pod_string_clear() {
	let mut s = PodString::<32>::default();
	s.set("test");
	assert!(!s.is_empty());
	s.clear();
	assert!(s.is_empty());
}

#[test]
fn pod_string_bytemuck_roundtrip() {
	let mut s = PodString::<32>::default();
	s.set("test");
	let bytes: &[u8] = unsafe {
		core::slice::from_raw_parts(&s as *const _ as *const u8, size_of::<PodString<32>>())
	};
	assert_eq!(bytes[0], 4); // len = 4
	assert_eq!(&bytes[1..5], b"test");
	let restored = unsafe { &*(bytes.as_ptr() as *const PodString<32>) };
	assert_eq!(restored.try_as_str().unwrap(), "test");
}
