---
core: major
pina_root: none
---

# Make generated clients safe to customize and regenerate

Adds explicit `auto`, `create`, `update`, and `overwrite` destination modes plus scaffold controls to project-aware CPI, Rust, TypeScript, and Dart/Flutter generation. Global `[clients]` defaults can be overridden for each target, including its output directory, while CLI flags support one-off lifecycle and source-only generation.

Update mode replaces only renderer-owned generated sources and preserves user-edited manifests and entrypoints. Overwrite mode is an explicit clean sweep with working-tree and symbolic-link safety checks. The native Rust and CPI renderers and the Codama CPI visitor expose the same controls.

This is a breaking `core` API change because the public Rust renderer and CLI option structs gain required generation-policy fields.
