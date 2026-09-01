use pina::*;
#[repr(u8)]
#[non_exhaustive]
pub enum AccountDisc {
	ConfigState = 1,
	GameState = 2,
	DataAccount = 3,
	BalanceAccount = 4,
	Custom = 5,
	LargeState = 6,
}
#[automatically_derived]
impl ::core::fmt::Debug for AccountDisc {
	#[inline]
	fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
		::core::fmt::Formatter::write_str(
			f,
			match self {
				AccountDisc::ConfigState => "ConfigState",
				AccountDisc::GameState => "GameState",
				AccountDisc::DataAccount => "DataAccount",
				AccountDisc::BalanceAccount => "BalanceAccount",
				AccountDisc::Custom => "Custom",
				AccountDisc::LargeState => "LargeState",
			},
		)
	}
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for AccountDisc {}
#[automatically_derived]
impl ::core::clone::Clone for AccountDisc {
	#[inline]
	fn clone(&self) -> AccountDisc {
		*self
	}
}
#[automatically_derived]
impl ::core::marker::Copy for AccountDisc {}
#[automatically_derived]
impl ::core::marker::StructuralPartialEq for AccountDisc {}
#[automatically_derived]
impl ::core::cmp::PartialEq for AccountDisc {
	#[inline]
	fn eq(&self, other: &AccountDisc) -> bool {
		let __self_discr = ::core::intrinsics::discriminant_value(self);
		let __arg1_discr = ::core::intrinsics::discriminant_value(other);
		__self_discr == __arg1_discr
	}
}
#[automatically_derived]
impl ::core::cmp::Eq for AccountDisc {
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
impl ::core::convert::From<AccountDisc> for u8 {
	#[inline]
	fn from(enum_value: AccountDisc) -> Self {
		enum_value as Self
	}
}
impl ::core::convert::TryFrom<u8> for AccountDisc {
	type Error = ::pina::ProgramError;

	#[inline]
	fn try_from(number: u8) -> ::core::result::Result<Self, ::pina::ProgramError> {
		#![allow(non_upper_case_globals)]
		const __CONFIG_STATE: u8 = 1;
		const __GAME_STATE: u8 = 2;
		const __DATA_ACCOUNT: u8 = 3;
		const __BALANCE_ACCOUNT: u8 = 4;
		const __CUSTOM: u8 = 5;
		const __LARGE_STATE: u8 = 6;
		#[deny(unreachable_patterns)]
		match number {
			__CONFIG_STATE => ::core::result::Result::Ok(Self::ConfigState),
			__GAME_STATE => ::core::result::Result::Ok(Self::GameState),
			__DATA_ACCOUNT => ::core::result::Result::Ok(Self::DataAccount),
			__BALANCE_ACCOUNT => ::core::result::Result::Ok(Self::BalanceAccount),
			__CUSTOM => ::core::result::Result::Ok(Self::Custom),
			__LARGE_STATE => ::core::result::Result::Ok(Self::LargeState),
			#[allow(unreachable_patterns)]
			_ => ::core::result::Result::Err(::pina::PinaProgramError::InvalidDiscriminator.into()),
		}
	}
}
const _: () = if !(::core::mem::size_of::<AccountDisc>() == ::core::mem::size_of::<u8>()) {
	{
		::core::panicking::panic_fmt(format_args!(
			"The size of the enum `AccountDisc` must match the size of its primitive \
			 representation\n\t\t\t\t`u8`.",
		));
	}
};
impl ::pina::IntoDiscriminator for AccountDisc {
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
pub struct ConfigState {
	discriminator: [u8; AccountDisc::BYTES],
	pub version: u8,
	pub bump: u8,
}
#[repr(C)]
pub struct ConfigStateZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
	discriminator: [u8; AccountDisc::BYTES],
	pub version: u8,
	pub bump: u8,
}
impl Copy for ConfigStateZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
}
impl Clone for ConfigStateZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
	fn clone(&self) -> Self {
		*self
	}
}
const _: () = if !(core::mem::align_of::<ConfigStateZc>() == 1) {
	::core::panicking::panic("assertion failed: core::mem::align_of::<ConfigStateZc>() == 1")
};
impl ConfigStateZc {
	#[inline(always)]
	pub fn discriminator(&self) -> &[u8; AccountDisc::BYTES] {
		&self.discriminator
	}

	#[inline(always)]
	pub fn version(&self) -> u8 {
		self.version
	}

	#[inline(always)]
	pub fn bump(&self) -> u8 {
		self.bump
	}
}
impl zeropod::ZcValidate for ConfigStateZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
	fn validate_ref(value: &Self) -> Result<(), zeropod::ZeroPodError> {
		<[u8; AccountDisc::BYTES] as zeropod::ZcValidate>::validate_ref(&value.discriminator)?;
		<u8 as zeropod::ZcValidate>::validate_ref(&value.version)?;
		<u8 as zeropod::ZcValidate>::validate_ref(&value.bump)?;
		Ok(())
	}
}
impl zeropod::ZeroPodSchema for ConfigState
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
	const LAYOUT: zeropod::LayoutKind = zeropod::LayoutKind::Fixed;
}
impl zeropod::ZeroPodFixed for ConfigState
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
	type Zc = ConfigStateZc;

	const SIZE: usize = core::mem::size_of::<ConfigStateZc>();

	fn from_bytes(data: &[u8]) -> Result<&Self::Zc, zeropod::ZeroPodError> {
		Self::validate(data)?;
		Ok(unsafe { &*(data.as_ptr() as *const Self::Zc) })
	}

	fn from_bytes_mut(data: &mut [u8]) -> Result<&mut Self::Zc, zeropod::ZeroPodError> {
		Self::validate(data)?;
		Ok(unsafe { &mut *(data.as_mut_ptr() as *mut Self::Zc) })
	}

	fn validate(data: &[u8]) -> Result<(), zeropod::ZeroPodError> {
		if data.len() < core::mem::size_of::<ConfigStateZc>() {
			return Err(zeropod::ZeroPodError::BufferTooSmall);
		}
		let __zc = unsafe { &*(data.as_ptr() as *const Self::Zc) };
		<Self::Zc as zeropod::ZcValidate>::validate_ref(__zc)?;
		Ok(())
	}
}
impl zeropod::ZcField for ConfigState
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
	type Pod = ConfigStateZc;

	const POD_SIZE: usize = core::mem::size_of::<ConfigStateZc>();
}
unsafe impl zeropod::ZcElem for ConfigStateZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
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
	assert_storage::<ConfigStateZc>();
};
const _: () = {
	if !(::core::mem::align_of::<ConfigStateZc>() == 1) {
		::core::panicking::panic("assertion failed: ::core::mem::align_of::<ConfigStateZc>() == 1")
	}
	if !(::core::mem::size_of::<ConfigStateZc>()
		== AccountDisc::BYTES
			+ ::core::mem::size_of::<::core::primitive::u8>()
			+ ::core::mem::size_of::<::core::primitive::u8>())
	{
		::core::panicking::panic(
			"assertion failed: ::core::mem::size_of::<ConfigStateZc>() ==\n    AccountDisc::BYTES \
			 + ::core::mem::size_of::<::core::primitive::u8>() +\n        \
			 ::core::mem::size_of::<::core::primitive::u8>()",
		)
	}
};
impl ConfigState {
	/// The exact number of bytes required by the zeropod representation.
	pub const SIZE: usize = <Self as pina::ZeroPodFixed>::SIZE;

