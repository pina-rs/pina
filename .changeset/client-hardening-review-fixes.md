---
pina_cli: fix
pina_codama_renderer: fix
---

# Preserve JS event barrels and maximal-version tests

The JavaScript event hardening read `events/index.ts` with `unwrap_or_default`, so an unwritable or invalid-UTF-8 barrel was treated as absent and the following write replaced it with the lone `./logs` export, silently dropping the other event exports. Only a missing barrel now counts as fresh generation; every other read failure surfaces as `CodamaError::HardenJavaScript` before anything is overwritten.

The generated Rust projection tests built their future-version fixture with a saturating `version + 1`, which at a type's maximum version emitted the current version (or an overflowing literal for `u8`/`u16`/`u32` widths), so the generated crate failed to compile or its `future_versions_fail_closed` test failed. Events at the maximum representable version now emit `maximal_version_decodes_as_current` instead, matching `try_from_bytes`' classification of the true maximum as current.
