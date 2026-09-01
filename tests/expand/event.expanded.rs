use pina::*;
#[repr(u8)]
#[non_exhaustive]
pub enum EventDisc {
	TransferEvent = 1,
	InitializeEvent = 2,
	EmptyEvent = 3,
	AuditEvent = 4,
}
#[automatically_derived]
impl ::core::fmt::Debug for EventDisc {
	#[inline]
	fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
		::core::fmt::Formatter::write_str(
			f,
			match self {
				EventDisc::TransferEvent => "TransferEvent",
				EventDisc::InitializeEvent => "InitializeEvent",
				EventDisc::EmptyEvent => "EmptyEvent",
				EventDisc::AuditEvent => "AuditEvent",
			},
		)
	}
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for EventDisc {}
#[automatically_derived]
impl ::core::clone::Clone for EventDisc {
	#[inline]
	fn clone(&self) -> EventDisc {
		*self
	}
}
#[automatically_derived]
impl ::core::marker::Copy for EventDisc {}
#[automatically_derived]
impl ::core::marker::StructuralPartialEq for EventDisc {}
#[automatically_derived]
impl ::core::cmp::PartialEq for EventDisc {
	#[inline]
	fn eq(&self, other: &EventDisc) -> bool {
		let __self_discr = ::core::intrinsics::discriminant_value(self);
		let __arg1_discr = ::core::intrinsics::discriminant_value(other);
		__self_discr == __arg1_discr
	}
}
#[automatically_derived]
impl ::core::cmp::Eq for EventDisc {
	#[inline]
	#[doc(hidden)]
	#[coverage(off)]
	fn assert_receiver_is_total_eq(&self) {}
}
const _: () = {
	if !(::core::mem::size_of::<u8>() <= ::pina::MAX_DISCRIMINATOR_SPACE) {
		{
			::core::panicking::panic_fmt(format_args!(
				"A discriminator with primitive `u8` (1 bytes) exceeds `MAX_DISCRIMINATOR_SPACE` \
				 and cannot be safely used for zero-copy layouts. Supported primitives: `u8`, \
				 `u16`, `u32`, `u64`.",
			));
		}
	}
};
impl ::core::convert::From<EventDisc> for u8 {
	#[inline]
	fn from(enum_value: EventDisc) -> Self {
		enum_value as Self
	}
}
impl ::core::convert::TryFrom<u8> for EventDisc {
	type Error = ::pina::ProgramError;

	#[inline]
	fn try_from(number: u8) -> ::core::result::Result<Self, ::pina::ProgramError> {
		#![allow(non_upper_case_globals)]
		const __TRANSFER_EVENT: u8 = 1;
		const __INITIALIZE_EVENT: u8 = 2;
		const __EMPTY_EVENT: u8 = 3;
		const __AUDIT_EVENT: u8 = 4;
		#[deny(unreachable_patterns)]
		match number {
			__TRANSFER_EVENT => ::core::result::Result::Ok(Self::TransferEvent),
			__INITIALIZE_EVENT => ::core::result::Result::Ok(Self::InitializeEvent),
			__EMPTY_EVENT => ::core::result::Result::Ok(Self::EmptyEvent),
			__AUDIT_EVENT => ::core::result::Result::Ok(Self::AuditEvent),
			#[allow(unreachable_patterns)]
			_ => ::core::result::Result::Err(::pina::PinaProgramError::InvalidDiscriminator.into()),
		}
	}
}
const _: () = if !(::core::mem::size_of::<EventDisc>() == ::core::mem::size_of::<u8>()) {
	{
		::core::panicking::panic_fmt(format_args!(
			"The size of the enum `EventDisc` must match the size of its primitive \
			 representation\n\t\t\t\t`u8`.",
		));
	}
};
impl ::pina::IntoDiscriminator for EventDisc {
	fn discriminator_from_bytes(
		bytes: &[u8],
	) -> ::core::result::Result<Self, ::pina::ProgramError> {
		<u8 as ::pina::IntoDiscriminator>::discriminator_from_bytes(bytes)
			.and_then(|primitive| Self::try_from(primitive))
	}

	fn write_discriminator(&self, bytes: &mut [u8]) {
		(*self as u8).write_discriminator(bytes);
	}

