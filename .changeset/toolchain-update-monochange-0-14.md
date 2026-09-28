---
pina: none
pina_cli: none
pina_test: none
pina_macros: none
# Owns the root `Cargo.toml`, so the workspace dependency pins below are its
# change too. Unpublished, so it carries no bump.
pina_root: none
---

# Update the pinned toolchain and the classification gate

The devenv inputs moved to a newer `ifiokjr-nixpkgs` revision, which advances the pinned `monochange` to 0.14.0, `mdt` to 0.9.5, and the rest of the nix-provided toolchain. `monochange/actions` moves back to v0.9.4 in the same change, because the CLI and the action read the same report and must move together.

`monochange 0.14.0` emits its change-classification report as `schema_version` `"0.2"` — a string — with `compatibility_impact`, the renamed `unmodeled` impact, `decision.release_impact`, and label-driven skipping. The v0.9.2 action pinned here earlier parses the integer `schemaVersion` that 0.13 emitted and rejects the new shape with "monochange did not return a supported change-classification report"; v0.9.4 is the first release that parses it. The two pins are a pair: bumping either alone breaks the `changeset-policy` job on the next pull request whose classification actually runs, which is why the earlier action-only bump was reverted in #546. `monochange check` passes against the existing `monochange.toml` unchanged; 0.14.0's new `[changesets.classification].skip_labels` default of `["release"]` means the release pull request is no longer classified, matching the existing `[changesets.affected].skip_labels` entry.

# Repair the Kani toolchain link

`devenv.nix` linked Kani against `nightly-2025-11-21`, but the nixpkgs revision now ships Kani 0.68.0, whose release bundle was built against `nightly-2026-08-21` and resolves `librustc_driver` out of `$out/toolchain` at runtime. Every harness aborted before verification with `Library not loaded: librustc_driver-...dylib`. The link now uses 0.68.0's own toolchain, and the `kani (quick)` and `kani (compact layouts)` jobs pin 0.68.0 to match; `test:kani:quick` verifies 23 of 23 harnesses again.
