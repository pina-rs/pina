//! # pina
//!
//! A performant Solana smart contract framework built on top of
//! [`pinocchio`](https://docs.rs/pinocchio) — a lightweight alternative to
//! `solana-program` that massively reduces dependency bloat and compute units.
//!
//! ## Features
//!
//! - **Zero-copy account deserialization** via `pinapod` — no heap allocation.
//! - **Compact dynamic accounts** *(optional)* with checked trailing vectors
//!   and typed, rent-adjusting creation/reallocation helpers.
//! - **`no_std` compatible** — designed for on-chain deployment to the SBF
//!   target.
//! - **Discriminator system** — every account, instruction, and event type
//!   carries a discriminator as its first field, enabling safe type
//!   identification.
//! - **Validation chaining** — chain assertions on `AccountView` references
//!   (e.g. `account.assert_signer()?.assert_writable()?.assert_owner(&id)?`).
//! - **Declarative validation** *(optional)* — generate allocation-free
//!   `PinaValidate` implementations for accounts, instructions, events, and
//!   instruction account lists.
//! - **Proc-macro sugar** — `#[account]`, `#[instruction]`, `#[event]`,
//!   `#[error]`, `#[discriminator]`, and `#[derive(Accounts)]` reduce
//!   boilerplate.
//! - **CPI helpers** — account creation, PDA allocation, and token operations.
//!
//! ## Crate features
//!
//! - `logs` *(default)* — enables on-chain logging via `solana-program-log`,
//!   including [`emit_event`] and the generated `#[event]` `emit` helper.
//! - `derive` *(default)* — enables the `pina_macros` proc-macro crate.
//! - `compact` — enables compact account schemas, checked loaders, and typed
//!   account APIs. This also enables `derive`.
//! - `floats` — enables IEEE-754 `f32` and `f64` schema fields, stored as
//!   their bit pattern through `PodF32` and `PodF64`. This also enables
//!   `derive`.
//! - `fixed` — enables fixed-point `FixedI*<Frac>` and `FixedU*<Frac>` schema
//!   fields from the pinned `fixed` crate, re-exported as `pina::fixed`. This
//!   also enables `derive`.
//! - `validation` — enables `PinaValidate` and declarative
//!   `#[pina(validate(...))]` rules. This also enables `derive`.
//! - `token` — enables SPL token / token-2022 helpers and associated token
//!   account utilities.
//! - `memo` — enables memo program helpers.
//! - `account-resize` — enables raw realloc helpers on top of Pinocchio's safe
//!   resize support. Enable it with `compact` for typed compact-account creation
//!   and reallocation.

#![no_std]
// Kani injects `feature(register_tool)` while compiling proof harnesses.
#![cfg_attr(kani, allow(unstable_features))]
// CU optimization for on-chain programs; inline_always ensures discriminator
// reads compile to a single load instruction in BPF bytecode.
#![allow(clippy::inline_always)]

mod cpi;
mod error;
mod event;
mod impls;
pub mod introspection;
mod migration;
mod pda;
mod pod;
#[cfg(feature = "token")]
pub mod token;
#[cfg(feature = "token")]
pub mod token_2022;
mod traits;
pub mod transaction;
mod utils;
#[cfg(kani)]
mod verification;

