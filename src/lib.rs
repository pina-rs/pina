//! Internal cross-crate test harness for the Pina workspace.
//!
//! This crate is the workspace root itself. It is never published; its only
//! job is to host tests that need more than one Pina crate at a time,
//! mirroring the workspace layout in submodules so every crate is exercised
//! through its public API exactly as a downstream consumer would.
//!
//! # Layout
//!
//! - [`pina_macros`] — proc-macro contract tests: real macro invocations,
//!   compile-pass fixtures, and compile-failure UI tests.

pub mod pina_macros;
