#![allow(dead_code)]

use pina::*;

#[discriminator(crate = ::pina, primitive = u8, final)]
pub enum MyAccount {
	ConfigState = 0,
}

#[account(crate = ::pina, discriminator = MyAccount)]
#[derive(Debug)]
pub struct ConfigState {
	/// The version of the state.
	pub version: u8,
	/// The authority which can update this config.
	pub authority: Address,
	/// Store the bump to save compute units.
	pub bump: u8,
}

#[test]
fn test_account_macro() {
	let authority = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let config_state = ConfigState::builder()
		.version(1)
		.authority(authority)
		.bump(255)
		.build();

	assert_eq!(config_state.version, 1);
	assert_eq!(config_state.authority, authority);
	assert_eq!(config_state.bump, 255);

	let mut expected_discriminator = [0u8; MyAccount::BYTES];
	MyAccount::ConfigState.write_discriminator(&mut expected_discriminator);

	assert_eq!(config_state.discriminator, expected_discriminator);
}

#[test]
fn test_account_assert_returns_ok_when_condition_true() {
	let authority = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let config_state = ConfigState::builder()
		.version(1)
		.authority(authority)
		.bump(255)
		.build();

	let result = config_state.assert(|s| s.version == 1);
	assert!(result.is_ok());
}

#[test]
fn test_account_assert_returns_err_when_condition_false() {
	let authority = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let config_state = ConfigState::builder()
		.version(1)
		.authority(authority)
		.bump(255)
		.build();

	let result = config_state.assert(|s| s.version == 99);
	assert!(result.is_err());
	assert_eq!(result.unwrap_err(), ProgramError::InvalidAccountData);
}

#[test]
fn test_account_assert_mut_returns_ok_when_condition_true() {
	let authority = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let mut config_state = ConfigState::builder()
		.version(1)
		.authority(authority)
		.bump(255)
		.build();

	let result = config_state.assert_mut(|s| s.version == 1);
	assert!(result.is_ok());
}

#[test]
fn test_account_assert_mut_returns_err_when_condition_false() {
	let authority = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let mut config_state = ConfigState::builder()
		.version(1)
		.authority(authority)
		.bump(255)
		.build();

	let result = config_state.assert_mut(|s| s.version == 99);
	assert!(result.is_err());
	assert_eq!(result.unwrap_err(), ProgramError::InvalidAccountData);
}

#[test]
fn test_account_assert_msg_returns_ok_when_condition_true() {
	let authority = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let config_state = ConfigState::builder()
		.version(1)
		.authority(authority)
		.bump(255)
		.build();

	let result = config_state.assert_msg(|s| s.bump == 255, "bump must be 255");
	assert!(result.is_ok());
}

#[test]
fn test_account_assert_msg_returns_err_when_condition_false() {
	let authority = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let config_state = ConfigState::builder()
		.version(1)
		.authority(authority)
		.bump(255)
		.build();

	let result = config_state.assert_msg(|s| s.bump == 0, "bump must be 0");
	assert!(result.is_err());
	assert_eq!(result.unwrap_err(), ProgramError::InvalidAccountData);
}
