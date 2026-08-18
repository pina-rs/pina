//! Zeropod's zero-copy storage types.
//!
//! Pina's `#[account]`, `#[instruction]`, and `#[event]` macros accept only
//! their documented closed field grammar. Some storage types re-exported here
//! are useful for direct zeropod integrations but are deliberately rejected by
//! those macros. Direct derives and manual `ZeroPodFixed` implementations are
//! advanced APIs outside Pina's audited macro-generated contract.

pub use zeropod::pod::*;
