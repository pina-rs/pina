---
default: major
---

Unify all crate and package versions under a single `[workspace.package] version` field. All publishable crates (`pina`, `pina_macros`, `pina_pod_primitives`, `pina_sdk_ids`, `pina_cli`, `pina_codama_renderer`) and the `codama-nodes-from-pina` JS package now share the same version, managed by a single `[package]` entry in `knope.toml`. This replaces the previous per-crate `[packages.*]` configuration and ensures all crates are released together with a single version bump.

Simplify the assets workflow to match the new unified release tag format and remove the per-crate version validation step. Update tooling versions for `cargo-llvm-cov`, `cargo-nextest`, `cargo-semver-checks`, and `mdt_cli`. Switch publishing from `cargo-workspaces` to `cargo publish --workspace`.
