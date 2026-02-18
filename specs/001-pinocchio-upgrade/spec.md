# Feature Specification: Pinocchio Ecosystem Upgrade & Crate Cleanup

**Feature Branch**: `001-pinocchio-upgrade` **Created**: 2026-02-15 **Status**: Draft **Input**: User description: "Upgrade pinocchio ecosystem to latest versions, remove pina_token_2022_extensions, update readme and add examples"

## User Scenarios & Testing _(mandatory)_

### User Story 1 - Upgrade to Latest Pinocchio Types (Priority: P1)

A developer maintaining the pina framework upgrades all pinocchio ecosystem dependencies to the latest versions (pinocchio 0.10.x, pinocchio-token 0.5.x, pinocchio-system 0.5.x, etc.) so that pina users benefit from the latest performance improvements, SDK type alignment (AccountView, Address), and CPI feature gating.

**Why this priority**: This is the foundational change. Every other story depends on the type renames and API migrations being complete. Without this, the framework cannot compile against the latest pinocchio ecosystem.

**Independent Test**: Build the entire workspace with `cargo build --all-features` and run `cargo nextest run` to confirm no regressions.

**Acceptance Scenarios**:

1. **Given** the workspace Cargo.toml specifies pinocchio ^0.10, pinocchio-token ^0.5, pinocchio-system ^0.5, pinocchio-associated- token-account ^0.3, pinocchio-memo ^0.3, and pinocchio-token-2022 ^0.2, **When** `cargo build --all-features` is run, **Then** the build succeeds with zero errors.
2. **Given** the pina crate re-exports the new upstream types (AccountView, Address), **When** a downstream program uses `pina::AccountView` and `pina::Address`, **Then** the types resolve correctly to the pinocchio/solana-address equivalents.
3. **Given** the pinocchio-pubkey crate is deprecated upstream, **When** the pina_sdk_ids crate is built, **Then** it uses the new `declare_id!` macro source instead of `pinocchio_pubkey::declare_id!`.
4. **Given** CPI functionality now requires the `"cpi"` feature on pinocchio, **When** the pina crate enables CPI helpers, **Then** the `cpi` feature is correctly propagated to pinocchio.
5. **Given** the existing test suite covers account validation, Pod types, discriminators, and CPI helpers, **When** `cargo nextest run` is executed, **Then** all existing tests pass.

---

### User Story 2 - Remove pina_token_2022_extensions Crate (Priority: P2)

A maintainer removes the `pina_token_2022_extensions` crate entirely from the workspace because upstream pinocchio-token-2022 is actively adding native extension support. Keeping a deprecated crate adds maintenance burden and confuses users about the canonical way to handle token-2022 extensions.

**Why this priority**: This crate is explicitly slated for deprecation in CLAUDE.md. Removing it during the breaking upgrade avoids carrying forward code that references the old pinocchio 0.9.x API.

**Independent Test**: After removal, `cargo build --all-features` and `cargo nextest run` succeed. No remaining references to `pina_token_2022_extensions` exist in the workspace.

**Acceptance Scenarios**:

1. **Given** the `crates/pina_token_2022_extensions/` directory exists, **When** the removal is applied, **Then** the directory and all its contents are deleted.
2. **Given** the root `Cargo.toml` lists `pina_token_2022_extensions` as a workspace member and dependency, **When** the removal is applied, **Then** all references are removed from the workspace manifest.
3. **Given** other crates may reference `pina_token_2022_extensions` in their `Cargo.toml` or source files, **When** the removal is applied, **Then** no compile errors or dangling references remain.
4. **Given** `CLAUDE.md`, `readme.md`, and changeset configuration reference `pina_token_2022_extensions`, **When** the removal is applied, **Then** all documentation and configuration references are removed or updated.

---

### User Story 3 - Update README with Comprehensive Usage Documentation (Priority: P3)

A new user visiting the pina repository reads the `readme.md` to understand what pina is, how to install it, how to define accounts and instructions using pina macros, and how to build and test a Solana program. The readme MUST provide enough information to get started without reading source code.

**Why this priority**: Documentation is a critical adoption driver. With the breaking API changes (AccountView, Address), the README MUST reflect the current API to avoid confusing users.

**Independent Test**: A reader can follow the README end-to-end and understand pina's value proposition, installation, core concepts (entrypoint, accounts, instructions, discriminators, validation, Pod types, CPI), and how to build for SBF.

**Acceptance Scenarios**:

1. **Given** the readme.md exists, **When** the update is applied, **Then** it contains sections for: overview, installation, quick start, core concepts (entrypoint, accounts, instructions, discriminators, validation chains, Pod types, CPI helpers), building for SBF, testing, and contributing.
2. **Given** the API now uses `AccountView` and `Address`, **When** the readme shows code examples, **Then** all examples use the new type names.
3. **Given** `pina_token_2022_extensions` has been removed, **When** the readme is reviewed, **Then** no references to that crate remain.

---

### User Story 4 - Port Solana Example Programs (Priority: P4)

A developer learning pina studies example programs ported from the solana-developers/program-examples repository. Each example is heavily commented, documented, and demonstrates how pina simplifies common Solana program patterns compared to raw pinocchio or solana-program.

**Why this priority**: Examples are the most effective teaching tool, but they depend on the core upgrade (US1) and documentation (US3) being complete. This is also specified as a separate PR.

