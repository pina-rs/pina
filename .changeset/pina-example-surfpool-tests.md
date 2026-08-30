---
pina_test: feat
pina_cli: feat
---

# Exercise every example on real runtimes

Replace the deploy-only filler suites with behavior tests that run each compiled SBF artifact through an isolated Surfpool instance: on-chain state transitions, runtime error codes, signature and ownership guards, PDA math, SPL token provisioning, and an escrow Make/Take round trip. The support surface moves into the `pina_test` crate, and generated test manifests inherit the pinned Surfpool compatibility graph from it instead of pinning Mollusk in every program manifest.
