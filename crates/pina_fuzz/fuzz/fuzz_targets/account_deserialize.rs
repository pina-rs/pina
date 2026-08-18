//! Fuzz harness for `PinaAccount::try_from_bytes`.
//!
//! Feeds arbitrary byte slices to the account deserialization path for
//! real account types from the counter_program and role_registry_program
//! examples, exercising discriminator checks, size validation, and
//! zeropod content validation and zero-copy reinterpretation.

#![allow(clippy::all)]
#![no_main]

use counter_program::CounterState;
use libfuzzer_sys::fuzz_target;
use role_registry_program::RegistryConfig;
use role_registry_program::RoleEntry;

const _: [(); 10] = [(); CounterState::SIZE];
const _: [(); 42] = [(); RegistryConfig::SIZE];
const _: [(); 83] = [(); RoleEntry::SIZE];

// Exercise every real account layout from one libFuzzer entry point. Defining
// multiple `fuzz_target!` invocations in a binary emits duplicate linker symbols.
fuzz_target!(|data: &[u8]| {
	let _ = CounterState::try_from_bytes(data);
	let _ = RegistryConfig::try_from_bytes(data);
	let _ = RoleEntry::try_from_bytes(data);
});