	/// Validate `data` and return zeropod's immutable zero-copy companion.
	pub fn try_from_bytes(
		data: &[u8],
	) -> Result<&<Self as pina::ZeroPodFixed>::Zc, pina::ProgramError> {
		if data.len() != Self::SIZE
			|| !<Self as pina::HasDiscriminator>::matches_discriminator(data)
		{
			return Err(pina::ProgramError::InvalidAccountData);
		}
		<Self as pina::ZeroPodFixed>::from_bytes(data)
			.map_err(|_| pina::ProgramError::InvalidAccountData)
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
			return Err(pina::ProgramError::InvalidAccountData);
		}
		data.fill(0);
		<Self as pina::HasDiscriminator>::write_discriminator(data);
		<Self as pina::ZeroPodFixed>::from_bytes_mut(data)
			.map_err(|_| pina::ProgramError::InvalidAccountData)
	}
}
impl pina::HasDiscriminator for ConfigState {
	type Type = AccountDisc;

	const VALUE: Self::Type = AccountDisc::ConfigState;
}
impl pina::AccountValidation for ConfigStateZc {
	#[track_caller]
	fn assert<F>(&self, condition: F) -> Result<&Self, pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		if condition(self) {
			return Ok(self);
		}
		pina::log_caller();
		Err(pina::ProgramError::InvalidAccountData)
	}

	#[track_caller]
	fn assert_msg<F>(&self, condition: F, msg: &str) -> Result<&Self, pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		match pina::assert(condition(self), pina::ProgramError::InvalidAccountData, msg) {
			Err(err) => Err(err),
			Ok(()) => Ok(self),
		}
	}

	#[track_caller]
	fn assert_mut<F>(&mut self, condition: F) -> Result<&mut Self, pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		if condition(self) {
			return Ok(self);
		}
		pina::log_caller();
		Err(pina::ProgramError::InvalidAccountData)
	}

	#[track_caller]
	fn assert_mut_msg<F>(
		&mut self,
		condition: F,
		msg: &str,
	) -> Result<&mut Self, pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		match pina::assert(condition(self), pina::ProgramError::InvalidAccountData, msg) {
			Err(err) => Err(err),
			Ok(()) => Ok(self),
		}
	}
}
impl pina::PinaAccount for ConfigState {}
pub struct GameState {
	discriminator: [u8; AccountDisc::BYTES],
	pub score: u8,
	pub level: u8,
}
#[automatically_derived]
impl ::core::fmt::Debug for GameState {
	#[inline]
	fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
		::core::fmt::Formatter::debug_struct_field3_finish(
			f,
			"GameState",
			"discriminator",
			&self.discriminator,
			"score",
			&self.score,
			"level",
			&&self.level,
		)
	}
}
#[repr(C)]
pub struct GameStateZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
	discriminator: [u8; AccountDisc::BYTES],
	pub score: u8,
	pub level: u8,
}
impl Copy for GameStateZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
}
impl Clone for GameStateZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
	fn clone(&self) -> Self {
		*self
	}
}
const _: () = if !(core::mem::align_of::<GameStateZc>() == 1) {
	::core::panicking::panic("assertion failed: core::mem::align_of::<GameStateZc>() == 1")
};
impl GameStateZc {
	#[inline(always)]
	pub fn discriminator(&self) -> &[u8; AccountDisc::BYTES] {
		&self.discriminator
	}

	#[inline(always)]
	pub fn score(&self) -> u8 {
		self.score
	}

	#[inline(always)]
	pub fn level(&self) -> u8 {
		self.level
	}
}
impl zeropod::ZcValidate for GameStateZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
	fn validate_ref(value: &Self) -> Result<(), zeropod::ZeroPodError> {
		<[u8; AccountDisc::BYTES] as zeropod::ZcValidate>::validate_ref(&value.discriminator)?;
		<u8 as zeropod::ZcValidate>::validate_ref(&value.score)?;
		<u8 as zeropod::ZcValidate>::validate_ref(&value.level)?;
		Ok(())
	}
}
impl zeropod::ZeroPodSchema for GameState
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
	const LAYOUT: zeropod::LayoutKind = zeropod::LayoutKind::Fixed;
}
impl zeropod::ZeroPodFixed for GameState
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
	type Zc = GameStateZc;

	const SIZE: usize = core::mem::size_of::<GameStateZc>();

	fn from_bytes(data: &[u8]) -> Result<&Self::Zc, zeropod::ZeroPodError> {
		Self::validate(data)?;
		Ok(unsafe { &*(data.as_ptr() as *const Self::Zc) })
	}

	fn from_bytes_mut(data: &mut [u8]) -> Result<&mut Self::Zc, zeropod::ZeroPodError> {
		Self::validate(data)?;
		Ok(unsafe { &mut *(data.as_mut_ptr() as *mut Self::Zc) })
	}

	fn validate(data: &[u8]) -> Result<(), zeropod::ZeroPodError> {
		if data.len() < core::mem::size_of::<GameStateZc>() {
			return Err(zeropod::ZeroPodError::BufferTooSmall);
		}
		let __zc = unsafe { &*(data.as_ptr() as *const Self::Zc) };
		<Self::Zc as zeropod::ZcValidate>::validate_ref(__zc)?;
		Ok(())
	}
}
impl zeropod::ZcField for GameState
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
	type Pod = GameStateZc;

	const POD_SIZE: usize = core::mem::size_of::<GameStateZc>();
}
unsafe impl zeropod::ZcElem for GameStateZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
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
	assert_storage::<GameStateZc>();
};
const _: () = {
	if !(::core::mem::align_of::<GameStateZc>() == 1) {
		::core::panicking::panic("assertion failed: ::core::mem::align_of::<GameStateZc>() == 1")
	}
	if !(::core::mem::size_of::<GameStateZc>()
		== AccountDisc::BYTES
			+ ::core::mem::size_of::<::core::primitive::u8>()
			+ ::core::mem::size_of::<::core::primitive::u8>())
	{
		::core::panicking::panic(
			"assertion failed: ::core::mem::size_of::<GameStateZc>() ==\n    AccountDisc::BYTES + \
			 ::core::mem::size_of::<::core::primitive::u8>() +\n        \
			 ::core::mem::size_of::<::core::primitive::u8>()",
		)
	}
};
impl GameState {
	/// The exact number of bytes required by the zeropod representation.
	pub const SIZE: usize = <Self as pina::ZeroPodFixed>::SIZE;