	fn matches_discriminator(&self, bytes: &[u8]) -> bool {
		(*self as u8).matches_discriminator(bytes)
	}
}
pub struct TransferEvent {
	discriminator: [u8; EventDisc::BYTES],
	pub from: [u8; 32],
	pub to: [u8; 32],
	pub amount: PodU64,
}
#[repr(C)]
pub struct TransferEventZc
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
	discriminator: [u8; EventDisc::BYTES],
	pub from: [u8; 32],
	pub to: [u8; 32],
	pub amount: <PodU64 as zeropod::ZcField>::Pod,
}
impl Copy for TransferEventZc
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
}
impl Clone for TransferEventZc
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
	fn clone(&self) -> Self {
		*self
	}
}
const _: () = if !(core::mem::align_of::<TransferEventZc>() == 1) {
	::core::panicking::panic("assertion failed: core::mem::align_of::<TransferEventZc>() == 1")
};
impl TransferEventZc {
	#[inline(always)]
	pub fn discriminator(&self) -> &[u8; EventDisc::BYTES] {
		&self.discriminator
	}

	#[inline(always)]
	pub fn from(&self) -> &[u8; 32] {
		&self.from
	}

	#[inline(always)]
	pub fn to(&self) -> &[u8; 32] {
		&self.to
	}

