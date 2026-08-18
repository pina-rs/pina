//! Zeropod's zero-copy storage types.
//!
//! Application schemas should use native Rust field types and derive
//! [`zeropod::ZeroPod`]. The derive generates a separate zero-copy view whose
//! fields use these storage types. Byte slices are converted to validated views
//! through [`zeropod::ZeroPodFixed`], so Pina does not expose an independent
//! raw-casting API.

pub use zeropod::pod::*;
