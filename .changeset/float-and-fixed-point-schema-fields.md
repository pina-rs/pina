---
pina: feat
pina_abi: feat
pina_cli: feat
pina_macros: feat
---

# Add the `floats` feature for float and fixed-point schema fields

The new `floats` feature on `pina` lets `#[account]`, `#[instruction]`, and `#[event]` schemas accept `f32` and `f64` fields and fixed-point `FixedI*<Frac>`/`FixedU*<Frac>` fields. Float fields convert to and from their bit pattern under the hood through the new `pina::PodF32`/`pina::PodF64` alignment-one pods, so generated accessors take and return native floats exactly like `u32` fields do through `PodU32`. Fixed-point fields map to their backing little-endian integer pods, mirroring pinapod's `fixed` feature that Pina now forwards; Pina re-exports the exact pinned `fixed =1.30.0` instance as `pina::fixed`, so schemas never face a version-mismatch failure mode and users need no separate dependency.

Every fractional field is stored as the complete bit pattern of its backing little-endian integer, so all bit patterns are valid stored values, validation stays total, and zeroed payload reads as value zero with the typed discriminator still guarding account identity. Generated Codama clients describe float and fixed-point fields as their backing integers, keeping every Rust, TypeScript, and Dart client working without float codec support; `pina_abi` sizes the new types for migration layout planning, and the macros rewrite float primitives to their pods before the `PinaPod` derive expands, since `ZcField` cannot be implemented for the foreign float primitives. The measured build cost is two extra `no_std` crates (`fixed`, `typenum`) compiled once, with the `pina` rlib byte-identical for programs that do not use the feature.