/// Re-export of the pinned [`fixed`] crate behind the `fixed` feature.
///
/// Fixed-point schema fields must use this exact crate instance: `pinapod`
/// pins `fixed =1.30.0`, and a `ZcField` implementation for a different
/// `fixed` build does not exist. Deriving schemas over `pina::fixed` types
/// removes the version-mismatch failure mode entirely.
#[cfg(feature = "fixed")]
pub use fixed;
/// Re-export all proc macros from `pina_macros` when the `derive` feature is
/// enabled.
#[cfg(feature = "derive")]
pub use pina_macros::*;
/// Re-export of the [`pinapod`] crate for advanced direct use.
///
/// Pina's audited zero-copy contract is the closed field grammar enforced by
/// [`account`], [`instruction`], and [`macro@event`]. Direct `PinaPod` derives and
/// manual trait implementations are outside that contract and must uphold
/// `PinaPod`'s complete safety invariants themselves.
pub use pinapod;
/// Derives a validated zero-copy companion for a native schema.
pub use pinapod::PinaPod;
/// Zero-copy access for compact (variable-length) types.
#[cfg(feature = "compact")]
pub use pinapod::PinaPodCompact;
/// Error type for `PinaPod` validation failures.
pub use pinapod::PinaPodError;
/// Zero-copy access for fixed-size types.
pub use pinapod::PinaPodFixed;
/// Atomic update contract implemented by generated compact patches.
#[cfg(feature = "compact")]
pub use pinapod::PinaPodPatch;
/// Fixed-capacity UTF-8 string schema used by `PinaPod` derives.
pub use pinapod::String;
/// Bounded vector schema used by fixed and compact `PinaPod` derives.
pub use pinapod::Vec;
/// Marker trait for types that can be safely cast from any byte pattern.
pub use pinapod::ZcElem;
/// Maps a native Rust type to its pod (zero-copy) companion and byte size.
pub use pinapod::ZcField;
/// Validation trait for stored (pod) types.
pub use pinapod::ZcValidate;
/// Alignment-one storage for an IEEE-754 `f32` schema field.
///
/// A schema declares `f32`; Pina maps it to this pod so the field is stored
/// as its little-endian bit pattern, and the generated accessors still take
/// and return the native float.
#[cfg(feature = "floats")]
pub use pinapod::pod::PodF32;
/// Alignment-one storage for an IEEE-754 `f64` schema field.
///
/// A schema declares `f64`; Pina maps it to this pod so the field is stored
/// as its little-endian bit pattern, and the generated accessors still take
/// and return the native float.
#[cfg(feature = "floats")]
pub use pinapod::pod::PodF64;
/// Re-export of the [`pinocchio`] crate for low-level Solana program
/// primitives.
pub use pinocchio;
/// A Solana account as seen by the runtime during instruction execution.
pub use pinocchio::AccountView;
/// A 32-byte Solana public key / address.
pub use pinocchio::Address;
/// The result type returned by Solana program entrypoints and instruction
/// handlers.
pub use pinocchio::ProgramResult;
/// An immutable borrow guard for account-backed data.
pub use pinocchio::account::Ref;
/// A mutable borrow guard for account-backed data.
pub use pinocchio::account::RefMut;
/// Number of bytes in a Solana address (32).
pub use pinocchio::address::ADDRESS_BYTES;
/// Maximum length in bytes of a single PDA seed.
pub use pinocchio::address::MAX_SEED_LEN;
/// Maximum number of seeds allowed when deriving a PDA.
pub use pinocchio::address::MAX_SEEDS;
/// A single seed byte slice used in PDA signing.
pub use pinocchio::cpi::Seed;
/// A set of seeds that identifies a PDA signer for CPI calls.
pub use pinocchio::cpi::Signer;
/// The Solana program entrypoint attribute.
pub use pinocchio::entrypoint;
/// Error type returned by Solana programs.
pub use pinocchio::error::ProgramError;
/// An account reference passed as part of an instruction.
pub use pinocchio::instruction::InstructionAccount;
/// A view of a cross-program invocation instruction.
pub use pinocchio::instruction::InstructionView;
/// Macro for declaring a Solana program entrypoint.
pub use pinocchio::program_entrypoint;
/// Solana sysvar access utilities.
pub use pinocchio::sysvars;
/// Re-export of `pinocchio_associated_token_account` for ATA operations.
#[cfg(feature = "token")]
pub use pinocchio_associated_token_account as associated_token_account;
/// Re-export of `pinocchio_memo` for memo program helpers.
#[cfg(feature = "memo")]
pub use pinocchio_memo as memo;
/// Re-export of `pinocchio_system` for system program CPI helpers.
pub use pinocchio_system as system;
/// `PinaPod`'s alignment-one storage primitives (`PodBool`, `PodU16`,
/// `PodU64`, etc.). Prefer native field types in schemas; the `PinaPod`
/// derive selects these representation types for the generated `*Zc` view.
pub use pod::*;
/// Macro for creating a compile-time [`Address`] from a base-58 string
/// literal.
pub use solana_address::address;
/// Macro for declaring a program ID constant with associated `ID` and `id()`
/// items.
pub use solana_address::declare_id;
/// Re-export of `solana_program_log` for on-chain logging utilities.
#[cfg(feature = "logs")]
pub use solana_program_log;
/// A logger instance for formatting on-chain log messages.
#[cfg(feature = "logs")]
pub use solana_program_log::Logger;
/// Logs the current compute unit usage to the Solana runtime.
#[cfg(feature = "logs")]
pub use solana_program_log::log_cu_usage;

