---
pina: docs
pina_abi: docs
pina_cli: docs
pina_cli_renderer: docs
pina_codama_renderer: docs
pina_cpi_renderer: docs
pina_profile: docs
pina_root: docs
---

# Enforce docstring coverage with the missing_docs lint

The root `[workspace.lints.rust]` table now sets `missing_docs = "warn"`, which CI escalates through `-D warnings`: every public item reachable from a crate root that inherits the workspace lints must carry a doc comment, and the crate itself must have a crate-level doc comment. Three undocumented items in `pina` (the `AccountMigrationOutcome::AlreadyCurrent.version` field and the `token`/`token_2022` `state` modules) are documented to satisfy the gate.

Exclusions follow the surfaces the docstring effort should not reach. Generated Codama clients never inherit the workspace lints, so `codama/**` is excluded by default with no generated file touched. The security lessons, the example programs, and the six crates whose public surface is still being documented (`pina_cli`, `pina_abi`, and the renderers plus `pina_profile`) carry a crate-level `#![allow(missing_docs)]` so the gate lands green today while those surfaces are documented in follow-ups; the allow is visible in each crate root rather than hidden in configuration.

A new `pina_root` integration test pins the gate's contract by compiling fixtures with the pinned toolchain under `-D missing_docs`: an undocumented public item is rejected, the same surface with crate and item docs compiles, and the crate-level allow opt-out the excluded crates rely on suppresses the check.
