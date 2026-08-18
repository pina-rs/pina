//! Fuzz harnesses for pina — targeting `AccountDeserialize::try_from_bytes`
//! and `parse_instruction`.
//!
//! This crate is **not** a workspace member. It is built separately with
//! `cargo fuzz` and depends on `pina` and the workspace example programs
//! for real account/instruction types.

#![allow(clippy::all)]
