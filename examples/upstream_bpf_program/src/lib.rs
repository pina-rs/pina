//! Upstream BPF — the sbpf-linker showcase program.
//!
//! This example exists to exercise the **experimental upstream toolchain**:
//! building a pina program with upstream nightly Rust and the pinned
//! `sbpf-linker` (0.2.3, see `[workspace.metadata.bin]`) for the
//! `bpfel-unknown-none` target, instead of Agave's `cargo build-sbf` fork.
//! The program is deliberately ordinary — it must stay inside the toolchain's
//! measured viable envelope (no u128 division, small entrypoint frames, no
//! fat LTO). Every build instruction and caveat lives in this example's
//! `readme.md`; the toolchain comparison matrix lives in
//! `docs/sbpf-linker-evaluation.md`.
//!
//! Two instructions:
//!
//! | Variant | Description                                                        |
//! |---------|---------------------------------------------------------------------|
//! | `Hello` | Log a greeting; validates the signer.                               |
//! | `Ping`  | Validate the `Clock` sysvar account, read it zero-copy, and log it. |
//!
//! `Hello` is the load-bearing half for the upstream build flags: its `log!`
//! reaches the runtime through a *static* syscall, which only exists when
//! the build sets `--cfg=target_feature="static-syscalls"` (see the
//! `[target.bpfel-unknown-none]` rustflags in `.cargo/config.toml`), so the
//! Surfpool suite asserting the greeting is the functional gate for the
//! upstream build flags. `Ping` demonstrates the sysvar-account pattern: the
//! runtime `Sysvar::get` syscall (`sol_get_sysvar`) is valid on mainnet but
//! not implemented by the Surfpool test runtime, so this example reads the
//! `Clock` sysvar account the way `sysvar_checks_program` does.

#![allow(missing_docs)]
#![allow(clippy::inline_always)]
#![no_std]

// On native builds the cdylib target needs std for unwinding and panic
// handling. On BPF, `nostd_entrypoint!()` provides the panic handler and
// allocator. Tests link against std automatically.
#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::sysvars::clock::Clock;
use pina::*;

// ---------------------------------------------------------------------------
// Program ID
// ---------------------------------------------------------------------------

// The on-chain address of this program.
declare_id!("2oYypU3qZSK14274jeqP8XXGmXWn6Zhf4RRpLSDGURmk");

/// The clock sysvar address, validated by the `Ping` instruction.
const CLOCK_SYSVAR_ID: Address = address!("SysvarC1ock11111111111111111111111111111111");

// ---------------------------------------------------------------------------
// Discriminators
// ---------------------------------------------------------------------------

#[discriminator]
pub enum UpstreamInstruction {
	/// Log a greeting to show the program executed.
	Hello = 0,
	/// Read and log the current clock slot through the runtime syscall.
	Ping = 1,
}

// ---------------------------------------------------------------------------
// Instruction data
// ---------------------------------------------------------------------------

/// Greeting request: only the discriminator byte, no payload.
#[instruction(discriminator = UpstreamInstruction::Hello)]
pub struct HelloInstructionData {}

/// Clock request: only the discriminator byte, no payload.
#[instruction(discriminator = UpstreamInstruction::Ping)]
pub struct PingInstructionData {}

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

/// The accounts required by the Hello instruction: a signing caller.
#[derive(Accounts, Debug)]
pub struct HelloAccounts<'a> {
	/// The user invoking the program. Must be a signer so we can trust the
	/// address is authentic.
	pub user: &'a AccountView,
}

/// The accounts required by the Ping instruction: a signing caller plus the
/// clock sysvar account, read zero-copy for its slot.
#[derive(Accounts, Debug)]
pub struct PingAccounts<'a> {
	/// The user invoking the program. Must be a signer so we can trust the
	/// address is authentic.
	pub user: &'a AccountView,
	/// The clock sysvar account, validated against `CLOCK_SYSVAR_ID`.
	pub clock: &'a AccountView,
}

// ---------------------------------------------------------------------------
// Instruction processors
// ---------------------------------------------------------------------------

impl<'a> ProcessAccountInfos<'a> for HelloAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		// Validate instruction data (checks the discriminator byte).
		let _ = HelloInstructionData::try_from_bytes(data)?;

		// Validate that the user signed the transaction.
		self.user.assert_signer()?;

		// Log a greeting to the Solana runtime.
		log!("Hello, upstream BPF!");

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for PingAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		// Validate instruction data (checks the discriminator byte).
		let _ = PingInstructionData::try_from_bytes(data)?;

		// Validate that the user signed the transaction.
		self.user.assert_signer()?;

		// Validate and read the clock sysvar account zero-copy. The runtime
		// `Sysvar::get` syscall is mainnet-valid but not implemented by the
		// Surfpool test runtime, so the example follows the account-based
		// pattern from `sysvar_checks_program`.
		self.clock.assert_sysvar(&CLOCK_SYSVAR_ID)?;
		let clock = Clock::from_account_view(self.clock)?;

		// The slot only drives which static marker is logged: the format-arg
		// form of `log!` requires the `verbose-logs` feature, and linking
		// `core::fmt` would inflate this size-focused showcase program.
		if clock.slot == 0 {
			log!("clock sysvar read at the genesis slot");
		} else {
			log!("clock sysvar read");
		}

		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Entrypoint (only compiled for on-chain BPF builds)
// ---------------------------------------------------------------------------

/// The entrypoint module is gated behind the `bpf-entrypoint` feature so that
/// tests and CPI consumers can use this crate as a library without pulling in
/// the BPF entrypoint machinery.
#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use super::*;

	nostd_entrypoint!(process_instruction);

	/// The top-level instruction router.
	#[inline(always)]
	pub fn process_instruction(
		program_id: &Address,
		accounts: &mut [AccountView],
		data: &[u8],
	) -> ProgramResult {
		let instruction: UpstreamInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			UpstreamInstruction::Hello => {
				HelloAccounts::try_from((program_id, accounts))?.process(data)
			}
			UpstreamInstruction::Ping => {
				PingAccounts::try_from((program_id, accounts))?.process(data)
			}
		}
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn discriminator_hello_value() {
		assert_eq!(UpstreamInstruction::Hello as u8, 0);
	}

	#[test]
	fn discriminator_ping_value() {
		assert_eq!(UpstreamInstruction::Ping as u8, 1);
	}

	#[test]
	fn instruction_data_has_discriminators() {
		assert!(HelloInstructionData::matches_discriminator(&[0u8]));
		assert!(PingInstructionData::matches_discriminator(&[1u8]));
		assert!(!HelloInstructionData::matches_discriminator(&[1u8]));
	}

	#[test]
	fn program_id_is_valid() {
		assert_ne!(ID, Address::default());
	}
}
