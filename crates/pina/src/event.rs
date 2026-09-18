//! On-chain event emission.
//!
//! Solana's runtime base64-encodes each slice passed to `sol_log_data` and
//! writes it after the stable `Program data: ` prefix. Pina's generated Rust,
//! TypeScript, and Dart decoders read exactly that line, so an event reaches
//! an indexer only when the program routes its record through
//! [`emit_event`]. The `#[event]` macro generates an `emit` associated function
//! that builds a validated record and calls this function.

/// Largest event record the generated `emit` helper can build on the SBF
/// stack.
///
/// The generated `emit` function materializes the complete record in one
/// frame (`let mut record = [0u8; Self::SIZE];`) and then calls into the
/// runtime, so the record occupies the frame for the whole emission. The SBF
/// runtime allows 4 KiB of stack per frame, and the call itself needs room for
/// the frame it pushes: this bound leaves 512 bytes of that budget for the
/// callee and the caller's own frame. Keeping the record below the limit
/// means an oversized `#[event]` schema fails the build instead of exhausting
/// the stack at runtime, where the failure is a silent frame overflow.
///
/// The bound follows [`crate::MAX_MIGRATION_WORKSPACE`], which applies the
/// same rule to the historical-normalization workspace: reserve part of the
/// frame budget for the code the frame calls into, and turn the rest into a
/// compile-time limit. It is larger because a record's frame is a single flat
/// byte array rather than nested transition workspaces.
pub const MAX_EVENT_RECORD_BYTES: usize = 4096 - 512;

/// Emit one complete event record to the transaction log.
///
/// `record` must be the full `[discriminator][schema version][payload]`
/// envelope rather than the payload alone. The generated `#[event]` `emit`
/// function builds that envelope; reach for this function directly only when
/// forwarding bytes that were already built and validated elsewhere.
///
/// The record is passed as a single slice because the runtime writes one
/// base64 field per slice and joins them with a space. Generated decoders
/// base64-decode the entire `Program data: ` remainder, so splitting one
/// record across several slices makes it undecodable.
///
/// This function writes nothing when the `logs` feature is disabled. An event
/// is a log record, so a program built without `logs` cannot emit one; the
/// generated `emit` helper reports that configuration as an error instead of
/// dropping the record silently.
///
/// # Errors
///
/// Returns [`crate::ProgramError::UnsupportedSysvar`] when the `logs` feature
/// is disabled, because there is no log to write to.
#[cfg(feature = "logs")]
#[inline(always)]
pub fn emit_event(record: &[u8]) -> Result<(), crate::ProgramError> {
	solana_program_log::log_data(&[record]);

	Ok(())
}

/// Emit one complete event record to the transaction log.
///
/// # Errors
///
/// Always fails with [`crate::ProgramError::UnsupportedSysvar`] when the
/// `logs` feature is disabled. Enable the `logs` feature on `pina` to emit
/// events.
#[cfg(not(feature = "logs"))]
#[inline(always)]
pub fn emit_event(record: &[u8]) -> Result<(), crate::ProgramError> {
	let _ = record;

	Err(crate::ProgramError::UnsupportedSysvar)
}