/// CPI helpers for account creation, PDA allocation, and account closure.
pub use crate::cpi::*;
/// Built-in framework error types.
pub use crate::error::*;
/// On-chain event emission to the transaction log.
pub use crate::event::*;
/// Version envelopes and generated account-migration runtime contracts.
pub use crate::migration::*;
/// PDA (Program Derived Address) derivation and verification functions.
pub use crate::pda::*;
/// Core traits for account validation, deserialization, and instruction
/// processing.
pub use crate::traits::*;
/// Utility functions for instruction parsing, assertions, and token address
/// derivation.
pub use crate::utils::*;

/// Static replacement message for formatted diagnostics when `verbose-logs`
/// is disabled.
///
/// Keeps the failure path identical across feature sets without linking
/// `core::fmt`.
pub const DETAIL_POINTER_MESSAGE: &str =
	"pina: operation failed; enable the `verbose-logs` feature for details";

/// Whether formatted diagnostics are compiled in.
///
/// Exported macros read this constant instead of testing
/// `cfg(feature = "verbose-logs")` themselves, because a `cfg` written inside
/// an exported macro is evaluated against the *calling* crate's features. The
/// caller usually does not declare `verbose-logs`, which would make the
/// condition fail to compile rather than simply evaluate to `false`.
#[cfg(feature = "verbose-logs")]
pub const VERBOSE_LOGS_ENABLED: bool = true;

/// See [`VERBOSE_LOGS_ENABLED`] for why this is a constant.
#[cfg(not(feature = "verbose-logs"))]
pub const VERBOSE_LOGS_ENABLED: bool = false;

/// Sets up a `no_std` Solana program entrypoint.
///
/// This macro wires up the BPF entrypoint, disables the default allocator, and
/// installs a minimal panic handler. The entry function receives:
///
/// ```ignore
/// fn process_instruction(
///     program_id: &Address,
///     accounts: &mut [AccountView],
///     data: &[u8],
/// ) -> ProgramResult
/// ```
///
/// Usage:
///
/// ```ignore
/// nostd_entrypoint!(process_instruction);
/// ```
///
/// An optional second argument overrides the maximum number of transaction
/// accounts (defaults to `pinocchio::MAX_TX_ACCOUNTS`).
///
/// The allocator is denied rather than merely unused: any dynamic allocation
/// aborts at runtime. Programs that need the heap opt in explicitly with
/// [`nostd_entrypoint_alloc!`], which is identical apart from the global
/// allocator it installs.
#[macro_export]
macro_rules! nostd_entrypoint {
	($process_instruction:expr) => {
		$crate::nostd_entrypoint!($process_instruction, { $crate::pinocchio::MAX_TX_ACCOUNTS });
	};
	($process_instruction:expr, $maximum:expr) => {
		$crate::pinocchio::program_entrypoint!($process_instruction, $maximum);
		$crate::pinocchio::no_allocator!();
		$crate::pinocchio::nostd_panic_handler!();
	};
}

