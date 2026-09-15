---
pina: feat
pina_abi: feat
pina_cli: feat
pina_macros: feat
---

# Add the `floats` feature for float schema fields

Fractional-field support arrives as two independent features. `floats` lets `#[account]`, `#[instruction]`, and `#[event]` schemas accept `f32` and `f64` fields, and the separate `fixed` feature accepts fixed-point `FixedI*<Frac>`/`FixedU*<Frac>` fields. Enable the feature that matches the stored field type: a schema using fixed-point types needs `fixed`, and `floats` alone will not compile it. Float fields convert to and from their bit pattern through `pina::PodF32`/`pina::PodF64`, which now come from `pinapod` rather than a Pina-local definition, so generated accessors take and return native floats exactly like `u32` fields do through `PodU32`. Fixed-point fields map to their backing little-endian integer pods through pinapod's `fixed` support; Pina re-exports the exact pinned `fixed =1.30.0` instance as `pina::fixed`, so schemas never face a version-mismatch failure mode and users need no separate dependency.

Every fractional field is stored as the complete bit pattern of its backing little-endian integer, so all bit patterns are valid stored values, validation stays total, and zeroed payload reads as value zero with the typed discriminator still guarding account identity. Generated Codama clients describe float and fixed-point fields as their backing integers, keeping every Rust, TypeScript, and Dart client working without float codec support, and `pina_abi` sizes the new types for migration layout planning. `pinapod` implements the schema-field mapping for the `f32` and `f64` primitives directly, which retires two macro workarounds: fields keep their declared spelling all the way to the `PinaPod` derive instead of being rewritten to pods first. The measured build cost is bounded by the feature a program enables — `fixed` alone pulls in `fixed` and `typenum`, and a program using neither compiles a `pina` rlib byte-identical to one built before the split.
