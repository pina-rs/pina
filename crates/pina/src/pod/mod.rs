//! `PinaPod`'s zero-copy storage types.
//!
//! Pina's `#[account]`, `#[instruction]`, and `#[event]` macros accept only
//! their documented closed field grammar. Some storage types re-exported here
//! are useful for direct `PinaPod` integrations but are deliberately rejected by
//! those macros. Direct derives and manual `PinaPodFixed` implementations are
//! advanced APIs outside Pina's audited macro-generated contract.

pub use pinapod::pod::*;
