---
pina: patch
pina_cli: patch
pina_macros: patch
pina_sdk_ids: patch
---

Documentation and release-quality updates across crates:

- Standardized crate README badges to explicitly show crates.io and docs.rs links with current versions.
- Added a dedicated `pina_sdk_ids` crate README with crates.io/docs.rs badges and switched the crate manifest to use it.
- Added workspace coverage tooling with `coverage:all` and a CI `coverage` workflow that produces an LCOV artifact and uploads to Codecov.