	#[inline(always)]
	pub fn amount(&self) -> &<PodU64 as zeropod::ZcField>::Pod {
		&self.amount
	}
}
impl zeropod::ZcValidate for TransferEventZc
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
	fn validate_ref(value: &Self) -> Result<(), zeropod::ZeroPodError> {
		<[u8; EventDisc::BYTES] as zeropod::ZcValidate>::validate_ref(&value.discriminator)?;
		<[u8; 32] as zeropod::ZcValidate>::validate_ref(&value.from)?;
		<[u8; 32] as zeropod::ZcValidate>::validate_ref(&value.to)?;
		<<PodU64 as zeropod::ZcField>::Pod as zeropod::ZcValidate>::validate_ref(&value.amount)?;
		Ok(())
	}
}
impl zeropod::ZeroPodSchema for TransferEvent
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
	const LAYOUT: zeropod::LayoutKind = zeropod::LayoutKind::Fixed;
}
impl zeropod::ZeroPodFixed for TransferEvent
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
	type Zc = TransferEventZc;

	const SIZE: usize = core::mem::size_of::<TransferEventZc>();

	fn from_bytes(data: &[u8]) -> Result<&Self::Zc, zeropod::ZeroPodError> {
		Self::validate(data)?;
		Ok(unsafe { &*(data.as_ptr() as *const Self::Zc) })
	}

	fn from_bytes_mut(data: &mut [u8]) -> Result<&mut Self::Zc, zeropod::ZeroPodError> {
		Self::validate(data)?;
		Ok(unsafe { &mut *(data.as_mut_ptr() as *mut Self::Zc) })
	}

	fn validate(data: &[u8]) -> Result<(), zeropod::ZeroPodError> {
		if data.len() < core::mem::size_of::<TransferEventZc>() {
			return Err(zeropod::ZeroPodError::BufferTooSmall);
		}
		let __zc = unsafe { &*(data.as_ptr() as *const Self::Zc) };
		<Self::Zc as zeropod::ZcValidate>::validate_ref(__zc)?;
		Ok(())
	}
}
impl zeropod::ZcField for TransferEvent
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
	type Pod = TransferEventZc;

	const POD_SIZE: usize = core::mem::size_of::<TransferEventZc>();
}
unsafe impl zeropod::ZcElem for TransferEventZc
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
}
const _: fn([u8; 32]) -> [::core::primitive::u8; 32] = |value| value;
const _: fn() = || {
	fn assert_mapping<T: pina::ZcField<Pod = [::core::primitive::u8; 32]>>() {}
	fn assert_storage<T: pina::ZcElem>() {}
	assert_mapping::<[u8; 32]>();
	assert_storage::<[::core::primitive::u8; 32]>();
};
const _: () = {
	if !(::core::mem::align_of::<[::core::primitive::u8; 32]>() == 1) {
		::core::panicking::panic(
			"assertion failed: ::core::mem::align_of::<[::core::primitive::u8; 32]>() == 1",
		)
	}
	if !(::core::mem::size_of::<[::core::primitive::u8; 32]>()
		== <[u8; 32] as pina::ZcField>::POD_SIZE)
	{
		::core::panicking::panic(
			"assertion failed: ::core::mem::size_of::<[::core::primitive::u8; 32]>() ==\n    \
			 <[u8; 32] as pina::ZcField>::POD_SIZE",
		)
	}
};
const _: fn([u8; 32]) -> [::core::primitive::u8; 32] = |value| value;
const _: fn() = || {
	fn assert_mapping<T: pina::ZcField<Pod = [::core::primitive::u8; 32]>>() {}
	fn assert_storage<T: pina::ZcElem>() {}
	assert_mapping::<[u8; 32]>();
	assert_storage::<[::core::primitive::u8; 32]>();
};
const _: () = {
	if !(::core::mem::align_of::<[::core::primitive::u8; 32]>() == 1) {
		::core::panicking::panic(
			"assertion failed: ::core::mem::align_of::<[::core::primitive::u8; 32]>() == 1",
		)
	}
	if !(::core::mem::size_of::<[::core::primitive::u8; 32]>()
		== <[u8; 32] as pina::ZcField>::POD_SIZE)
	{
		::core::panicking::panic(
			"assertion failed: ::core::mem::size_of::<[::core::primitive::u8; 32]>() ==\n    \
			 <[u8; 32] as pina::ZcField>::POD_SIZE",
		)
	}
};
const _: fn(PodU64) -> pina::PodU64 = |value| value;
const _: fn() = || {
	fn assert_mapping<T: pina::ZcField<Pod = pina::PodU64>>() {}
	fn assert_storage<T: pina::ZcElem>() {}
	assert_mapping::<PodU64>();
	assert_storage::<pina::PodU64>();
};
const _: () = {
	if !(::core::mem::align_of::<pina::PodU64>() == 1) {
		::core::panicking::panic("assertion failed: ::core::mem::align_of::<pina::PodU64>() == 1")
	}
	if !(::core::mem::size_of::<pina::PodU64>() == <PodU64 as pina::ZcField>::POD_SIZE) {
		::core::panicking::panic(
			"assertion failed: ::core::mem::size_of::<pina::PodU64>() == <PodU64 as \
			 pina::ZcField>::POD_SIZE",
		)
	}
};
const _: fn() = || {
	fn assert_storage<T: pina::ZcElem>() {}
	assert_storage::<TransferEventZc>();
};
const _: () = {
	if !(::core::mem::align_of::<TransferEventZc>() == 1) {
		::core::panicking::panic(
			"assertion failed: ::core::mem::align_of::<TransferEventZc>() == 1",
		)
	}
	if !(::core::mem::size_of::<TransferEventZc>()
		== EventDisc::BYTES
			+ ::core::mem::size_of::<[::core::primitive::u8; 32]>()
			+ ::core::mem::size_of::<[::core::primitive::u8; 32]>()
			+ ::core::mem::size_of::<pina::PodU64>())
	{
		::core::panicking::panic(
			"assertion failed: ::core::mem::size_of::<TransferEventZc>() ==\n    EventDisc::BYTES \
			 + ::core::mem::size_of::<[::core::primitive::u8; 32]>() +\n            \
			 ::core::mem::size_of::<[::core::primitive::u8; 32]>() +\n        \
			 ::core::mem::size_of::<pina::PodU64>()",
		)
	}
};
impl TransferEvent {
	/// The exact number of bytes required by the zeropod representation.
	pub const SIZE: usize = <Self as pina::ZeroPodFixed>::SIZE;

	/// Validate `data` and return zeropod's immutable zero-copy companion.
	pub fn try_from_bytes(
		data: &[u8],
	) -> Result<&<Self as pina::ZeroPodFixed>::Zc, pina::ProgramError> {
		if data.len() != Self::SIZE
			|| !<Self as pina::HasDiscriminator>::matches_discriminator(data)
		{
			return Err(pina::ProgramError::InvalidInstructionData);
		}
		<Self as pina::ZeroPodFixed>::from_bytes(data)
			.map_err(|_| pina::ProgramError::InvalidInstructionData)
	}