**Independent Test**: Each example program builds for SBF target and passes mollusk-svm tests with comprehensive inline documentation.

**Acceptance Scenarios**:

1. **Given** the `examples/` directory exists, **When** example programs are added, **Then** each builds successfully for the SBF target.
2. **Given** each example program, **When** its test suite is run, **Then** all tests pass using mollusk-svm simulation.
3. **Given** each example program, **When** the source code is reviewed, **Then** every public function, struct, and module has documentation comments explaining what it does and why.
4. **Given** the examples are delivered in a separate PR, **When** the PR is created, **Then** it is independent of the upgrade PR and can be merged separately.

---

### User Story 5 - Changeset & Semver Compliance (Priority: P1)

A release manager verifies that all breaking changes from the pinocchio upgrade are captured in granular knope changeset files and that `cargo semver-checks` is run after each change to detect API breakage. Each changeset MUST describe the specific change and its migration path.

**Why this priority**: Co-equal with US1 because changesets MUST accompany every code change per the project constitution. Without them, the release workflow breaks.

**Independent Test**: Changeset files exist in `.changeset/` for each distinct breaking change.

**Acceptance Scenarios**:

1. **Given** the pinocchio type renames are applied, **When** `cargo semver-checks` is run on pina, **Then** it reports breaking changes.
2. **Given** breaking changes are detected, **When** changeset files are created, **Then** each changeset specifies the affected package with change type `major`.
3. **Given** multiple distinct changes (type renames, crate removal, dependency bumps), **When** changesets are created, **Then** they are granular (one changeset per logical change, not one monolithic changeset).
4. **Given** all changesets are created, **When** `dprint fmt .changeset/* --allow-no-files` is run, **Then** all changeset files pass formatting.

---

### Edge Cases

- What happens if downstream crates depend on `pina_token_2022_extensions`? The crate has never been widely adopted; removal is clean. Migration notes in the changeset provide guidance.
- What if pinocchio 0.10.x changes the entrypoint macro signature? The `nostd_entrypoint!` macro in pina MUST be updated to match the new function signature using AccountView and Address.
- What if `pinocchio-pubkey` is still needed as a transitive dependency? It is not; `solana-address` provides all equivalent functionality including `declare_id!`.
- What if the escrow example program uses deprecated types? It MUST be updated to use AccountView/Address as part of US1.
- What if `pinocchio-log` changes its API? It remains at 0.5.1 and is independent of the core pinocchio 0.10.x migration.

## Requirements _(mandatory)_

### Functional Requirements

- **FR-001**: The workspace MUST compile against pinocchio ^0.10 and all related crates at their latest compatible versions.
- **FR-002**: All occurrences of `AccountInfo` in the pina public API MUST be renamed to `AccountView` (or re-exported under the new name).
- **FR-003**: All occurrences of `Pubkey` in the pina public API MUST be renamed to `Address` (or re-exported under the new name).
- **FR-004**: The `pinocchio-pubkey` dependency MUST be replaced with `solana-address` (or its pinocchio re-export) across all crates.
- **FR-005**: CPI helpers in pina MUST work with the new `"cpi"` feature gate on pinocchio.
- **FR-006**: The `pina_token_2022_extensions` crate and all references to it MUST be removed from the workspace.
- **FR-007**: The pina_sdk_ids crate MUST use the new declare_id! macro source.
- **FR-008**: The escrow example program MUST be updated to use the new types and MUST build for SBF.
- **FR-009**: The readme.md MUST be updated with comprehensive documentation reflecting the new API.
- **FR-010**: All changes MUST be accompanied by knope changeset files with `major` change type for breaking changes.
- **FR-011**: `cargo semver-checks` MUST be run to validate that all breaking changes are correctly identified and documented.
- **FR-012**: Ported example programs MUST be in a separate PR and MUST include heavy inline documentation and comments.
- **FR-013**: All existing tests MUST pass after the upgrade, and new tests MUST be added to cover expanded functionality.
- **FR-014**: The `nostd_entrypoint!` macro MUST be updated to use the new pinocchio entrypoint function signature.
- **FR-015**: The CLAUDE.md file MUST be updated to reflect any architectural changes resulting from the upgrade.

### Key Entities

- **AccountView**: The new pinocchio type replacing `AccountInfo`. Zero-copy view of an account passed to on-chain programs.
- **Address**: The new type replacing `Pubkey`. A 32-byte Solana account address, sourced from `solana-address`.
- **InstructionView**: Replaces `Instruction` for CPI calls. Now behind the `"cpi"` feature gate on pinocchio.

## Success Criteria _(mandatory)_

### Measurable Outcomes

- **SC-001**: `cargo build --all-features` succeeds with zero errors and zero warnings related to deprecated pinocchio APIs.
- **SC-002**: `cargo nextest run` passes all tests (existing and new) with a 100% pass rate.
- **SC-003**: `cargo semver-checks` correctly identifies all breaking changes and no unexpected breakage is introduced.
- **SC-004**: At least one changeset file exists per distinct breaking change (minimum 3: type renames, crate removal, dependency bumps).
- **SC-005**: The readme.md covers all core concepts with working code examples that use the new API types.
- **SC-006**: The escrow example program builds for SBF and its tests pass under mollusk-svm simulation.
- **SC-007**: Zero references to `pina_token_2022_extensions` remain in the workspace after removal.
- **SC-008**: Ported example programs each have documentation comments on every public item and inline comments explaining non-obvious logic.
