//! Tests for `#[account(compact)]` — fixed header + variable-length tail.

use pina::*;

#[discriminator(crate = ::pina, primitive = u8, final)]
pub enum ProfileAccountType {
	Profile = 1,
}

#[account(compact, crate = ::pina, discriminator = ProfileAccountType)]
pub struct Profile {
	/// The PDA bump seed.
	pub bump: u8,
	/// The profile display name.
	pub name: PodString<32>,
	/// A longer free-form bio.
	pub bio: PodString<128>,
	/// Up to 8 tags.
	pub tags: PodVec<PodU64, 8>,
	/// An optional nickname.
	pub nickname: PodOption<PodString<16>>,
}

// ---------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------

#[test]
fn compact_builder_and_serialization() {
	let profile = Profile::builder()
		.bump(255)
		.name(pod_string("bob"))
		.bio(pod_string("hello world"))
		.tags(pod_vec(&[
			PodU64::from(1),
			PodU64::from(2),
			PodU64::from(3),
		]))
		.nickname(PodOption::some(pod_string("bobby")))
		.build();

	// compact_size = header (disc + bump) + name(1+3) + bio(1+11) + tags(2+24) + nick(1+1+5)
	// header = 1 (disc) + 1 (bump) = 2
	// name = 1 + 3 = 4
	// bio = 1 + 11 = 12
	// tags = 2 + 3*8 = 26
	// nick = 1 + 1 + 5 = 7
	assert_eq!(profile.compact_size(), 2 + 4 + 12 + 26 + 7);

	let bytes = profile.to_compact_bytes();
	assert_eq!(bytes.len(), profile.compact_size());

	// Discriminator first.
	assert_eq!(bytes[0], 1);
	// Bump.
	assert_eq!(bytes[1], 255);
	// name: len=3, "bob"
	assert_eq!(bytes[2], 3);
	assert_eq!(&bytes[3..6], b"bob");
	// bio: len=11, "hello world"
	assert_eq!(bytes[6], 11);
	assert_eq!(&bytes[7..18], b"hello world");
	// tags: count=3, then 3 x 8-byte LE u64s
	assert_eq!(u16::from_le_bytes([bytes[18], bytes[19]]), 3);
	assert_eq!(u64::from_le_bytes(bytes[20..28].try_into().unwrap()), 1);
	assert_eq!(u64::from_le_bytes(bytes[28..36].try_into().unwrap()), 2);
	assert_eq!(u64::from_le_bytes(bytes[36..44].try_into().unwrap()), 3);
	// nickname: tag=1, len=5, "bobby"
	assert_eq!(bytes[44], 1);
	assert_eq!(bytes[45], 5);
	assert_eq!(&bytes[46..51], b"bobby");
}

#[test]
fn compact_empty_tail_serialization() {
	let profile = Profile::builder()
		.bump(1)
		.name(pod_string(""))
		.bio(pod_string(""))
		.tags(pod_vec(&[]))
		.nickname(PodOption::none())
		.build();

	let bytes = profile.to_compact_bytes();
	// header(2) + name(1+0) + bio(1+0) + tags(2+0) + nick(1)
	assert_eq!(bytes.len(), 2 + 1 + 1 + 2 + 1);
	assert_eq!(bytes[2], 0); // empty name
	assert_eq!(bytes[3], 0); // empty bio
	assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), 0); // no tags
	assert_eq!(bytes[6], 0); // nickname None
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

#[test]
fn compact_validate_accepts_valid_data() {
	let profile = Profile::builder()
		.bump(7)
		.name(pod_string("alice"))
		.bio(pod_string("bio"))
		.tags(pod_vec(&[PodU64::from(9)]))
		.nickname(PodOption::none())
		.build();
	let bytes = profile.to_compact_bytes();

	assert!(<Profile as CompactAccount>::validate(&bytes).is_ok());
}

#[test]
fn compact_validate_rejects_wrong_discriminator() {
	let profile = Profile::builder()
		.bump(7)
		.name(pod_string("alice"))
		.bio(pod_string("bio"))
		.tags(pod_vec(&[]))
		.nickname(PodOption::none())
		.build();
	let mut bytes = profile.to_compact_bytes();
	bytes[0] = 99; // wrong discriminator
	assert!(<Profile as CompactAccount>::validate(&bytes).is_err());
}

#[test]
fn compact_validate_rejects_truncated_data() {
	let profile = Profile::builder()
		.bump(7)
		.name(pod_string("alice"))
		.bio(pod_string("bio"))
		.tags(pod_vec(&[]))
		.nickname(PodOption::none())
		.build();
	let bytes = profile.to_compact_bytes();

	// Truncate in the middle of the name payload.
	assert!(<Profile as CompactAccount>::validate(&bytes[..5]).is_err());
	// Truncate the header.
	assert!(<Profile as CompactAccount>::validate(&bytes[..1]).is_err());
}