	/// Initialize caller-owned storage and return its mutable zero-copy view.
	///
	/// The complete slice is initialized before zeropod validates it. The
	/// returned borrow prevents the caller from observing or changing the raw
	/// bytes while the typed view is live.
	///
	/// Every accepted field has an audited all-zero representation. The macro
	/// rejects custom types and other layouts whose zero state cannot be
	/// established by Pina's closed schema grammar.
	///
	/// # Errors
	///
	/// Returns the generated invalid-data error when `data` has the wrong
	/// length or zeroed storage is not a valid zeropod representation.
	pub fn initialize(
		data: &mut [u8],
	) -> Result<&mut <Self as pina::ZeroPodFixed>::Zc, pina::ProgramError> {
		if data.len() != Self::SIZE {
			return Err(pina::ProgramError::InvalidInstructionData);
		}
		data.fill(0);
		<Self as pina::HasDiscriminator>::write_discriminator(data);
		<Self as pina::ZeroPodFixed>::from_bytes_mut(data)
			.map_err(|_| pina::ProgramError::InvalidInstructionData)
	}
}
impl pina::HasDiscriminator for TransferEvent {
	type Type = EventDisc;

	const VALUE: Self::Type = EventDisc::TransferEvent;
}
pub struct InitEvent {
	discriminator: [u8; EventDisc::BYTES],
	pub choice: u8,
}
#[repr(C)]
pub struct InitEventZc
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
	discriminator: [u8; EventDisc::BYTES],
	pub choice: u8,
}
impl Copy for InitEventZc
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
}
impl Clone for InitEventZc
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
	fn clone(&self) -> Self {
		*self
	}
}
const _: () = if !(core::mem::align_of::<InitEventZc>() == 1) {
	::core::panicking::panic("assertion failed: core::mem::align_of::<InitEventZc>() == 1")
};
impl InitEventZc {
	#[inline(always)]
	pub fn discriminator(&self) -> &[u8; EventDisc::BYTES] {
		&self.discriminator
	}

	#[inline(always)]
	pub fn choice(&self) -> u8 {
		self.choice
	}
}
impl zeropod::ZcValidate for InitEventZc
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
	fn validate_ref(value: &Self) -> Result<(), zeropod::ZeroPodError> {
		<[u8; EventDisc::BYTES] as zeropod::ZcValidate>::validate_ref(&value.discriminator)?;
		<u8 as zeropod::ZcValidate>::validate_ref(&value.choice)?;
		Ok(())
	}
}
impl zeropod::ZeroPodSchema for InitEvent
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
	const LAYOUT: zeropod::LayoutKind = zeropod::LayoutKind::Fixed;
}
impl zeropod::ZeroPodFixed for InitEvent
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
	type Zc = InitEventZc;

	const SIZE: usize = core::mem::size_of::<InitEventZc>();

	fn from_bytes(data: &[u8]) -> Result<&Self::Zc, zeropod::ZeroPodError> {
		Self::validate(data)?;
		Ok(unsafe { &*(data.as_ptr() as *const Self::Zc) })
	}

	fn from_bytes_mut(data: &mut [u8]) -> Result<&mut Self::Zc, zeropod::ZeroPodError> {
		Self::validate(data)?;
		Ok(unsafe { &mut *(data.as_mut_ptr() as *mut Self::Zc) })
	}

	fn validate(data: &[u8]) -> Result<(), zeropod::ZeroPodError> {
		if data.len() < core::mem::size_of::<InitEventZc>() {
			return Err(zeropod::ZeroPodError::BufferTooSmall);
		}
		let __zc = unsafe { &*(data.as_ptr() as *const Self::Zc) };
		<Self::Zc as zeropod::ZcValidate>::validate_ref(__zc)?;
		Ok(())
	}
}
impl zeropod::ZcField for InitEvent
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
	type Pod = InitEventZc;

	const POD_SIZE: usize = core::mem::size_of::<InitEventZc>();
}
unsafe impl zeropod::ZcElem for InitEventZc
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
}
const _: fn(u8) -> ::core::primitive::u8 = |value| value;
const _: fn() = || {
	fn assert_mapping<T: pina::ZcField<Pod = ::core::primitive::u8>>() {}
	fn assert_storage<T: pina::ZcElem>() {}
	assert_mapping::<u8>();
	assert_storage::<::core::primitive::u8>();
};
const _: () = {
	if !(::core::mem::align_of::<::core::primitive::u8>() == 1) {
		::core::panicking::panic(
			"assertion failed: ::core::mem::align_of::<::core::primitive::u8>() == 1",
		)
	}
	if !(::core::mem::size_of::<::core::primitive::u8>() == <u8 as pina::ZcField>::POD_SIZE) {
		::core::panicking::panic(
			"assertion failed: ::core::mem::size_of::<::core::primitive::u8>() ==\n    <u8 as \
			 pina::ZcField>::POD_SIZE",
		)
	}
};
const _: fn() = || {
	fn assert_storage<T: pina::ZcElem>() {}
	assert_storage::<InitEventZc>();
};
const _: () = {
	if !(::core::mem::align_of::<InitEventZc>() == 1) {
		::core::panicking::panic("assertion failed: ::core::mem::align_of::<InitEventZc>() == 1")
	}
	if !(::core::mem::size_of::<InitEventZc>()
		== EventDisc::BYTES + ::core::mem::size_of::<::core::primitive::u8>())
	{
		::core::panicking::panic(
			"assertion failed: ::core::mem::size_of::<InitEventZc>() ==\n    EventDisc::BYTES + \
			 ::core::mem::size_of::<::core::primitive::u8>()",
		)
	}
};
impl InitEvent {
	/// The exact number of bytes required by the zeropod representation.
	pub const SIZE: usize = <Self as pina::ZeroPodFixed>::SIZE;

