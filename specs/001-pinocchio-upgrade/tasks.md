# Tasks: Pinocchio Ecosystem Upgrade & Crate Cleanup

**Input**: Design documents from `/specs/001-pinocchio-upgrade/` **Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, quickstart.md

**Tests**: Tests ARE requested — spec FR-013 requires all existing tests pass and new tests be added. Test updates are integrated into implementation tasks rather than as separate TDD tasks, since this is a migration (not new feature development).

**Organization**: Tasks are grouped by user story. US5 (Changeset & Semver) is interwoven into each phase as changesets MUST accompany code changes per the constitution.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- Rust workspace with crates at `crates/` and examples at `examples/`
- Config files at repository root

---

## Phase 1: Setup (Workspace Dependency Updates)

**Purpose**: Update all workspace-level dependency versions so individual crates can begin migration

- [ ] T001 [US1] Update workspace dependency versions in `Cargo.toml`: bump pinocchio to ^0.10, pinocchio-system to ^0.5, pinocchio-token to ^0.5, pinocchio-token-2022 to ^0.2, pinocchio-associated-token-account to ^0.3, pinocchio-memo to ^0.3. Add solana-address ^2.0 (features: decode) and solana-program-log ^1.1. Remove pinocchio-pubkey and pinocchio-log entries. Remove pina_token_2022_extensions from workspace members and dependencies.
- [ ] T002 [US1] Update pina crate dependencies in `crates/pina/Cargo.toml`: replace pinocchio-pubkey with solana-address (features: decode), replace pinocchio-log with solana-program-log (with macro feature), add features = ["cpi"] to pinocchio dependency. Bump all pinocchio helper crate versions to match workspace.

---

## Phase 2: Foundational (Core pina Crate Migration)

**Purpose**: Migrate the core `pina` crate to pinocchio 0.10.x types. This MUST complete before any other crate can compile.

**CRITICAL**: All downstream crates (pina_macros tests, pina_sdk_ids, escrow_program) depend on pina re-exports. This phase must fully compile before Phase 3.

- [ ] T003 [US1] Update re-exports and macros in `crates/pina/src/lib.rs`: replace `pinocchio::account_info::AccountInfo` with `pinocchio::AccountView`, replace `pinocchio::pubkey::Pubkey` with `pinocchio::Address`, replace `pinocchio_pubkey::*` re-export with `pinocchio::address::*`, update `pinocchio::instruction::*` imports for cpi feature, replace `pinocchio_log` with `solana_program_log` in log! macro and re-exports, update `pinocchio::program_error::ProgramError` import path, update nostd_entrypoint! macro doc comments to reference Address and AccountView
- [ ] T004 [US1] Update trait definitions in `crates/pina/src/traits.rs`: replace all `AccountInfo` with `AccountView`, replace all `Pubkey` with `Address` in trait method signatures, update `TryFromAccountInfos` trait (rename and/or update parameter types), update `AccountInfoValidation` trait (update impl target type), update `AsAccount` trait signatures, update `LamportTransfer` and `CloseAccountWithRecipient` trait signatures, update `AsTokenAccount` trait signatures (depends on T003)
- [ ] T005 [US1] Update validation implementations in `crates/pina/src/loaders.rs`: change all `impl ... for AccountInfo` to `impl ... for AccountView`, replace `key()` calls with `address()`, replace `try_borrow_data()` with `try_borrow()`, replace `try_borrow_mut_data()` with `try_borrow_mut()`, replace `realloc()` with `resize()`, update `try_find_program_address` and `create_program_address` import paths, update all `Pubkey` references to `Address` (depends on T004)
- [ ] T006 [US1] Update CPI helpers in `crates/pina/src/cpi.rs`: replace `Instruction` with `InstructionView`, replace `AccountMeta` with `InstructionAccount`, update `Signer` and `Seed` import paths from solana_instruction_view, update `invoke_signed` import from pinocchio::cpi, update `combine_seeds_with_bump` for new Seed type, update all `Pubkey` to `Address`, update Rent::get() usage if API changed (depends on T003)
- [ ] T007 [US1] Update utility functions in `crates/pina/src/utils.rs`: replace `Pubkey` with `Address` in parse_instruction and try_get_associated_token_address, update pinocchio ATA ID import path (depends on T003)
- [ ] T008 [US1] Verify pina crate compiles: run `cargo build -p pina --all-features` and fix any remaining compilation errors across all modified source files (depends on T003-T007)
- [ ] T009 [US1] [US5] Create changeset for pina core type migration in `.changeset/pina-pinocchio-upgrade.md`: document AccountInfo→AccountView, Pubkey→Address, pinocchio-pubkey→solana-address, pinocchio-log→solana-program-log replacements. Set change type to `major` for pina. Run `dprint fmt .changeset/* --allow-no-files` (depends on T008)

