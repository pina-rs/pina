---
pina_test: feat
---

# Rejoin pina_test with train-anchored ranges

Drop every exact (`=`) version requirement declared by this repository. `pina_test` rejoins the main workspace together with the 21 `examples/*/tests/surfpool` packages, and its dependency set becomes tilde train-anchors on the Agave 4.1 stack that Surfpool 1.5 ships on. Tildes are required because `pina init` generates downstream Surfpool test packages without a lockfile, and Agave keeps republishing interface crates onto newer serialization stacks mid-train — an unanchored resolve mixes `wincode` 0.5 and 0.6 into a graph that cannot compile. The workspace aligns its host test stack by moving `mollusk-svm` from 0.15 (Agave 4.2) to 0.14 (Agave 4.1), and `pina_test` now re-exports `Rent` so generated tests do not declare `solana-rent` themselves.

Also: the standalone pina_test publish step is gone from `publish.yml` (monochange publishes it like every other crate, with a longer publish dry-run timeout), cargo-deny gains the Surfpool stack's licenses, `program_under_test` path dependencies gain explicit versions, the Surfpool advisory exceptions move into the workspace audit, and `semver-checks` skips `pina_test` until its rebalanced release becomes the published baseline.
