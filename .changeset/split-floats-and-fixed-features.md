---
pina: major
pina_macros: feat
pina_abi: none
pina_cli: none
---

# split `floats` and `fixed`, and source pods from pinapod

The fractional-field support is now two independent features. `floats` enables IEEE-754 `f32` and `f64` schema fields, and `fixed` enables fixed-point `FixedI*<Frac>` and `FixedU*<Frac>` fields from the pinned `fixed` crate. Previously one feature covered both, which forced every float program to compile `fixed` and `typenum` for types it never used.

`PodF32` and `PodF64` now come from `pinapod` 0.3.3 rather than a Pina-local definition, and `pinapod` implements the schema-field mapping for the `f32` and `f64` primitives directly. Pina re-exports both pods, so `pina::PodF32`, `pina::PodF64`, and `pina::pod::PodF32` keep working unchanged.

That upstream mapping removes two workarounds this crate no longer needs. The macros previously rewrote float field types to their pod spellings before the `PinaPod` derive expanded, and skipped the identity proof for float vectors, because the generated `Vec<T, N>` alias normalizes through a trait implementation that did not exist for the float primitives. Fields now keep their declared spelling all the way to the derive. The same mapping also fixes optional float values in generated compact patches: `patch.maybe_bias(Some(0.5))` and `patch.maybe_bias(None)` both work without importing a pod, where the pod spelling was previously required.

The feature split is a breaking change for programs that enabled `floats` for fixed-point fields only, or the reverse. Add the feature that matches the stored field type. `PodF32` and `PodF64` are additionally distinct types from the Pina-local definitions they replace, so code naming `pina::PodF32` in a type position now binds to pinapod's struct. Wire formats, validation order, and error variants are unchanged, and both families still store the complete bit pattern of their backing little-endian integer.
