<!--
Sync Impact Report
===================
Version change: N/A → 1.0.0 (initial adoption)
Modified principles: N/A (initial)
Added sections:
  - Core Principles (5 principles)
  - Security & Deployment Constraints
  - Development Workflow
  - Governance
Removed sections: N/A
Templates requiring updates:
  - .specify/templates/plan-template.md ✅ compatible (Constitution Check
    section is generic; principles map cleanly)
  - .specify/templates/spec-template.md ✅ compatible (no constitution-
    specific references to update)
  - .specify/templates/tasks-template.md ✅ compatible (phase structure
    accommodates all principle-driven task types)
  - .specify/templates/checklist-template.md ✅ compatible (generic
    category structure)
Follow-up TODOs: none
-->

# Pina Constitution

## Core Principles

### I. no_std & Performance First

Every crate in the workspace MUST compile under `no_std` for on-chain deployment to the Solana SBF target (`bpfel-unknown-none`). Minimising compute unit consumption is a primary design goal; the framework exists specifically to eliminate the dependency bloat and overhead of `solana-program`.

- All library code MUST be `no_std`-compatible. Standard-library dependencies are permitted only behind feature gates used exclusively in tests or off-chain tooling.
- New abstractions MUST NOT introduce measurable CU overhead compared to hand-written pinocchio code, unless the overhead is explicitly documented and justified.
- Dependencies MUST default to `default-features = false`. Enabling additional features requires justification in a PR description.

**Rationale**: Pina's value proposition is performance parity with raw pinocchio while providing ergonomic account validation, discriminator management, and CPI helpers. Any regression undermines the framework's reason for existence.

### II. Safety Without Unsafe

`unsafe_code` is **denied** workspace-wide. All memory layout guarantees MUST be achieved through `bytemuck` Pod/Zeroable derives and `#[repr(C)]` annotations rather than raw pointer manipulation.

- Account data MUST be accessed through the validation trait chain (`assert_signer`, `assert_writable`, `assert_owner`, etc.) which returns `Result<&AccountInfo>` to enforce correctness at every step.
- Pod wrapper types (`PodBool`, `PodU16`, `PodU64`, etc.) MUST be used for alignment-safe fields in `#[repr(C)]` account structs.
- `Result::expect` is a disallowed method; use `unwrap_or_else` with an explicit panic message instead, to ensure meaningful error context.

**Rationale**: On-chain programs handle real financial assets. Undefined behaviour from unsafe code or unchecked account access can lead to exploitable vulnerabilities and loss of funds.

### III. Strict Code Quality

All code MUST pass the project's lint and format gates before merge. These gates are non-negotiable and MUST NOT be bypassed with `#[allow(...)]` attributes unless accompanied by a comment explaining why the suppression is necessary.

- Formatting is controlled by `dprint` (delegating to `rustfmt`, `nixfmt`, `shfmt`). Always run `fix:format` or `dprint fmt`, never `rustfmt` directly.
- Style rules: hard tabs, max width 100, one import per line (`imports_granularity = "Item"`), imports grouped by `StdExternalCrate`.
- `clippy::correctness` is **denied** (not warned). `clippy::wildcard_dependencies` is **denied**. `unstable_features` is **denied**.
- `cargo semver-checks` MUST pass for all publishable crates before release. Breaking changes MUST be reflected in a `major` changeset.

**Rationale**: Consistent formatting and strict lints catch entire classes of bugs at compile time and keep the codebase navigable as it grows.

### IV. Testing Discipline

All non-trivial logic MUST have corresponding tests. On-chain program logic MUST be tested via `mollusk-svm` simulation rather than deploying to a live cluster.

- Programs are tested as regular Rust libraries (without the `bpf-entrypoint` feature) against the mollusk SVM.
- `cargo nextest run` is the preferred test runner; standard `cargo test` is also acceptable.
- Snapshot tests use `cargo-insta`. Coverage is measured via `cargo-llvm-cov`.
- Custom lint rules are enforced via `dylint` (`cargo-dylint` + `dylint-link`).