	/// Validate `data` and return zeropod's immutable zero-copy companion.
	pub fn try_from_bytes(
		data: &[u8],
	) -> Result<&<Self as pina::ZeroPodFixed>::Zc, pina::ProgramError> {
		if data.len() != Self::SIZE
			|| !<Self as pina::HasDiscriminator>::matches_discriminator(data)
		{
			return Err(pina::ProgramError::InvalidAccountData);
		}
		<Self as pina::ZeroPodFixed>::from_bytes(data)
			.map_err(|_| pina::ProgramError::InvalidAccountData)
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
			return Err(pina::ProgramError::InvalidAccountData);
		}
		data.fill(0);
		<Self as pina::HasDiscriminator>::write_discriminator(data);
		<Self as pina::ZeroPodFixed>::from_bytes_mut(data)
			.map_err(|_| pina::ProgramError::InvalidAccountData)
	}
}
impl pina::HasDiscriminator for GameState {
	type Type = AccountDisc;

	const VALUE: Self::Type = AccountDisc::GameState;
}
impl pina::AccountValidation for GameStateZc {
	#[track_caller]
	fn assert<F>(&self, condition: F) -> Result<&Self, pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		if condition(self) {
			return Ok(self);
		}
		pina::log_caller();
		Err(pina::ProgramError::InvalidAccountData)
	}

	#[track_caller]
	fn assert_msg<F>(&self, condition: F, msg: &str) -> Result<&Self, pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		match pina::assert(condition(self), pina::ProgramError::InvalidAccountData, msg) {
			Err(err) => Err(err),
			Ok(()) => Ok(self),
		}
	}

	#[track_caller]
	fn assert_mut<F>(&mut self, condition: F) -> Result<&mut Self, pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		if condition(self) {
			return Ok(self);
		}
		pina::log_caller();
		Err(pina::ProgramError::InvalidAccountData)
	}

	#[track_caller]
	fn assert_mut_msg<F>(
		&mut self,
		condition: F,
		msg: &str,
	) -> Result<&mut Self, pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		match pina::assert(condition(self), pina::ProgramError::InvalidAccountData, msg) {
			Err(err) => Err(err),
			Ok(()) => Ok(self),
		}
	}
}
impl pina::PinaAccount for GameState {}
pub struct DataAccount {
	discriminator: [u8; AccountDisc::BYTES],
	pub authority: [u8; 32],
	pub data: [u8; 64],
	pub flags: [u8; 4],
}
#[repr(C)]
pub struct DataAccountZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	[u8; 64]: zeropod::ZcValidate,
	[u8; 4]: zeropod::ZcValidate,
{
	discriminator: [u8; AccountDisc::BYTES],
	pub authority: [u8; 32],
	pub data: [u8; 64],
	pub flags: [u8; 4],
}
impl Copy for DataAccountZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	[u8; 64]: zeropod::ZcValidate,
	[u8; 4]: zeropod::ZcValidate,
{
}
impl Clone for DataAccountZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	[u8; 64]: zeropod::ZcValidate,
	[u8; 4]: zeropod::ZcValidate,
{
	fn clone(&self) -> Self {
		*self
	}
}
const _: () = if !(core::mem::align_of::<DataAccountZc>() == 1) {
	::core::panicking::panic("assertion failed: core::mem::align_of::<DataAccountZc>() == 1")
};
impl DataAccountZc {
	#[inline(always)]
	pub fn discriminator(&self) -> &[u8; AccountDisc::BYTES] {
		&self.discriminator
	}

	#[inline(always)]
	pub fn authority(&self) -> &[u8; 32] {
		&self.authority
	}

	#[inline(always)]
	pub fn data(&self) -> &[u8; 64] {
		&self.data
	}

