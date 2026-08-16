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

// ---------------------------------------------------------------------------
// UTF-8 soundness: PodString loaded from untrusted bytes
// ---------------------------------------------------------------------------

/// A `PodString` loaded from arbitrary account data may contain invalid
/// UTF-8. `try_as_str()` must reject it rather than producing a `&str`.
#[test]
fn pod_string_invalid_utf8_rejected_by_try_as_str() {
	// Length prefix 2, data bytes [0xff, 0xfe] — invalid UTF-8.
	let bytes = [2u8, 0xff, 0xfe];
	let pod = try_from_bytes::<PodString<2>>(&bytes)
		.unwrap_or_else(|e| panic!("try_from_bytes failed for {bytes:?}: {e}"));
	assert_eq!(pod.len(), 2);
	assert!(matches!(
		pod.try_as_str(),
		Err(PodCollectionError::InvalidUtf8)
	));
}

/// A truncated multi-byte sequence is also invalid UTF-8.
#[test]
fn pod_string_incomplete_utf8_rejected_by_try_as_str() {
	// Length prefix 1, data byte 0xc3 — a lead byte with no continuation.
	let bytes = [1u8, 0xc3];
	let pod = try_from_bytes::<PodString<1>>(&bytes)
		.unwrap_or_else(|e| panic!("try_from_bytes failed for {bytes:?}: {e}"));
	assert!(matches!(
		pod.try_as_str(),
		Err(PodCollectionError::InvalidUtf8)
	));
}

/// Safe traits must never panic or produce a `&str` from invalid UTF-8.
#[test]
fn pod_string_invalid_utf8_safe_traits_do_not_panic() {
	let bytes = [2u8, 0xff, 0xfe];
	let pod = try_from_bytes::<PodString<2>>(&bytes)
		.unwrap_or_else(|e| panic!("try_from_bytes failed for {bytes:?}: {e}"));

	// Debug and Display fall back to a placeholder.
	assert_eq!(std::format!("{pod:?}"), "PodString { len: 2 }");
	assert_eq!(std::format!("{pod}"), "<invalid utf8>");

	// Byte access and byte-based comparison remain total and safe.
	assert_eq!(pod.as_bytes(), &[0xff, 0xfe]);
	assert_eq!(<PodString<2> as AsRef<[u8]>>::as_ref(&pod), &[0xff, 0xfe]);
	assert_eq!(pod, pod);
	assert_ne!(pod, "valid");
}

/// Valid multi-byte UTF-8 survives a Pod round-trip.
#[test]
fn pod_string_valid_utf8_roundtrip_via_pod() {
	let mut s = PodString::<32>::default();
	s.set("héllo wörld");
	let bytes = bytemuck::bytes_of(&s);
	let restored = try_from_bytes::<PodString<32>>(bytes)
		.unwrap_or_else(|e| panic!("try_from_bytes failed: {e}"));
	assert_eq!(
		restored
			.try_as_str()
			.unwrap_or_else(|e| panic!("invalid UTF-8: {e}")),
		"héllo wörld"
	);
}

/// `as_str_unchecked` remains available as an explicit unsafe escape hatch.
#[test]
fn pod_string_as_str_unchecked_valid() {
	let mut s = PodString::<32>::default();
	s.set("hello");
	// SAFETY: "hello" is valid UTF-8.
	let s = unsafe { s.as_str_unchecked() };
	assert_eq!(s, "hello");
}