#[test]
fn compact_validate_rejects_invalid_utf8() {
	let profile = Profile::builder()
		.bump(7)
		.name(pod_string("alice"))
		.bio(pod_string("bio"))
		.tags(pod_vec(&[]))
		.nickname(PodOption::none())
		.build();
	let mut bytes = profile.to_compact_bytes();
	// Corrupt the name payload with invalid UTF-8 (0xFF).
	bytes[3] = 0xFF;
	assert!(<Profile as CompactAccount>::validate(&bytes).is_err());
}

#[test]
fn compact_validate_rejects_oversized_segment() {
	let profile = Profile::builder()
		.bump(7)
		.name(pod_string("alice"))
		.bio(pod_string("bio"))
		.tags(pod_vec(&[]))
		.nickname(PodOption::none())
		.build();
	let mut bytes = profile.to_compact_bytes();
	// Claim the name is 200 bytes (over the 32 capacity).
	bytes[2] = 200;
	assert!(<Profile as CompactAccount>::validate(&bytes).is_err());
}

#[test]
fn compact_validate_rejects_invalid_option_tag() {
	let profile = Profile::builder()
		.bump(7)
		.name(pod_string("alice"))
		.bio(pod_string("bio"))
		.tags(pod_vec(&[]))
		.nickname(PodOption::none())
		.build();
	let mut bytes = profile.to_compact_bytes();
	// Corrupt the nickname tag to 7 (invalid).
	let nick_offset = bytes.len() - 1;
	bytes[nick_offset] = 7;
	assert!(<Profile as CompactAccount>::validate(&bytes).is_err());
}

// ---------------------------------------------------------------------------
// Views
// ---------------------------------------------------------------------------

#[test]
fn compact_ref_accessors() {
	let profile = Profile::builder()
		.bump(42)
		.name(pod_string("carol"))
		.bio(pod_string("a longer bio here"))
		.tags(pod_vec(&[PodU64::from(5), PodU64::from(6)]))
		.nickname(PodOption::some(pod_string("caz")))
		.build();
	let bytes = profile.to_compact_bytes();

	let view = ProfileRef::new(&bytes).unwrap();
	assert_eq!(view.header().bump, 42);
	assert_eq!(view.name().unwrap(), "carol");
	assert_eq!(view.bio().unwrap(), "a longer bio here");
	assert_eq!(view.tags().unwrap(), &[PodU64::from(5), PodU64::from(6)]);
	assert_eq!(view.nickname().unwrap(), Some("caz"));
}

#[test]
fn compact_ref_rejects_invalid_data() {
	let profile = Profile::builder()
		.bump(42)
		.name(pod_string("carol"))
		.bio(pod_string("bio"))
		.tags(pod_vec(&[]))
		.nickname(PodOption::none())
		.build();
	let mut bytes = profile.to_compact_bytes();
	bytes[0] = 0; // wrong discriminator
	assert!(ProfileRef::new(&bytes).is_err());
}

#[test]
fn compact_ref_mut_header_mutation() {
	let profile = Profile::builder()
		.bump(1)
		.name(pod_string("dave"))
		.bio(pod_string("bio"))
		.tags(pod_vec(&[]))
		.nickname(PodOption::none())
		.build();
	let mut bytes = profile.to_compact_bytes();

	{
		let mut view = ProfileRefMut::new(&mut bytes).unwrap();
		view.header_mut().bump = 99;
	}

	// Re-validate and read back.
	let view = ProfileRef::new(&bytes).unwrap();
	assert_eq!(view.header().bump, 99);
	assert_eq!(view.name().unwrap(), "dave");
}

#[test]
fn compact_header_and_header_mut() {
	let profile = Profile::builder()
		.bump(3)
		.name(pod_string("eve"))
		.bio(pod_string("bio"))
		.tags(pod_vec(&[]))
		.nickname(PodOption::none())
		.build();
	let bytes = profile.to_compact_bytes();

	let header = <Profile as CompactAccount>::header(&bytes).unwrap();
	assert_eq!(header.bump, 3);

	let mut bytes = bytes;
	{
		let header = <Profile as CompactAccount>::header_mut(&mut bytes).unwrap();
		header.bump = 8;
	}
	let header = <Profile as CompactAccount>::header(&bytes).unwrap();
	assert_eq!(header.bump, 8);
}

// ---------------------------------------------------------------------------
// Suffix-only rule (compile-time)
// ---------------------------------------------------------------------------

// An inline field after a tail field must fail to compile. We can't test a
// compile error in a unit test, but the macro enforces it via a spanned
// compile_error!. The following struct is intentionally NOT compiled.

// ---------------------------------------------------------------------------
// PodString / PodVec construction helpers
// ---------------------------------------------------------------------------

fn pod_string<const N: usize>(s: &str) -> PodString<N> {
	PodString::from(s)
}

fn pod_vec<const N: usize>(items: &[PodU64]) -> PodVec<PodU64, N> {
	PodVec::from(items)
}
