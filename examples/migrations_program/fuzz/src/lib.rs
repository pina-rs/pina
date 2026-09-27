//! Rlib re-export of the migrations program for the fuzz targets.
//!
//! The program is a cdylib only, so `crates/pina_fuzz` cannot depend on it
//! directly; it source-includes the program here instead, from inside the
//! program's own tree where migration-aware codegen resolves
//! `migrations/manifest.json`, and re-exports the real types.

#[path = "../../src/lib.rs"]
pub mod program;

pub use program::*;
