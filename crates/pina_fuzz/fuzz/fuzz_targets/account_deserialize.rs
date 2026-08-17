//! Fuzz harness for `PinaAccount::try_from_bytes`.
//!
//! Feeds arbitrary byte slices to the account deserialization path for
//! real account types from the counter_program and role_registry_program
//! examples, exercising discriminator checks, size validation, and
//! bytemuck zero-copy reinterpretation.

#![allow(clippy::all)]

use counter_program::CounterState;
use libfuzzer_sys::fuzz_target;
use pina::PinaAccount;
use role_registry_program::RegistryConfig;
use role_registry_program::RoleEntry;

/// Fuzz `CounterState::try_from_bytes` — a 10-byte account with a
/// single-byte discriminator (`CounterAccountType::CounterState = 1`).
///
/// Valid data must be at least 10 bytes with `[0] == 1`.
/// Anything shorter or with the wrong first byte should return `Err`.
/// Arbitrary lengths and byte patterns exercise alignment, truncation,
/// and discriminator-mismatch branches.
fuzz_target!(|data: &[u8]| {
	let _ = CounterState::try_from_bytes(data);
});

/// Fuzz `RegistryConfig::try_from_bytes` — a 42-byte account
/// (`1 + 32 + 8 + 1`) with discriminator byte `1`.
fuzz_target!(|data: &[u8]| {
	let _ = RegistryConfig::try_from_bytes(data);
});

/// Fuzz `RoleEntry::try_from_bytes` — a 60-byte account with
/// discriminator byte `2`.
fuzz_target!(|data: &[u8]| {
	let _ = RoleEntry::try_from_bytes(data);
});