	#[inline(always)]
	pub fn flags(&self) -> &[u8; 4] {
		&self.flags
	}
}
impl zeropod::ZcValidate for DataAccountZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	[u8; 64]: zeropod::ZcValidate,
	[u8; 4]: zeropod::ZcValidate,
{
	fn validate_ref(value: &Self) -> Result<(), zeropod::ZeroPodError> {
		<[u8; AccountDisc::BYTES] as zeropod::ZcValidate>::validate_ref(&value.discriminator)?;
		<[u8; 32] as zeropod::ZcValidate>::validate_ref(&value.authority)?;
		<[u8; 64] as zeropod::ZcValidate>::validate_ref(&value.data)?;
		<[u8; 4] as zeropod::ZcValidate>::validate_ref(&value.flags)?;
		Ok(())
	}
}
impl zeropod::ZeroPodSchema for DataAccount
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	[u8; 64]: zeropod::ZcValidate,
	[u8; 4]: zeropod::ZcValidate,
{
	const LAYOUT: zeropod::LayoutKind = zeropod::LayoutKind::Fixed;
}
impl zeropod::ZeroPodFixed for DataAccount
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	[u8; 64]: zeropod::ZcValidate,
	[u8; 4]: zeropod::ZcValidate,
{
	type Zc = DataAccountZc;

	const SIZE: usize = core::mem::size_of::<DataAccountZc>();

	fn from_bytes(data: &[u8]) -> Result<&Self::Zc, zeropod::ZeroPodError> {
		Self::validate(data)?;
		Ok(unsafe { &*(data.as_ptr() as *const Self::Zc) })
	}

	fn from_bytes_mut(data: &mut [u8]) -> Result<&mut Self::Zc, zeropod::ZeroPodError> {
		Self::validate(data)?;
		Ok(unsafe { &mut *(data.as_mut_ptr() as *mut Self::Zc) })
	}

	fn validate(data: &[u8]) -> Result<(), zeropod::ZeroPodError> {
		if data.len() < core::mem::size_of::<DataAccountZc>() {
			return Err(zeropod::ZeroPodError::BufferTooSmall);
		}
		let __zc = unsafe { &*(data.as_ptr() as *const Self::Zc) };
		<Self::Zc as zeropod::ZcValidate>::validate_ref(__zc)?;
		Ok(())
	}
}
impl zeropod::ZcField for DataAccount
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	[u8; 64]: zeropod::ZcValidate,
	[u8; 4]: zeropod::ZcValidate,
{
	type Pod = DataAccountZc;

	const POD_SIZE: usize = core::mem::size_of::<DataAccountZc>();
}
unsafe impl zeropod::ZcElem for DataAccountZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	[u8; 64]: zeropod::ZcValidate,
	[u8; 4]: zeropod::ZcValidate,
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
const _: fn([u8; 64]) -> [::core::primitive::u8; 64] = |value| value;
const _: fn() = || {
	fn assert_mapping<T: pina::ZcField<Pod = [::core::primitive::u8; 64]>>() {}
	fn assert_storage<T: pina::ZcElem>() {}
	assert_mapping::<[u8; 64]>();
	assert_storage::<[::core::primitive::u8; 64]>();
};
const _: () = {
	if !(::core::mem::align_of::<[::core::primitive::u8; 64]>() == 1) {
		::core::panicking::panic(
			"assertion failed: ::core::mem::align_of::<[::core::primitive::u8; 64]>() == 1",
		)
	}
	if !(::core::mem::size_of::<[::core::primitive::u8; 64]>()
		== <[u8; 64] as pina::ZcField>::POD_SIZE)
	{
		::core::panicking::panic(
			"assertion failed: ::core::mem::size_of::<[::core::primitive::u8; 64]>() ==\n    \
			 <[u8; 64] as pina::ZcField>::POD_SIZE",
		)
	}
};
const _: fn([u8; 4]) -> [::core::primitive::u8; 4] = |value| value;
const _: fn() = || {
	fn assert_mapping<T: pina::ZcField<Pod = [::core::primitive::u8; 4]>>() {}
	fn assert_storage<T: pina::ZcElem>() {}
	assert_mapping::<[u8; 4]>();
	assert_storage::<[::core::primitive::u8; 4]>();
};
const _: () = {
	if !(::core::mem::align_of::<[::core::primitive::u8; 4]>() == 1) {
		::core::panicking::panic(
			"assertion failed: ::core::mem::align_of::<[::core::primitive::u8; 4]>() == 1",
		)
	}
	if !(::core::mem::size_of::<[::core::primitive::u8; 4]>()
		== <[u8; 4] as pina::ZcField>::POD_SIZE)
	{
		::core::panicking::panic(
			"assertion failed: ::core::mem::size_of::<[::core::primitive::u8; 4]>() ==\n    <[u8; \
			 4] as pina::ZcField>::POD_SIZE",
		)
	}
};
const _: fn() = || {
	fn assert_storage<T: pina::ZcElem>() {}
	assert_storage::<DataAccountZc>();
};
const _: () = {
	if !(::core::mem::align_of::<DataAccountZc>() == 1) {
		::core::panicking::panic("assertion failed: ::core::mem::align_of::<DataAccountZc>() == 1")
	}
	if !(::core::mem::size_of::<DataAccountZc>()
		== AccountDisc::BYTES
			+ ::core::mem::size_of::<[::core::primitive::u8; 32]>()
			+ ::core::mem::size_of::<[::core::primitive::u8; 64]>()
			+ ::core::mem::size_of::<[::core::primitive::u8; 4]>())
	{
		::core::panicking::panic(
			"assertion failed: ::core::mem::size_of::<DataAccountZc>() ==\n    AccountDisc::BYTES \
			 + ::core::mem::size_of::<[::core::primitive::u8; 32]>()\n            + \
			 ::core::mem::size_of::<[::core::primitive::u8; 64]>() +\n        \
			 ::core::mem::size_of::<[::core::primitive::u8; 4]>()",
		)
	}
};
impl DataAccount {
	/// The exact number of bytes required by the zeropod representation.
	pub const SIZE: usize = <Self as pina::ZeroPodFixed>::SIZE;

	/// Validate `data` and return zeropod's immutable zero-copy companion.
	pub fn try_from_bytes(
		data: &[u8],
	) -> Result<&<Self as pina::ZeroPodFixed>::Zc, pina::ProgramError> {
		if data.len() != Self::SIZE
			|| !<Self as pina::HasDiscriminator>::matches_discriminator(data)
		{
			return Err(pina::ProgramError::InvalidAccountData);
		}
		<Self as pina::ZeroPodFixed>::from_bytes(data)
			.map_err(|_| pina::ProgramError::InvalidAccountData)
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
			return Err(pina::ProgramError::InvalidAccountData);
		}
		data.fill(0);
		<Self as pina::HasDiscriminator>::write_discriminator(data);
		<Self as pina::ZeroPodFixed>::from_bytes_mut(data)
			.map_err(|_| pina::ProgramError::InvalidAccountData)
	}
}
impl pina::HasDiscriminator for DataAccount {
	type Type = AccountDisc;