	/// Validate `data` and return zeropod's immutable zero-copy companion.
	pub fn try_from_bytes(
		data: &[u8],
	) -> Result<&<Self as pina::ZeroPodFixed>::Zc, pina::ProgramError> {
		if data.len() != Self::SIZE
			|| !<Self as pina::HasDiscriminator>::matches_discriminator(data)
		{
			return Err(pina::ProgramError::InvalidInstructionData);
		}
		<Self as pina::ZeroPodFixed>::from_bytes(data)
			.map_err(|_| pina::ProgramError::InvalidInstructionData)
	}

	/// Initialize caller-owned storage and return its mutable zero-copy view.
	///
	/// The complete slice is initialized before zeropod validates it. The
	/// returned borrow prevents the caller from observing or changing the raw
	/// bytes while the typed view is live.
	///
	/// Every accepted field has an audited all-zero representation. The macro
	/// rejects custom types and other layouts whose zero state cannot be
	/// established by Pina's closed schema grammar.
	///
	/// # Errors
	///
	/// Returns the generated invalid-data error when `data` has the wrong
	/// length or zeroed storage is not a valid zeropod representation.
	pub fn initialize(
		data: &mut [u8],
	) -> Result<&mut <Self as pina::ZeroPodFixed>::Zc, pina::ProgramError> {
		if data.len() != Self::SIZE {
			return Err(pina::ProgramError::InvalidInstructionData);
		}
		data.fill(0);
		<Self as pina::HasDiscriminator>::write_discriminator(data);
		<Self as pina::ZeroPodFixed>::from_bytes_mut(data)
			.map_err(|_| pina::ProgramError::InvalidInstructionData)
	}
}
impl pina::HasDiscriminator for InitEvent {
	type Type = EventDisc;