**Checkpoint**: The pina crate compiles against pinocchio 0.10.x. Downstream crates can now begin migration.

---

## Phase 3: User Story 1 - Complete Pinocchio Upgrade (Priority: P1)

**Goal**: All remaining crates compile and all tests pass with pinocchio 0.10.x types

**Independent Test**: `cargo build --all-features && cargo nextest run` succeeds

### Implementation for User Story 1

- [ ] T010 [P] [US1] Update generated code in `crates/pina_macros/src/lib.rs`: replace `::pina::AccountInfo` with `::pina::AccountView` in #[derive(Accounts)] generated code, update `::pina::TryFromAccountInfos` trait name if renamed, verify all other `::pina::` references still resolve correctly (depends on Phase 2)
- [ ] T011 [P] [US1] Update pina_sdk_ids crate: replace `pinocchio-pubkey` with `solana-address` (features: decode) in `crates/pina_sdk_ids/Cargo.toml`, replace all `pinocchio_pubkey::declare_id!` with `solana_address::declare_id!` in `crates/pina_sdk_ids/src/lib.rs` (depends on Phase 2)
- [ ] T012 [P] [US1] Update escrow example in `examples/escrow_program/src/lib.rs`: replace all `Pubkey` with `Address` in EscrowState fields and SPL_PROGRAM_IDS, replace all `&'a AccountInfo` with `&'a AccountView` in account structs, update CPI instruction calls for new token/system/ata API, update `key()` calls to `address()` if used, verify all validation chain methods still work (depends on Phase 2)
- [ ] T013 [US1] Update all test files in `crates/pina/tests/`: update `accounts_derive.rs` to use AccountView, update `account_macro.rs` for new types, update `discriminator_macro.rs`, `instruction_macro.rs`, `error_macro.rs`, `event_macro.rs` if they reference AccountInfo/Pubkey, update `cpi_helpers.rs` for new CPI types (depends on T010)
- [ ] T014 [US1] Run full workspace build: `cargo build --all-features` — fix any remaining compilation errors across all crates (depends on T010-T013)
- [ ] T015 [US1] Run full test suite: `cargo nextest run` — fix any test failures, update test assertions for new type names (depends on T014)
- [ ] T016 [US1] Build escrow for SBF: run `cargo build-escrow-program` — verify the on-chain build succeeds with pinocchio 0.10.x (depends on T014)
- [ ] T017 [P] [US1] [US5] Create changeset for pina_macros in `.changeset/pina-macros-pinocchio-upgrade.md`: document generated code changes (AccountView references). Set change type to `major` for pina_macros. Run `dprint fmt .changeset/* --allow-no-files` (depends on T010)
- [ ] T018 [P] [US1] [US5] Create changeset for pina_sdk_ids in `.changeset/pina-sdk-ids-solana-address.md`: document pinocchio-pubkey→solana-address migration for declare_id!. Set change type to `major` for pina_sdk_ids. Run `dprint fmt .changeset/* --allow-no-files` (depends on T011)

**Checkpoint**: All crates compile, all tests pass, SBF build succeeds. Core upgrade is complete.

---

## Phase 4: User Story 2 - Remove pina_token_2022_extensions (Priority: P2)

**Goal**: The deprecated crate is fully removed with zero remaining references

**Independent Test**: `grep -r "pina_token_2022_extensions" --include="*.toml" --include="*.rs" --include="*.md" .` returns zero results

### Implementation for User Story 2