	const VALUE: Self::Type = AccountDisc::DataAccount;
}
impl pina::AccountValidation for DataAccountZc {
	#[track_caller]
	fn assert<F>(&self, condition: F) -> Result<&Self, pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		if condition(self) {
			return Ok(self);
		}
		pina::log_caller();
		Err(pina::ProgramError::InvalidAccountData)
	}

	#[track_caller]
	fn assert_msg<F>(&self, condition: F, msg: &str) -> Result<&Self, pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		match pina::assert(condition(self), pina::ProgramError::InvalidAccountData, msg) {
			Err(err) => Err(err),
			Ok(()) => Ok(self),
		}
	}

	#[track_caller]
	fn assert_mut<F>(&mut self, condition: F) -> Result<&mut Self, pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		if condition(self) {
			return Ok(self);
		}
		pina::log_caller();
		Err(pina::ProgramError::InvalidAccountData)
	}

	#[track_caller]
	fn assert_mut_msg<F>(
		&mut self,
		condition: F,
		msg: &str,
	) -> Result<&mut Self, pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		match pina::assert(condition(self), pina::ProgramError::InvalidAccountData, msg) {
			Err(err) => Err(err),
			Ok(()) => Ok(self),
		}
	}
}
impl pina::PinaAccount for DataAccount {}
pub struct BalanceAccount {
	discriminator: [u8; AccountDisc::BYTES],
	pub owner: [u8; 32],
	pub amount: PodU64,
	pub decimals: u8,
	pub is_frozen: PodBool,
}
#[repr(C)]
pub struct BalanceAccountZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	<PodBool as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
	discriminator: [u8; AccountDisc::BYTES],
	pub owner: [u8; 32],
	pub amount: <PodU64 as zeropod::ZcField>::Pod,
	pub decimals: u8,
	pub is_frozen: <PodBool as zeropod::ZcField>::Pod,
}
impl Copy for BalanceAccountZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	<PodBool as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
}
impl Clone for BalanceAccountZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	<PodBool as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
	fn clone(&self) -> Self {
		*self
	}
}
const _: () = if !(core::mem::align_of::<BalanceAccountZc>() == 1) {
	::core::panicking::panic("assertion failed: core::mem::align_of::<BalanceAccountZc>() == 1")
};
impl BalanceAccountZc {
	#[inline(always)]
	pub fn discriminator(&self) -> &[u8; AccountDisc::BYTES] {
		&self.discriminator
	}

	#[inline(always)]
	pub fn owner(&self) -> &[u8; 32] {
		&self.owner
	}

	#[inline(always)]
	pub fn amount(&self) -> &<PodU64 as zeropod::ZcField>::Pod {
		&self.amount
	}

	#[inline(always)]
	pub fn decimals(&self) -> u8 {
		self.decimals
	}

	#[inline(always)]
	pub fn is_frozen(&self) -> &<PodBool as zeropod::ZcField>::Pod {
		&self.is_frozen
	}
}
impl zeropod::ZcValidate for BalanceAccountZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	<PodBool as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
	fn validate_ref(value: &Self) -> Result<(), zeropod::ZeroPodError> {
		<[u8; AccountDisc::BYTES] as zeropod::ZcValidate>::validate_ref(&value.discriminator)?;
		<[u8; 32] as zeropod::ZcValidate>::validate_ref(&value.owner)?;
		<<PodU64 as zeropod::ZcField>::Pod as zeropod::ZcValidate>::validate_ref(&value.amount)?;
		<u8 as zeropod::ZcValidate>::validate_ref(&value.decimals)?;
		<<PodBool as zeropod::ZcField>::Pod as zeropod::ZcValidate>::validate_ref(
			&value.is_frozen,
		)?;
		Ok(())
	}
}
impl zeropod::ZeroPodSchema for BalanceAccount
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	<PodBool as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
	const LAYOUT: zeropod::LayoutKind = zeropod::LayoutKind::Fixed;
}
impl zeropod::ZeroPodFixed for BalanceAccount
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	<PodBool as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
	type Zc = BalanceAccountZc;

	const SIZE: usize = core::mem::size_of::<BalanceAccountZc>();

	fn from_bytes(data: &[u8]) -> Result<&Self::Zc, zeropod::ZeroPodError> {
		Self::validate(data)?;
		Ok(unsafe { &*(data.as_ptr() as *const Self::Zc) })
	}

	fn from_bytes_mut(data: &mut [u8]) -> Result<&mut Self::Zc, zeropod::ZeroPodError> {
		Self::validate(data)?;
		Ok(unsafe { &mut *(data.as_mut_ptr() as *mut Self::Zc) })
	}

	fn validate(data: &[u8]) -> Result<(), zeropod::ZeroPodError> {
		if data.len() < core::mem::size_of::<BalanceAccountZc>() {
			return Err(zeropod::ZeroPodError::BufferTooSmall);
		}
		let __zc = unsafe { &*(data.as_ptr() as *const Self::Zc) };
		<Self::Zc as zeropod::ZcValidate>::validate_ref(__zc)?;
		Ok(())
	}
}
impl zeropod::ZcField for BalanceAccount
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	<PodBool as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
	type Pod = BalanceAccountZc;

	const POD_SIZE: usize = core::mem::size_of::<BalanceAccountZc>();
}
unsafe impl zeropod::ZcElem for BalanceAccountZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	<PodBool as zeropod::ZcField>::Pod: zeropod::ZcValidate,
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
const _: fn(PodBool) -> pina::PodBool = |value| value;
const _: fn() = || {
	fn assert_mapping<T: pina::ZcField<Pod = pina::PodBool>>() {}
	fn assert_storage<T: pina::ZcElem>() {}
	assert_mapping::<PodBool>();
	assert_storage::<pina::PodBool>();
};
const _: () = {
	if !(::core::mem::align_of::<pina::PodBool>() == 1) {
		::core::panicking::panic("assertion failed: ::core::mem::align_of::<pina::PodBool>() == 1")
	}
	if !(::core::mem::size_of::<pina::PodBool>() == <PodBool as pina::ZcField>::POD_SIZE) {
		::core::panicking::panic(
			"assertion failed: ::core::mem::size_of::<pina::PodBool>() ==\n    <PodBool as \
			 pina::ZcField>::POD_SIZE",
		)
	}
};
const _: fn() = || {
	fn assert_storage<T: pina::ZcElem>() {}
	assert_storage::<BalanceAccountZc>();
};
const _: () = {
	if !(::core::mem::align_of::<BalanceAccountZc>() == 1) {
		::core::panicking::panic(
			"assertion failed: ::core::mem::align_of::<BalanceAccountZc>() == 1",
		)
	}
	if !(::core::mem::size_of::<BalanceAccountZc>()
		== AccountDisc::BYTES
			+ ::core::mem::size_of::<[::core::primitive::u8; 32]>()
			+ ::core::mem::size_of::<pina::PodU64>()
			+ ::core::mem::size_of::<::core::primitive::u8>()
			+ ::core::mem::size_of::<pina::PodBool>())
	{
		::core::panicking::panic(
			"assertion failed: ::core::mem::size_of::<BalanceAccountZc>() ==\n    \
			 AccountDisc::BYTES + ::core::mem::size_of::<[::core::primitive::u8; 32]>()\n                \
			 + ::core::mem::size_of::<pina::PodU64>() +\n            \
			 ::core::mem::size_of::<::core::primitive::u8>() +\n        \
			 ::core::mem::size_of::<pina::PodBool>()",
		)
	}
};
impl BalanceAccount {
	/// The exact number of bytes required by the zeropod representation.
	pub const SIZE: usize = <Self as pina::ZeroPodFixed>::SIZE;

