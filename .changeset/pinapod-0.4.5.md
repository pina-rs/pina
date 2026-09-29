---
pina: fix
# The workspace manifest and lock files the dependency bump edits belong to
# `pina_root`, which is unpublished, so the coverage is recorded without a
# bump.
pina_root: none
---

# Upgrade pinapod to 0.4.5

Bump the workspace `pinapod` dependency from 0.4.4 to 0.4.5 and refresh the root, `pina_fuzz`, and `pina_fuzz/fuzz` lock files. No pina source changes are required — pina re-exports and type-mentions the pinapod API, and 0.4.5 adds no public API and changes no wire format.

The release closes a declaration-versus-layout divergence. A compact field declared as an array of explicit-prefix containers, such as `[PodString<8, 3>; 4]`, previously skipped the prefix-width check: validation descended through generic arguments but never through array elements, so the element-wise mapping silently re-encoded the field with the default one-byte prefix and the account compiled with a different wire layout than the schema declared. The runtime still validated whatever bytes the rewritten type produced, so this was never memory-unsound, but a client generated from the declared schema would disagree with the on-chain bytes. The declaration is now a compile error, and the descent covers `Option<[PodVec<u8, 8, 0>; 2]>` and parenthesized spellings while explicit widths inside an array remain accepted.

The same release trims compact-enum validation and documents the generated surface. The commit-entry check no longer interprets a variant's fixed payload, because relocation consumes only the payload's compile-time range; the semantic half stays in `validate` and every value-exposing boundary. The dead tag-size guard after the storage-length check is gone, compact-enum support helpers carry `#[inline(always)]` like their compact-struct counterparts, the compact `Ref` accessors and `Mut` setters carry `#[inline]`, staged vector setters route through `ZcValidate::validate_slice`, and the generated compact `Ref`/`Mut`/`Patch` surface now emits rustdoc — the one visible change in `tests/expand/pda.expanded.rs`, alongside the setter's `validate_slice` call.

Measured through this repository's instruction compute-unit harness against the 0.4.4 pina pins today, all 100 example instruction cases are compute-unit identical and 23 of 28 SBF artifacts are byte-identical; the five that differ in content measure the same. Wire format, validation order, error variants, and error precedence are unchanged.