	const VALUE: Self::Type = EventDisc::InitializeEvent;
}
pub struct EmptyEvent {
	discriminator: [u8; EventDisc::BYTES],
}
#[repr(C)]
pub struct EmptyEventZc
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
{
	discriminator: [u8; EventDisc::BYTES],
}
impl Copy for EmptyEventZc where [u8; EventDisc::BYTES]: zeropod::ZcValidate {}
impl Clone for EmptyEventZc
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
{
	fn clone(&self) -> Self {
		*self
	}
}
const _: () = if !(core::mem::align_of::<EmptyEventZc>() == 1) {
	::core::panicking::panic("assertion failed: core::mem::align_of::<EmptyEventZc>() == 1")
};
impl EmptyEventZc {
	#[inline(always)]
	pub fn discriminator(&self) -> &[u8; EventDisc::BYTES] {
		&self.discriminator
	}
}
impl zeropod::ZcValidate for EmptyEventZc
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
{
	fn validate_ref(value: &Self) -> Result<(), zeropod::ZeroPodError> {
		<[u8; EventDisc::BYTES] as zeropod::ZcValidate>::validate_ref(&value.discriminator)?;
		Ok(())
	}
}
impl zeropod::ZeroPodSchema for EmptyEvent
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
{
	const LAYOUT: zeropod::LayoutKind = zeropod::LayoutKind::Fixed;
}
impl zeropod::ZeroPodFixed for EmptyEvent
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
{
	type Zc = EmptyEventZc;

	const SIZE: usize = core::mem::size_of::<EmptyEventZc>();

	fn from_bytes(data: &[u8]) -> Result<&Self::Zc, zeropod::ZeroPodError> {
		Self::validate(data)?;
		Ok(unsafe { &*(data.as_ptr() as *const Self::Zc) })
	}

	fn from_bytes_mut(data: &mut [u8]) -> Result<&mut Self::Zc, zeropod::ZeroPodError> {
		Self::validate(data)?;
		Ok(unsafe { &mut *(data.as_mut_ptr() as *mut Self::Zc) })
	}

	fn validate(data: &[u8]) -> Result<(), zeropod::ZeroPodError> {
		if data.len() < core::mem::size_of::<EmptyEventZc>() {
			return Err(zeropod::ZeroPodError::BufferTooSmall);
		}
		let __zc = unsafe { &*(data.as_ptr() as *const Self::Zc) };
		<Self::Zc as zeropod::ZcValidate>::validate_ref(__zc)?;
		Ok(())
	}
}
impl zeropod::ZcField for EmptyEvent
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
{
	type Pod = EmptyEventZc;

	const POD_SIZE: usize = core::mem::size_of::<EmptyEventZc>();
}
unsafe impl zeropod::ZcElem for EmptyEventZc where [u8; EventDisc::BYTES]: zeropod::ZcValidate {}
const _: fn() = || {
	fn assert_storage<T: pina::ZcElem>() {}
	assert_storage::<EmptyEventZc>();
};
const _: () = {
	if !(::core::mem::align_of::<EmptyEventZc>() == 1) {
		::core::panicking::panic("assertion failed: ::core::mem::align_of::<EmptyEventZc>() == 1")
	}
	if !(::core::mem::size_of::<EmptyEventZc>() == EventDisc::BYTES) {
		::core::panicking::panic(
			"assertion failed: ::core::mem::size_of::<EmptyEventZc>() == EventDisc::BYTES",
		)
	}
};
impl EmptyEvent {
	/// The exact number of bytes required by the zeropod representation.
	pub const SIZE: usize = <Self as pina::ZeroPodFixed>::SIZE;

	/// Validate `data` and return zeropod's immutable zero-copy companion.
	pub fn try_from_bytes(
		data: &[u8],
	) -> Result<&<Self as pina::ZeroPodFixed>::Zc, pina::ProgramError> {
		if data.len() != Self::SIZE
			|| !<Self as pina::HasDiscriminator>::matches_discriminator(data)
		{
			return Err(pina::ProgramError::InvalidInstructionData);
		}
		<Self as pina::ZeroPodFixed>::from_bytes(data)
			.map_err(|_| pina::ProgramError::InvalidInstructionData)
	}

	/// Initialize caller-owned storage and return its mutable zero-copy view.
	///
	/// The complete slice is initialized before zeropod validates it. The
	/// returned borrow prevents the caller from observing or changing the raw
	/// bytes while the typed view is live.
	///
	/// Every accepted field has an audited all-zero representation. The macro
	/// rejects custom types and other layouts whose zero state cannot be
	/// established by Pina's closed schema grammar.
	///
	/// # Errors
	///
	/// Returns the generated invalid-data error when `data` has the wrong
	/// length or zeroed storage is not a valid zeropod representation.
	pub fn initialize(
		data: &mut [u8],
	) -> Result<&mut <Self as pina::ZeroPodFixed>::Zc, pina::ProgramError> {
		if data.len() != Self::SIZE {
			return Err(pina::ProgramError::InvalidInstructionData);
		}
		data.fill(0);
		<Self as pina::HasDiscriminator>::write_discriminator(data);
		<Self as pina::ZeroPodFixed>::from_bytes_mut(data)
			.map_err(|_| pina::ProgramError::InvalidInstructionData)
	}
}
impl pina::HasDiscriminator for EmptyEvent {
	type Type = EventDisc;