	/// Validate `data` and return zeropod's immutable zero-copy companion.
	pub fn try_from_bytes(
		data: &[u8],
	) -> Result<&<Self as pina::ZeroPodFixed>::Zc, pina::ProgramError> {
		if data.len() != Self::SIZE
			|| !<Self as pina::HasDiscriminator>::matches_discriminator(data)
		{
			return Err(pina::ProgramError::InvalidAccountData);
		}
		<Self as pina::ZeroPodFixed>::from_bytes(data)
			.map_err(|_| pina::ProgramError::InvalidAccountData)
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
			return Err(pina::ProgramError::InvalidAccountData);
		}
		data.fill(0);
		<Self as pina::HasDiscriminator>::write_discriminator(data);
		<Self as pina::ZeroPodFixed>::from_bytes_mut(data)
			.map_err(|_| pina::ProgramError::InvalidAccountData)
	}
}
impl pina::HasDiscriminator for BalanceAccount {
	type Type = AccountDisc;

	const VALUE: Self::Type = AccountDisc::BalanceAccount;
}
impl pina::AccountValidation for BalanceAccountZc {
	#[track_caller]
	fn assert<F>(&self, condition: F) -> Result<&Self, pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		if condition(self) {
			return Ok(self);
		}
		pina::log_caller();
		Err(pina::ProgramError::InvalidAccountData)
	}

	#[track_caller]
	fn assert_msg<F>(&self, condition: F, msg: &str) -> Result<&Self, pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		match pina::assert(condition(self), pina::ProgramError::InvalidAccountData, msg) {
			Err(err) => Err(err),
			Ok(()) => Ok(self),
		}
	}

	#[track_caller]
	fn assert_mut<F>(&mut self, condition: F) -> Result<&mut Self, pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		if condition(self) {
			return Ok(self);
		}
		pina::log_caller();
		Err(pina::ProgramError::InvalidAccountData)
	}

	#[track_caller]
	fn assert_mut_msg<F>(
		&mut self,
		condition: F,
		msg: &str,
	) -> Result<&mut Self, pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		match pina::assert(condition(self), pina::ProgramError::InvalidAccountData, msg) {
			Err(err) => Err(err),
			Ok(()) => Ok(self),
		}
	}
}
impl pina::PinaAccount for BalanceAccount {}
pub struct MyStruct {
	discriminator: [u8; AccountDisc::BYTES],
	pub value: u8,
}
#[repr(C)]
pub struct MyStructZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
	discriminator: [u8; AccountDisc::BYTES],
	pub value: u8,
}
impl Copy for MyStructZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
}
impl Clone for MyStructZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
	fn clone(&self) -> Self {
		*self
	}
}
const _: () = if !(core::mem::align_of::<MyStructZc>() == 1) {
	::core::panicking::panic("assertion failed: core::mem::align_of::<MyStructZc>() == 1")
};
impl MyStructZc {
	#[inline(always)]
	pub fn discriminator(&self) -> &[u8; AccountDisc::BYTES] {
		&self.discriminator
	}

	#[inline(always)]
	pub fn value(&self) -> u8 {
		self.value
	}
}
impl zeropod::ZcValidate for MyStructZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
	fn validate_ref(value: &Self) -> Result<(), zeropod::ZeroPodError> {
		<[u8; AccountDisc::BYTES] as zeropod::ZcValidate>::validate_ref(&value.discriminator)?;
		<u8 as zeropod::ZcValidate>::validate_ref(&value.value)?;
		Ok(())
	}
}
impl zeropod::ZeroPodSchema for MyStruct
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
	const LAYOUT: zeropod::LayoutKind = zeropod::LayoutKind::Fixed;
}
impl zeropod::ZeroPodFixed for MyStruct
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
	type Zc = MyStructZc;

	const SIZE: usize = core::mem::size_of::<MyStructZc>();

	fn from_bytes(data: &[u8]) -> Result<&Self::Zc, zeropod::ZeroPodError> {
		Self::validate(data)?;
		Ok(unsafe { &*(data.as_ptr() as *const Self::Zc) })
	}

	fn from_bytes_mut(data: &mut [u8]) -> Result<&mut Self::Zc, zeropod::ZeroPodError> {
		Self::validate(data)?;
		Ok(unsafe { &mut *(data.as_mut_ptr() as *mut Self::Zc) })
	}

	fn validate(data: &[u8]) -> Result<(), zeropod::ZeroPodError> {
		if data.len() < core::mem::size_of::<MyStructZc>() {
			return Err(zeropod::ZeroPodError::BufferTooSmall);
		}
		let __zc = unsafe { &*(data.as_ptr() as *const Self::Zc) };
		<Self::Zc as zeropod::ZcValidate>::validate_ref(__zc)?;
		Ok(())
	}
}
impl zeropod::ZcField for MyStruct
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
{
	type Pod = MyStructZc;

	const POD_SIZE: usize = core::mem::size_of::<MyStructZc>();
}
unsafe impl zeropod::ZcElem for MyStructZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
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
	assert_storage::<MyStructZc>();
};
const _: () = {
	if !(::core::mem::align_of::<MyStructZc>() == 1) {
		::core::panicking::panic("assertion failed: ::core::mem::align_of::<MyStructZc>() == 1")
	}
	if !(::core::mem::size_of::<MyStructZc>()
		== AccountDisc::BYTES + ::core::mem::size_of::<::core::primitive::u8>())
	{
		::core::panicking::panic(
			"assertion failed: ::core::mem::size_of::<MyStructZc>() ==\n    AccountDisc::BYTES + \
			 ::core::mem::size_of::<::core::primitive::u8>()",
		)
	}
};
impl MyStruct {
	/// The exact number of bytes required by the zeropod representation.
	pub const SIZE: usize = <Self as pina::ZeroPodFixed>::SIZE;

