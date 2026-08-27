---
pina_cli:
  bump: minor
  type: feat
pina_skill:
  bump: minor
  type: feat
---

# Add a project-aware daily CLI workflow

Add `Pina.toml` discovery plus focused `pina build` and `pina generate` commands. New projects record their program and selected client ecosystems locally, while existing Cargo packages remain discoverable without configuration.

`pina build` streams a structured SBF build with explicit feature forwarding, respects Cargo's target directory, stages both outputs, serializes concurrent publication, and atomically replaces each `<target>/deploy/<library>.so` and `<target>/idl/<library>.json` destination. Project-local paths fail closed on traversal and symbolic-link escapes. `pina generate` refreshes the same IDL and renders only the requested Rust, TypeScript, or Dart clients; Rust-only generation does not require Node.js. The existing `pina codama generate` surface remains compatible for repository-wide workflows.

Renderer processes retain bounded, control-escaped failure diagnostics even when they exit before consuming the complete driver input, and build publication locks release explicitly across supported operating systems.
