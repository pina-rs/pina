//! On-chain event emission.
//!
//! Solana's runtime base64-encodes each slice passed to `sol_log_data` and
//! writes it after the stable `Program data: ` prefix. Pina's generated Rust,
//! TypeScript, and Dart decoders read exactly that line, so an event reaches
//! an indexer only when the program routes its record through
//! [`emit_event`]. The `#[event]` macro generates an `emit` associated function
//! that builds a validated record and calls this function.

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
/// When the `logs` feature is disabled this is a no-op.
#[cfg(feature = "logs")]
#[inline(always)]
pub fn emit_event(record: &[u8]) {
	crate::solana_program_log::log_data(&[record]);
}

/// No-op variant of [`emit_event`] when `logs` is disabled.
#[cfg(not(feature = "logs"))]
#[inline(always)]
pub fn emit_event(record: &[u8]) {
	let _ = record;
}