	/// Validate `data` and return zeropod's immutable zero-copy companion.
	pub fn try_from_bytes(
		data: &[u8],
	) -> Result<&<Self as pina::ZeroPodFixed>::Zc, pina::ProgramError> {
		if data.len() != Self::SIZE
			|| !<Self as pina::HasDiscriminator>::matches_discriminator(data)
		{
			return Err(pina::ProgramError::InvalidAccountData);
		}
		<Self as pina::ZeroPodFixed>::from_bytes(data)
			.map_err(|_| pina::ProgramError::InvalidAccountData)
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
			return Err(pina::ProgramError::InvalidAccountData);
		}
		data.fill(0);
		<Self as pina::HasDiscriminator>::write_discriminator(data);
		<Self as pina::ZeroPodFixed>::from_bytes_mut(data)
			.map_err(|_| pina::ProgramError::InvalidAccountData)
	}
}
impl pina::HasDiscriminator for MyStruct {
	type Type = AccountDisc;

	const VALUE: Self::Type = AccountDisc::Custom;
}
impl pina::AccountValidation for MyStructZc {
	#[track_caller]
	fn assert<F>(&self, condition: F) -> Result<&Self, pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		if condition(self) {
			return Ok(self);
		}
		pina::log_caller();
		Err(pina::ProgramError::InvalidAccountData)
	}

	#[track_caller]
	fn assert_msg<F>(&self, condition: F, msg: &str) -> Result<&Self, pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		match pina::assert(condition(self), pina::ProgramError::InvalidAccountData, msg) {
			Err(err) => Err(err),
			Ok(()) => Ok(self),
		}
	}

	#[track_caller]
	fn assert_mut<F>(&mut self, condition: F) -> Result<&mut Self, pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		if condition(self) {
			return Ok(self);
		}
		pina::log_caller();
		Err(pina::ProgramError::InvalidAccountData)
	}

	#[track_caller]
	fn assert_mut_msg<F>(
		&mut self,
		condition: F,
		msg: &str,
	) -> Result<&mut Self, pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		match pina::assert(condition(self), pina::ProgramError::InvalidAccountData, msg) {
			Err(err) => Err(err),
			Ok(()) => Ok(self),
		}
	}
}
impl pina::PinaAccount for MyStruct {}
pub struct LargeState {
	discriminator: [u8; AccountDisc::BYTES],
	pub authority: [u8; 32],
	pub bump: u8,
	pub treasury_bump: u8,
	pub mint_bump: u8,
	pub version: u8,
	pub padding: [u8; 3],
	pub total_supply: PodU64,
	pub name: [u8; 32],
}
#[repr(C)]
pub struct LargeStateZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	[u8; 3]: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
{
	discriminator: [u8; AccountDisc::BYTES],
	pub authority: [u8; 32],
	pub bump: u8,
	pub treasury_bump: u8,
	pub mint_bump: u8,
	pub version: u8,
	pub padding: [u8; 3],
	pub total_supply: <PodU64 as zeropod::ZcField>::Pod,
	pub name: [u8; 32],
}
impl Copy for LargeStateZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	[u8; 3]: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
{
}
impl Clone for LargeStateZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	[u8; 3]: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
{
	fn clone(&self) -> Self {
		*self
	}
}
const _: () = if !(core::mem::align_of::<LargeStateZc>() == 1) {
	::core::panicking::panic("assertion failed: core::mem::align_of::<LargeStateZc>() == 1")
};
impl LargeStateZc {
	#[inline(always)]
	pub fn discriminator(&self) -> &[u8; AccountDisc::BYTES] {
		&self.discriminator
	}

	#[inline(always)]
	pub fn authority(&self) -> &[u8; 32] {
		&self.authority
	}

	#[inline(always)]
	pub fn bump(&self) -> u8 {
		self.bump
	}

	#[inline(always)]
	pub fn treasury_bump(&self) -> u8 {
		self.treasury_bump
	}

	#[inline(always)]
	pub fn mint_bump(&self) -> u8 {
		self.mint_bump
	}

	#[inline(always)]
	pub fn version(&self) -> u8 {
		self.version
	}

	#[inline(always)]
	pub fn padding(&self) -> &[u8; 3] {
		&self.padding
	}

	#[inline(always)]
	pub fn total_supply(&self) -> &<PodU64 as zeropod::ZcField>::Pod {
		&self.total_supply
	}