/// Sets up a `no_std` Solana program entrypoint with a heap allocator.
///
/// This is [`nostd_entrypoint!`] with one difference: it installs
/// `pinocchio::default_allocator!` — a `BumpAllocator` over the runtime's heap
/// region — instead of `no_allocator!`, so `Box`, `Vec`, and the rest of
/// `alloc` work. Everything else is unchanged, and the entry function keeps
/// the same signature:
///
/// ```ignore
/// fn process_instruction(
///     program_id: &Address,
///     accounts: &mut [AccountView],
///     data: &[u8],
/// ) -> ProgramResult
/// ```
///
/// Opt in only when the program needs the heap; [`nostd_entrypoint!`] keeps
/// allocation impossible at compile time and remains the default for every
/// other program. A heap-allocating crate declares the standard allocator
/// itself:
///
/// ```ignore
/// extern crate alloc;
///
/// use alloc::boxed::Box;
///
/// nostd_entrypoint_alloc!(process_instruction);
/// ```
///
/// An optional second argument overrides the maximum number of transaction
/// accounts (defaults to `pinocchio::MAX_TX_ACCOUNTS`).
///
/// # Costs of opting in
///
/// These are the failure modes that `no_allocator!` makes impossible, and they
/// are the reason this is opt-in rather than the default:
///
/// | Cost | Detail |
/// | --- | --- |
/// | Heap budget | The runtime grants a 32 KiB region by default, charged at 0 CU. A program that needs more requires the caller to send `ComputeBudgetInstruction::request_heap_frame`: the request must be a multiple of 1024 and at most 256 KiB, and each additional 32 KiB page costs 8 CU (`DEFAULT_HEAP_COST`). The allocator keeps its current position in the first word of that region, so the usable bytes are the granted frame minus one `usize`; a program that requests a frame sized for exactly its payload still aborts. |
/// | Monotonic allocator | `BumpAllocator` only moves a pointer forward. Its `dealloc` is a no-op, so memory is never reclaimed within a transaction and a loop that allocates grows the heap for the whole instruction. |
/// | Allocation failure aborts | An exhausted heap returns a null pointer, which `alloc` turns into an abort — not a `ProgramError`. Size the request against the worst case; there is no recoverable out-of-memory path. |
/// | Client-side dependency | A program that needs more than 32 KiB shifts a runtime requirement onto every caller. Only the transaction that sends the heap-frame request gets the larger region, so a caller that forgets it fails at runtime while the same instruction succeeds elsewhere. |
/// | Transaction-scoped | The heap is not persistent storage: nothing a program allocates is visible to another transaction, so anything that must outlive the instruction belongs in account data. |
///
/// The heap is a transaction-scoped arena, not a general-purpose allocator:
/// prefer the zero-copy account types for anything stored in an account and
/// reserve the heap for transient work that outgrows the stack.
#[macro_export]
macro_rules! nostd_entrypoint_alloc {
	($process_instruction:expr) => {
		$crate::nostd_entrypoint_alloc!($process_instruction, {
			$crate::pinocchio::MAX_TX_ACCOUNTS
		});
	};
	($process_instruction:expr, $maximum:expr) => {
		$crate::pinocchio::program_entrypoint!($process_instruction, $maximum);
		$crate::pinocchio::default_allocator!();
		$crate::pinocchio::nostd_panic_handler!();
	};
}

/// Logs a failure message with optional formatted detail.
///
/// The static `message` is always logged. The formatted `detail` is logged
/// only with the `verbose-logs` feature, so the default build keeps a
/// descriptive failure trail without linking `core::fmt`.
///
/// ```ignore
/// log_failure!("address is missing a required signature", "address: {}", addr);
/// ```
///
/// When the `logs` feature is disabled this is a no-op.
#[cfg(feature = "logs")]
#[macro_export]
macro_rules! log_failure {
	($message:literal) => {{
		$crate::solana_program_log::logger::log_message($message.as_bytes());
	}};
	($message:literal, $($arg:tt)*) => {{
		$crate::solana_program_log::logger::log_message($message.as_bytes());
		if $crate::VERBOSE_LOGS_ENABLED {
			$crate::solana_program_log::log!($($arg)*);
		} else {
			let _ = ($($arg)*);
		}
	}};
}

/// No-op variant of [`log_failure!`] when `logs` is disabled.
///
/// Arguments are still evaluated so call sites keep identical semantics.
#[cfg(not(feature = "logs"))]
#[macro_export]
macro_rules! log_failure {
	($($arg:tt)*) => {{
		let _ = ($($arg)*);
	}};
}