- [ ] T019 [US2] Delete the entire `crates/pina_token_2022_extensions/` directory
- [ ] T020 [US2] Remove pina_token_2022_extensions from `knope.toml`: delete the package entry, scopes, and changelog reference for pina_token_2022_extensions (depends on T019)
- [ ] T021 [US2] Verify no remaining references: search all `.toml`, `.rs`, `.md`, and `.nix` files for `pina_token_2022_extensions` — remove or update any found (depends on T019-T020)
- [ ] T022 [US2] Run `cargo build --all-features` to verify clean build after removal (depends on T021)
- [ ] T023 [US2] [US5] Create changeset for crate removal in `.changeset/remove-pina-token-2022-extensions.md`: document full removal of pina_token_2022_extensions crate with rationale (upstream pinocchio-token-2022 adding native support). Set change type to `major` for pina (since it's a workspace-level change removing a published crate). Run `dprint fmt .changeset/* --allow-no-files` (depends on T022)

**Checkpoint**: pina_token_2022_extensions fully removed. Workspace compiles cleanly.

---

## Phase 5: User Story 3 - Documentation Update (Priority: P3)

**Goal**: readme.md provides comprehensive usage documentation with new API types, CLAUDE.md reflects architectural changes

**Independent Test**: README contains sections for overview, installation, core concepts (entrypoint, accounts, instructions, discriminators, validation chains, Pod types, CPI), building for SBF, testing. All code examples use AccountView and Address.

### Implementation for User Story 3

- [ ] T024 [P] [US3] Rewrite `readme.md` with comprehensive documentation: overview and value proposition, installation instructions, quick start guide, core concepts (nostd_entrypoint! macro with Address/AccountView, #[account] macro, #[instruction] macro, #[discriminator] macro, #[event] macro, #[error] macro, #[derive(Accounts)], validation chains, Pod types, CPI helpers), building for SBF, testing with mollusk-svm, crate table (without pina_token_2022_extensions), contributing guide. All code examples MUST use AccountView/Address.
- [ ] T025 [P] [US3] Update `CLAUDE.md`: update workspace crates list (remove pina_token_2022_extensions), update core patterns section with AccountView/Address in entrypoint example, update account validation example, update toolchain notes if needed, update changeset package names list, update security constraints if any changed
- [ ] T026 [US3] Update `.specify/memory/constitution.md`: remove `pina_token_2022_extensions` from package scopes in Principle V, update `Result<&AccountInfo>` reference in Principle II to `Result<&AccountView>`, increment constitution version to 1.1.0 (MINOR: scope expansion), update Last Amended date to today (depends on T024-T025)
- [ ] T027 [US3] [US5] Create changeset for documentation in `.changeset/docs-pinocchio-upgrade.md`: document readme rewrite and CLAUDE.md updates. Set change type to `docs` for pina. Run `dprint fmt .changeset/* --allow-no-files` (depends on T024-T025)

**Checkpoint**: Documentation fully updated. New users can understand the framework from the README alone.

---

## Phase 6: User Story 5 - Final Changeset & Semver Validation (Priority: P1)

**Goal**: All breaking changes are captured in changesets, semver-checks validates, all quality gates pass

**Independent Test**: `cargo semver-checks` reports expected breakage, all changesets pass formatting, `lint:all` passes

### Validation for User Story 5

- [ ] T028 [US5] Run `cargo semver-checks` on all publishable crates: verify breaking changes are detected for pina, pina_macros, pina_sdk_ids. Document any unexpected breakage. If new breaking changes found not covered by existing changesets, create additional changeset files (depends on Phase 3-5)
- [ ] T029 [US5] Verify all changesets formatted: run `dprint fmt .changeset/* --allow-no-files` and confirm all pass. Verify at least 4 changeset files exist (pina upgrade, pina_macros, pina_sdk_ids, crate removal) (depends on T028)
- [ ] T030 [US5] Run full quality gate suite: `lint:all` (clippy + dprint check), `cargo nextest run`, `cargo build-escrow-program` — all must pass (depends on T029)

**Checkpoint**: All quality gates pass. PR is ready for review.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Final validation and cleanup

- [ ] T031 Run quickstart.md validation: follow each step in `specs/001-pinocchio-upgrade/quickstart.md` and verify all pass (depends on T030)
- [ ] T032 Add new tests for expanded functionality: add tests in `crates/pina/tests/` for any new pinocchio 0.10.x features exposed by pina (e.g., new system program helpers, new token instructions). Ensure `cargo nextest run` passes (depends on T030)
- [ ] T033 Final review: verify no TODO markers remain, no commented-out code, no leftover 0.9.x references, Cargo.lock is committed and clean (depends on T031-T032)

**Checkpoint**: PR1 (pinocchio upgrade) is complete and ready for merge.

---

**Note**: User Story 4 (Port Solana Examples) is explicitly scoped as a **separate PR** per the spec. It is NOT included in this task list. Create a new feature branch and task list for US4 after PR1 merges.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 — BLOCKS all user stories
- **US1 (Phase 3)**: Depends on Phase 2 — core upgrade completion
- **US2 (Phase 4)**: Depends on Phase 1 (Cargo.toml already updated) — can start after Phase 1 but verifying with full build requires Phase 3
- **US3 (Phase 5)**: Depends on Phase 3 and Phase 4 — documentation must reflect final API and crate list
- **US5 (Phase 6)**: Depends on Phase 3-5 — final validation of all changes
- **Polish (Phase 7)**: Depends on Phase 6 — final checks

### Within Phase 2 (Sequential)

1. T003 (lib.rs re-exports) FIRST — all other files import from crate root
2. T004 (traits.rs) — defines types used by loaders.rs
3. T005 (loaders.rs), T006 (cpi.rs), T007 (utils.rs) — can be parallel after T004
4. T008 (verify build) — after all source changes

### Within Phase 3 (Parallel Opportunities)

- T010, T011, T012 can all run in parallel (separate crates)
- T013 depends on T010 (macros must compile first for test macros)
- T017, T018 can run in parallel with each other

### Within Phase 4 (Sequential)

- T019 → T020 → T021 → T022 → T023

### Within Phase 5 (Parallel Opportunities)

- T024, T025 can run in parallel (different files)
- T026, T027 depend on T024-T025

---

## Parallel Execution Examples

### Phase 2 — After T004 completes:

```text
Task: "Update loaders.rs validation impls" (T005)
Task: "Update cpi.rs CPI helpers" (T006)
Task: "Update utils.rs helpers" (T007)
```

### Phase 3 — After Phase 2 completes:

```text
Task: "Update pina_macros generated code" (T010)
Task: "Update pina_sdk_ids declare_id!" (T011)
Task: "Update escrow example types" (T012)
```

### Phase 5 — After Phase 4 completes:

```text
Task: "Rewrite readme.md" (T024)
Task: "Update CLAUDE.md" (T025)
```

---

## Implementation Strategy

### MVP First (Phase 1-3 Only)

1. Complete Phase 1: Workspace dependency updates
2. Complete Phase 2: Core pina crate migration
3. Complete Phase 3: All crates compile + tests pass
4. **STOP and VALIDATE**: `cargo build --all-features && cargo nextest run && cargo build-escrow-program`
5. This is the minimum viable upgrade — workspace is functional

### Full Delivery (Phase 1-7)

1. Phase 1-3: Core upgrade (MVP)
2. Phase 4: Remove deprecated crate
3. Phase 5: Documentation rewrite
4. Phase 6: Changeset/semver validation
5. Phase 7: Polish and final checks
6. Open PR for review

### Commit Strategy

- Commit after each completed phase (minimum)
- Preferred: commit after each logical task group
- Suggested commits:
  1. After T002: `feat!: update workspace deps to pinocchio 0.10.x`
  2. After T008: `feat!: migrate pina core to AccountView/Address types`
  3. After T016: `feat!: complete pinocchio 0.10.x upgrade across all crates`
  4. After T023: `feat!: remove pina_token_2022_extensions crate`
  5. After T027: `docs: rewrite readme and update CLAUDE.md for new API`
  6. After T033: `chore: final validation and polish`

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- US4 (port examples) is a SEPARATE PR — not in this task list
- US5 (changesets) is distributed across all phases, not a standalone phase
- Phase 6 is US5's validation-only phase (checking all changesets are complete)
- Commit after each task or logical group per project convention
- Run `dprint fmt .changeset/* --allow-no-files` after every changeset creation