	const VALUE: Self::Type = EventDisc::EmptyEvent;
}
pub struct AuditEvent {
	discriminator: [u8; EventDisc::BYTES],
	pub action: u8,
	pub timestamp: PodU64,
}
#[automatically_derived]
impl ::core::fmt::Debug for AuditEvent {
	#[inline]
	fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
		::core::fmt::Formatter::debug_struct_field3_finish(
			f,
			"AuditEvent",
			"discriminator",
			&self.discriminator,
			"action",
			&self.action,
			"timestamp",
			&&self.timestamp,
		)
	}
}
#[repr(C)]
pub struct AuditEventZc
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
	discriminator: [u8; EventDisc::BYTES],
	pub action: u8,
	pub timestamp: <PodU64 as zeropod::ZcField>::Pod,
}
impl Copy for AuditEventZc
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
}
impl Clone for AuditEventZc
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
	fn clone(&self) -> Self {
		*self
	}
}
const _: () = if !(core::mem::align_of::<AuditEventZc>() == 1) {
	::core::panicking::panic("assertion failed: core::mem::align_of::<AuditEventZc>() == 1")
};
impl AuditEventZc {
	#[inline(always)]
	pub fn discriminator(&self) -> &[u8; EventDisc::BYTES] {
		&self.discriminator
	}

	#[inline(always)]
	pub fn action(&self) -> u8 {
		self.action
	}

	#[inline(always)]
	pub fn timestamp(&self) -> &<PodU64 as zeropod::ZcField>::Pod {
		&self.timestamp
	}
}
impl zeropod::ZcValidate for AuditEventZc
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
	fn validate_ref(value: &Self) -> Result<(), zeropod::ZeroPodError> {
		<[u8; EventDisc::BYTES] as zeropod::ZcValidate>::validate_ref(&value.discriminator)?;
		<u8 as zeropod::ZcValidate>::validate_ref(&value.action)?;
		<<PodU64 as zeropod::ZcField>::Pod as zeropod::ZcValidate>::validate_ref(&value.timestamp)?;
		Ok(())
	}
}
impl zeropod::ZeroPodSchema for AuditEvent
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
	const LAYOUT: zeropod::LayoutKind = zeropod::LayoutKind::Fixed;
}
impl zeropod::ZeroPodFixed for AuditEvent
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
	type Zc = AuditEventZc;

	const SIZE: usize = core::mem::size_of::<AuditEventZc>();

	fn from_bytes(data: &[u8]) -> Result<&Self::Zc, zeropod::ZeroPodError> {
		Self::validate(data)?;
		Ok(unsafe { &*(data.as_ptr() as *const Self::Zc) })
	}

	fn from_bytes_mut(data: &mut [u8]) -> Result<&mut Self::Zc, zeropod::ZeroPodError> {
		Self::validate(data)?;
		Ok(unsafe { &mut *(data.as_mut_ptr() as *mut Self::Zc) })
	}

	fn validate(data: &[u8]) -> Result<(), zeropod::ZeroPodError> {
		if data.len() < core::mem::size_of::<AuditEventZc>() {
			return Err(zeropod::ZeroPodError::BufferTooSmall);
		}
		let __zc = unsafe { &*(data.as_ptr() as *const Self::Zc) };
		<Self::Zc as zeropod::ZcValidate>::validate_ref(__zc)?;
		Ok(())
	}
}
impl zeropod::ZcField for AuditEvent
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
	type Pod = AuditEventZc;

	const POD_SIZE: usize = core::mem::size_of::<AuditEventZc>();
}
unsafe impl zeropod::ZcElem for AuditEventZc
where
	[u8; EventDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
}
const _: fn(u8) -> ::core::primitive::u8 = |value| value;
const _: fn() = || {
	fn assert_mapping<T: pina::ZcField<Pod = ::core::primitive::u8>>() {}
	fn assert_storage<T: pina::ZcElem>() {}
	assert_mapping::<u8>();
	assert_storage::<::core::primitive::u8>();
};
const _: () = {
	if !(::core::mem::align_of::<::core::primitive::u8>() == 1) {
		::core::panicking::panic(
			"assertion failed: ::core::mem::align_of::<::core::primitive::u8>() == 1",
		)
	}
	if !(::core::mem::size_of::<::core::primitive::u8>() == <u8 as pina::ZcField>::POD_SIZE) {
		::core::panicking::panic(
			"assertion failed: ::core::mem::size_of::<::core::primitive::u8>() ==\n    <u8 as \
			 pina::ZcField>::POD_SIZE",
		)
	}
};
const _: fn(PodU64) -> pina::PodU64 = |value| value;
const _: fn() = || {
	fn assert_mapping<T: pina::ZcField<Pod = pina::PodU64>>() {}
	fn assert_storage<T: pina::ZcElem>() {}
	assert_mapping::<PodU64>();
	assert_storage::<pina::PodU64>();
};
const _: () = {
	if !(::core::mem::align_of::<pina::PodU64>() == 1) {
		::core::panicking::panic("assertion failed: ::core::mem::align_of::<pina::PodU64>() == 1")
	}
	if !(::core::mem::size_of::<pina::PodU64>() == <PodU64 as pina::ZcField>::POD_SIZE) {
		::core::panicking::panic(
			"assertion failed: ::core::mem::size_of::<pina::PodU64>() == <PodU64 as \
			 pina::ZcField>::POD_SIZE",
		)
	}
};
const _: fn() = || {
	fn assert_storage<T: pina::ZcElem>() {}
	assert_storage::<AuditEventZc>();
};
const _: () = {
	if !(::core::mem::align_of::<AuditEventZc>() == 1) {
		::core::panicking::panic("assertion failed: ::core::mem::align_of::<AuditEventZc>() == 1")
	}
	if !(::core::mem::size_of::<AuditEventZc>()
		== EventDisc::BYTES
			+ ::core::mem::size_of::<::core::primitive::u8>()
			+ ::core::mem::size_of::<pina::PodU64>())
	{
		::core::panicking::panic(
			"assertion failed: ::core::mem::size_of::<AuditEventZc>() ==\n    EventDisc::BYTES + \
			 ::core::mem::size_of::<::core::primitive::u8>() +\n        \
			 ::core::mem::size_of::<pina::PodU64>()",
		)
	}
};
impl AuditEvent {
	/// The exact number of bytes required by the zeropod representation.
	pub const SIZE: usize = <Self as pina::ZeroPodFixed>::SIZE;