/// Logs a message to the Solana runtime.
///
/// Supports two forms:
/// - `log!("simple string literal")` — works in all crates
/// - `log!("format: {}", value)` — format-arg form, requires `verbose-logs`
///
/// # Limitations
///
/// The format-arg form (`log!("format: {}", value)`) only works inside pina
/// and crates that depend on `solana-program-log` directly, because the
/// proc macro generates absolute paths to `solana_program_log::log!`.
/// Crates that only re-export pina without their own `solana-program-log`
/// dependency will fail to resolve the macro path for the format arm.
///
/// Without the `verbose-logs` feature the format-arg form logs a static
/// message instead. Formatting pulls in `core::fmt`, which is the largest
/// avoidable contributor to deployed program size; use [`log_verbose!`] for
/// messages that are worth that cost in production builds.
///
/// When the `logs` feature is disabled this is a no-op that compiles to
/// nothing.
#[cfg(feature = "logs")]
#[macro_export]
macro_rules! log {
	($msg:literal) => {
		$crate::solana_program_log::logger::log_message($msg.as_bytes())
	};
	// Without `verbose-logs` the format arm logs a static message so
	// `core::fmt` is not linked. Arguments are still evaluated so call sites
	// keep identical semantics and bindings stay used in every feature
	// configuration.
	($($arg:tt)*) => {{
		if $crate::VERBOSE_LOGS_ENABLED {
			$crate::solana_program_log::log!($($arg)*);
		} else {
			let _ = ($($arg)*);
			$crate::solana_program_log::logger::log_message(
				$crate::DETAIL_POINTER_MESSAGE.as_bytes(),
			);
		}
	}};
}

/// Logs a formatted message. Always formats, even without `verbose-logs`.
///
/// Use for developer-facing messages that are worth linking `core::fmt`.
/// Prefer [`log!`] on failure paths inside deployed programs.
///
/// [`crate::assert`] is the one framework failure path that does *not*
/// use this macro in default builds: it writes its raw message straight to
/// the log instead of building a formatted one, so a program that only calls
/// `assert` keeps its failure diagnostics on the cheaper path.
#[cfg(feature = "verbose-logs")]
#[macro_export]
macro_rules! log_verbose {
	($($arg:tt)*) => {
		$crate::solana_program_log::log!($($arg)*);
	};
}

/// Logs a formatted message even when `verbose-logs` is disabled.
///
/// The contract is "always formats", so this arm deliberately keeps the
/// formatted log rather than becoming a no-op: a call site that chose
/// [`log_verbose!`] over [`log!`] wants the message in every build and pays
/// for `core::fmt` in every build. Arguments are evaluated either way so call
/// sites keep identical semantics.
///
/// When the `logs` feature is off there is no logger to write to, and the
/// macro becomes a no-op that still evaluates its arguments.
#[cfg(all(not(feature = "verbose-logs"), feature = "logs"))]
#[macro_export]
macro_rules! log_verbose {
	($($arg:tt)*) => {
		$crate::solana_program_log::log!($($arg)*);
	};
}

/// No-op variant used when logging is compiled out entirely.
#[cfg(not(feature = "logs"))]
#[macro_export]
macro_rules! log_verbose {
	($($arg:tt)*) => {{
		let _ = ($($arg)*);
	}};
}

#[cfg(not(feature = "logs"))]
#[macro_export]
macro_rules! log {
	($($arg:tt)*) => {{
		let _ = ($($arg)*);
	}};
}

/// Re-exports commonly used traits and helpers for instruction modules.
///
/// `use pina::prelude::*;` is the recommended import style inside on-chain
/// modules that want validation traits without long import lists.
///
/// <!-- {=pinaMdtManagedDocNote|trim|linePrefix:"/// ":true} -->
/// This section is synchronized by `mdt` from `api-docs.t.md`.<!-- {/pinaMdtManagedDocNote} -->
pub mod prelude {
	#[cfg(feature = "logs")]
	pub use solana_program_log::Logger;

	pub use crate::traits::*;
}