**Rationale**: SVM-level simulation provides deterministic, fast test execution that closely mirrors on-chain behaviour without the flakiness and latency of network-based testing.

### V. Semantic Versioning & Changesets

Every publishable crate follows strict semantic versioning. Every pull request that modifies code in any crate (`crates/` or `examples/`) MUST include at least one changeset file in `.changeset/`.

- Version bumps and changelogs are managed by `knope` (bot workflow). Each crate has its own changelog and version.
- Change types: `major` (breaking), `minor` (new features), `patch` (bug fixes), `docs` (documentation), `note` (general notes).
- Package scopes: `pina`, `pina_macros`, `pina_sdk_ids`, `pina_token_2022_extensions`.
- Changesets MUST be highly detailed and MUST be formatted with `dprint fmt .changeset/* --allow-no-files` after creation.

**Rationale**: Downstream consumers depend on accurate semver signals to safely upgrade. Missing or incorrect version bumps cause silent breakage in production programs.

## Security & Deployment Constraints

- Programs MUST be compiled to the `bpfel-unknown-none` target using `sbpf-linker` for on-chain deployment. The `.cargo/config.toml` linker flags MUST NOT be modified without team review.
- The `bpf-entrypoint` feature gate MUST separate the on-chain entrypoint from library code used in tests.
- Every account/instruction/event type MUST use the discriminator system (auto-injected by `#[account]`/`#[instruction]`/`#[event]` macros) to prevent type confusion attacks.
- All account operations MUST use the validation trait chain. Direct field access on raw `AccountInfo` without prior validation is prohibited in framework-level code.
- Secrets, private keys, and environment-specific configuration MUST NOT be committed to the repository.

## Development Workflow

- **Environment**: Uses `devenv` (Nix-based) for reproducible development environments. All tooling versions are pinned: Cargo binaries via `cargo-run-bin` (versions in `[workspace.metadata.bin]` in root `Cargo.toml`), external binaries via `eget`.
- **Rust toolchain**: Nightly (`nightly-2025-11-20`), edition 2024, MSRV 1.86.0.
- **Build verification**: `cargo build --all-features` MUST succeed. The escrow example MUST build for SBF via `cargo build-escrow-program`.
- **Pre-merge gates**: `lint:all` (clippy + format check), `cargo nextest run`, `cargo semver-checks`.
- **Release flow**: `knope document-change` to create changesets, `knope release` to bump versions and update changelogs, `knope publish` / `cargo workspaces publish --from-git` to publish.
- **Cargo aliases** (defined in `.cargo/config.toml`): `cargo dylint`, `cargo insta`, `cargo llvm-cov`, `cargo nextest`, `cargo semver-checks`, `cargo workspaces`.

## Governance

This constitution is the authoritative source of non-negotiable project rules. It supersedes ad-hoc decisions, informal conventions, and individual preferences.

- **Amendments**: Any change to this constitution MUST be documented in a pull request with a clear rationale. The constitution version MUST be incremented following semantic versioning (MAJOR for principle removals/redefinitions, MINOR for additions/expansions, PATCH for clarifications/typos).
- **Compliance review**: All pull requests and code reviews MUST verify compliance with the principles defined here. Reviewers SHOULD reference the specific principle number (e.g., "Principle II") when flagging violations.
- **Complexity justification**: Any deviation from these principles MUST be justified in writing (PR description or inline comment) and approved by a maintainer.
- **Runtime guidance**: The `CLAUDE.md` file at the repository root provides operational guidance for AI-assisted development. It MUST remain consistent with this constitution. When conflicts arise, this constitution takes precedence.

**Version**: 1.0.0 | **Ratified**: 2026-02-15 | **Last Amended**: 2026-02-15
