---
pina_cli:
  bump: minor
  type: feat
pina_cli_npm:
  bump: minor
  type: feat
---

# Add a project-aware daily CLI workflow

Add `Pina.toml` discovery plus focused `pina build` and `pina generate` commands. New projects record their program and selected client ecosystems locally, while existing Cargo packages remain discoverable without configuration.

`pina build` streams a structured SBF build with explicit feature forwarding, respects Cargo's target directory, and atomically publishes `<target>/deploy/<library>.so` with `<target>/idl/<library>.json`. `pina generate` refreshes the same IDL and renders only the requested Rust, TypeScript, or Dart clients; Rust-only generation does not require Node.js. The existing `pina codama generate` surface remains compatible for repository-wide workflows.