	#[inline(always)]
	pub fn name(&self) -> &[u8; 32] {
		&self.name
	}
}
impl zeropod::ZcValidate for LargeStateZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	[u8; 3]: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
{
	fn validate_ref(value: &Self) -> Result<(), zeropod::ZeroPodError> {
		<[u8; AccountDisc::BYTES] as zeropod::ZcValidate>::validate_ref(&value.discriminator)?;
		<[u8; 32] as zeropod::ZcValidate>::validate_ref(&value.authority)?;
		<u8 as zeropod::ZcValidate>::validate_ref(&value.bump)?;
		<u8 as zeropod::ZcValidate>::validate_ref(&value.treasury_bump)?;
		<u8 as zeropod::ZcValidate>::validate_ref(&value.mint_bump)?;
		<u8 as zeropod::ZcValidate>::validate_ref(&value.version)?;
		<[u8; 3] as zeropod::ZcValidate>::validate_ref(&value.padding)?;
		<<PodU64 as zeropod::ZcField>::Pod as zeropod::ZcValidate>::validate_ref(
			&value.total_supply,
		)?;
		<[u8; 32] as zeropod::ZcValidate>::validate_ref(&value.name)?;
		Ok(())
	}
}
impl zeropod::ZeroPodSchema for LargeState
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	[u8; 3]: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
{
	const LAYOUT: zeropod::LayoutKind = zeropod::LayoutKind::Fixed;
}
impl zeropod::ZeroPodFixed for LargeState
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	[u8; 3]: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
{
	type Zc = LargeStateZc;

	const SIZE: usize = core::mem::size_of::<LargeStateZc>();

	fn from_bytes(data: &[u8]) -> Result<&Self::Zc, zeropod::ZeroPodError> {
		Self::validate(data)?;
		Ok(unsafe { &*(data.as_ptr() as *const Self::Zc) })
	}

	fn from_bytes_mut(data: &mut [u8]) -> Result<&mut Self::Zc, zeropod::ZeroPodError> {
		Self::validate(data)?;
		Ok(unsafe { &mut *(data.as_mut_ptr() as *mut Self::Zc) })
	}

	fn validate(data: &[u8]) -> Result<(), zeropod::ZeroPodError> {
		if data.len() < core::mem::size_of::<LargeStateZc>() {
			return Err(zeropod::ZeroPodError::BufferTooSmall);
		}
		let __zc = unsafe { &*(data.as_ptr() as *const Self::Zc) };
		<Self::Zc as zeropod::ZcValidate>::validate_ref(__zc)?;
		Ok(())
	}
}
impl zeropod::ZcField for LargeState
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	[u8; 3]: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
{
	type Pod = LargeStateZc;

	const POD_SIZE: usize = core::mem::size_of::<LargeStateZc>();
}
unsafe impl zeropod::ZcElem for LargeStateZc
where
	[u8; AccountDisc::BYTES]: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	u8: zeropod::ZcValidate,
	[u8; 3]: zeropod::ZcValidate,
	<PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
	[u8; 32]: zeropod::ZcValidate,
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
const _: fn([u8; 3]) -> [::core::primitive::u8; 3] = |value| value;
const _: fn() = || {
	fn assert_mapping<T: pina::ZcField<Pod = [::core::primitive::u8; 3]>>() {}
	fn assert_storage<T: pina::ZcElem>() {}
	assert_mapping::<[u8; 3]>();
	assert_storage::<[::core::primitive::u8; 3]>();
};
const _: () = {
	if !(::core::mem::align_of::<[::core::primitive::u8; 3]>() == 1) {
		::core::panicking::panic(
			"assertion failed: ::core::mem::align_of::<[::core::primitive::u8; 3]>() == 1",
		)
	}
	if !(::core::mem::size_of::<[::core::primitive::u8; 3]>()
		== <[u8; 3] as pina::ZcField>::POD_SIZE)
	{
		::core::panicking::panic(
			"assertion failed: ::core::mem::size_of::<[::core::primitive::u8; 3]>() ==\n    <[u8; \
			 3] as pina::ZcField>::POD_SIZE",
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
const _: fn() = || {
	fn assert_storage<T: pina::ZcElem>() {}
	assert_storage::<LargeStateZc>();
};
const _: () = {
	if !(::core::mem::align_of::<LargeStateZc>() == 1) {
		::core::panicking::panic("assertion failed: ::core::mem::align_of::<LargeStateZc>() == 1")
	}
	if !(::core::mem::size_of::<LargeStateZc>()
		== AccountDisc::BYTES
			+ ::core::mem::size_of::<[::core::primitive::u8; 32]>()
			+ ::core::mem::size_of::<::core::primitive::u8>()
			+ ::core::mem::size_of::<::core::primitive::u8>()
			+ ::core::mem::size_of::<::core::primitive::u8>()
			+ ::core::mem::size_of::<::core::primitive::u8>()
			+ ::core::mem::size_of::<[::core::primitive::u8; 3]>()
			+ ::core::mem::size_of::<pina::PodU64>()
			+ ::core::mem::size_of::<[::core::primitive::u8; 32]>())
	{
		::core::panicking::panic(
			"assertion failed: ::core::mem::size_of::<LargeStateZc>() ==\n    AccountDisc::BYTES \
			 + ::core::mem::size_of::<[::core::primitive::u8; 32]>()\n                                \
			 + ::core::mem::size_of::<::core::primitive::u8>() +\n                            \
			 ::core::mem::size_of::<::core::primitive::u8>() +\n                        \
			 ::core::mem::size_of::<::core::primitive::u8>() +\n                    \
			 ::core::mem::size_of::<::core::primitive::u8>() +\n                \
			 ::core::mem::size_of::<[::core::primitive::u8; 3]>() +\n            \
			 ::core::mem::size_of::<pina::PodU64>() +\n        \
			 ::core::mem::size_of::<[::core::primitive::u8; 32]>()",
		)
	}
};
impl LargeState {
	/// The exact number of bytes required by the zeropod representation.
	pub const SIZE: usize = <Self as pina::ZeroPodFixed>::SIZE;

	/// Validate `data` and return zeropod's immutable zero-copy companion.
	pub fn try_from_bytes(
		data: &[u8],
	) -> Result<&<Self as pina::ZeroPodFixed>::Zc, pina::ProgramError> {
		if data.len() != Self::SIZE
			|| !<Self as pina::HasDiscriminator>::matches_discriminator(data)
		{
			return Err(pina::ProgramError::InvalidAccountData);
		}
		<Self as pina::ZeroPodFixed>::from_bytes(data)
			.map_err(|_| pina::ProgramError::InvalidAccountData)
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
			return Err(pina::ProgramError::InvalidAccountData);
		}
		data.fill(0);
		<Self as pina::HasDiscriminator>::write_discriminator(data);
		<Self as pina::ZeroPodFixed>::from_bytes_mut(data)
			.map_err(|_| pina::ProgramError::InvalidAccountData)
	}
}
impl pina::HasDiscriminator for LargeState {
	type Type = AccountDisc;

	const VALUE: Self::Type = AccountDisc::LargeState;
}
impl pina::AccountValidation for LargeStateZc {
	#[track_caller]
	fn assert<F>(&self, condition: F) -> Result<&Self, pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		if condition(self) {
			return Ok(self);
		}
		pina::log_caller();
		Err(pina::ProgramError::InvalidAccountData)
	}

	#[track_caller]
	fn assert_msg<F>(&self, condition: F, msg: &str) -> Result<&Self, pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		match pina::assert(condition(self), pina::ProgramError::InvalidAccountData, msg) {
			Err(err) => Err(err),
			Ok(()) => Ok(self),
		}
	}

	#[track_caller]
	fn assert_mut<F>(&mut self, condition: F) -> Result<&mut Self, pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		if condition(self) {
			return Ok(self);
		}
		pina::log_caller();
		Err(pina::ProgramError::InvalidAccountData)
	}

	#[track_caller]
	fn assert_mut_msg<F>(
		&mut self,
		condition: F,
		msg: &str,
	) -> Result<&mut Self, pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		match pina::assert(condition(self), pina::ProgramError::InvalidAccountData, msg) {
			Err(err) => Err(err),
			Ok(()) => Ok(self),
		}
	}
}
impl pina::PinaAccount for LargeState {}