	/// Validate `data` and return zeropod's immutable zero-copy companion.
	pub fn try_from_bytes(
		data: &[u8],
	) -> Result<&<Self as pina::ZeroPodFixed>::Zc, pina::ProgramError> {
		if data.len() != Self::SIZE
			|| !<Self as pina::HasDiscriminator>::matches_discriminator(data)
		{
			return Err(pina::ProgramError::InvalidInstructionData);
		}
		<Self as pina::ZeroPodFixed>::from_bytes(data)
			.map_err(|_| pina::ProgramError::InvalidInstructionData)
	}

	/// Initialize caller-owned storage and return its mutable zero-copy view.
	///
	/// The complete slice is initialized before zeropod validates it. The
	/// returned borrow prevents the caller from observing or changing the raw
	/// bytes while the typed view is live.
	///
	/// Every accepted field has an audited all-zero representation. The macro
	/// rejects custom types and other layouts whose zero state cannot be
	/// established by Pina's closed schema grammar.
	///
	/// # Errors
	///
	/// Returns the generated invalid-data error when `data` has the wrong
	/// length or zeroed storage is not a valid zeropod representation.
	pub fn initialize(
		data: &mut [u8],
	) -> Result<&mut <Self as pina::ZeroPodFixed>::Zc, pina::ProgramError> {
		if data.len() != Self::SIZE {
			return Err(pina::ProgramError::InvalidInstructionData);
		}
		data.fill(0);
		<Self as pina::HasDiscriminator>::write_discriminator(data);
		<Self as pina::ZeroPodFixed>::from_bytes_mut(data)
			.map_err(|_| pina::ProgramError::InvalidInstructionData)
	}
}
impl pina::HasDiscriminator for AuditEvent {
	type Type = EventDisc;

	const VALUE: Self::Type = EventDisc::AuditEvent;
}
