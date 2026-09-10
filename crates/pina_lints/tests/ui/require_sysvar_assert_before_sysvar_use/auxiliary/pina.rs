#![allow(dead_code)]

pub struct ClockView;
pub struct RentView;
pub struct OtherView;

pub trait AccountInfoValidation<Id>: Sized {
	fn assert_sysvar(self, id: &Id) -> Result<Self, ()>;
}

impl<Id> AccountInfoValidation<Id> for &ClockView {
	fn assert_sysvar(self, _id: &Id) -> Result<Self, ()> {
		Ok(self)
	}
}

impl ClockView {
	pub fn try_borrow(&self) -> Result<Self, ()> {
		Ok(Self)
	}

	pub fn slot(&self) -> u64 {
		0
	}

	pub fn unix_timestamp(&self) -> i64 {
		0
	}
}

impl<Id> AccountInfoValidation<Id> for &RentView {
	fn assert_sysvar(self, _id: &Id) -> Result<Self, ()> {
		Ok(self)
	}
}

impl RentView {
	pub fn try_borrow(&self) -> Result<Self, ()> {
		Ok(Self)
	}

	pub fn lamports_per_byte(&self) -> u64 {
		0
	}

	pub fn data(&self) -> &[u8] {
		&[]
	}
}

impl OtherView {
	pub fn try_borrow(&self) -> Result<(), ()> {
		Ok(())
	}
}
