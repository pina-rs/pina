---
pina:
  bump: none
  type: docs
pina_cli:
  bump: none
  type: docs
pina_macros:
  bump: none
  type: docs
pina_sdk_ids:
  bump: none
  type: docs
pina_codama_renderer:
  bump: none
  type: docs
---

# Unslop and cross-check documentation

Polish and verify the documentation corpus:

- Correct the license badge to Apache-2.0 (the workspace license) across the root, crate, and CLI readmes, and fix the broken `license` link in the root readme.
- Re-sync the compiled-in `pina docs` templates with the mdt providers so the CLI serves current content (Dart clients, npm packages, feature-selection and instruction-authoring tips).
- Fix stale references: the nonexistent `test_utils_solana` module, a duplicate `crates/pina` entry and incomplete Pod type list in the workspace architecture guide, and a duplicate `crates/pina` heading in the crates-and-features book page.
- Tighten example readmes: cut filler ("built with Pina", "reference example"), correct inaccurate claims (escrow Token-2022 validation, staking ATA idempotency, prop-amm CPI builder naming, float bit-pattern storage, `#[event(discriminator)]`), add missing e2e-test notes, and align heading and code-fence style.
- Note the authoritative `security:dylint` task in the lints readme and drop duplicated renderer description text.
