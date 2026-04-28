---
pina_cli: minor
pina_codama_renderer: minor
---

Resolve explicit PDA-derived account defaults in generated clients. Codama lowering now preserves deterministic PDA default metadata from account seeds, and the Rust renderer emits builders that derive those defaults while keeping signer and writable expectations explicit.
