# Port to zeropod — Implementation Tracking

_Issue: pina-rs/pina#193_ _Branch: `explore/zeropod-alignment` (PR: pina-rs/pina#195)_

## Design decisions (locked)

1. **zeropod as primitives only** — no zeropod derives. Pina's own macros generate the trait impls.
2. **Direct struct pattern** — `ZeroPodFixed` with `type Zc = Self`. No `FooZc` companion.
3. **Discriminator-first layout** — discriminator is the first field of the struct (ADR 0001). `SIZE` includes it.
4. **`PinaAccount: HasDiscriminator`** — new trait with `validate()`, `try_from_bytes()`, `try_from_bytes_mut()`. Replaces `AccountDeserialize` (removed).
5. **`unsafe_code`** — macro emits `#[allow(unsafe_code)]` on generated `unsafe impl ZcElem` and unsafe helper methods.
6. **Compact mode** — deferred to a follow-up PR (pina has no compact accounts today; fixed-layout port is the core). The `PinaAccount` trait is designed to extend.
7. **`zeroed()`/`to_bytes()`** — replaced with unsafe pointer ops (sound: all fields are align-1 pod types, compile-time asserted).

## Work log

### Phase 1: Foundation (pina crate + macros) ✅

- [x] Add `zeropod` to workspace deps + `pina` Cargo.toml (features `solana-address` + `solana-program-error`)
- [x] `pina/src/lib.rs`: remove `bytemuck`/`Pod`/`Zeroable` re-exports; add zeropod re-exports
- [x] `pina/src/traits.rs`: add `PinaAccount` (validate/try_from_bytes/try_from_bytes_mut); remove `AccountDeserialize`; update `assert_type` bounds
- [x] `pina/src/impls.rs`: update `as_account`/`as_account_mut` bounds to `T: PinaAccount`
- [x] `pina/src/cpi.rs`: update `create_program_account` bounds to `T: PinaAccount`
- [x] `pina/src/transaction.rs`: `InstructionBuilder` bound → `ZeroPodFixed<Zc = T>`; `data()` via unsafe pointer cast
- [x] `pina/src/pod/`: re-export zeropod primitives; `pod_from_bytes` reimplemented via `ZcElem` + `ZcValidate`
- [x] `pina_macros`: `account`/`instruction`/`event` generate zeropod trait impls (direct pattern); `discriminator` drops `unsafe impl Pod`/`Zeroable`
- [x] `pina` tests pass (all 19 test targets)

### Phase 2: Downstream crates ✅

- [x] `pina_codama_renderer`: scaffold generation → zeropod patterns; manual zero-copy casts in generated clients
- [x] `pina_fuzz`: `AccountDeserialize` → `PinaAccount`
- [x] `pina_cli`: unchanged (name-based parsing)

### Phase 3: Examples + security + codama clients ✅

- [x] All examples updated: `from_primitive` → `From`, `from_bool` → `From`, `try_as_str` → `as_str`, `as_mut_slice` → `as_slice_mut`, `.0` → `.as_ref()`
- [x] Security examples updated
- [x] Codama generated clients updated (manual casts, no bytemuck)

### Phase 4: Tests ✅

- [x] Update `crates/pina/tests/*`
- [x] Update `miri_loader_guards.rs` (zeropod impls; Miri passes 5/5)
- [x] Delete `pina_pod_primitives` + its tests
- [x] Add validation tests (boundary rejection) — `pod_enum.rs` (14 tests) + existing boundary-rejection coverage
- [x] 722 workspace tests pass

### Phase 5: CI + tooling ✅

- [x] `kani.yml` removed (no kani proofs remain after `pina_pod_primitives` deletion); `mutants.toml`, `devenv.nix`, `monochange.toml` — remove `pina_pod_primitives`
- [x] Root `Cargo.toml` — remove `pina_pod_primitives` member + workspace dep
- [x] Changesets added (`.changeset/port_to_zeropod.md`, `.changeset/pod_enum_derive.md`)

### Phase 6: Docs ✅

- [x] `docs/src/security-model.md` (content validation + unit-enum sections), ADR 0002, readme, templates, renderer docs
- [x] mdt sync + check pass (devenv mdt 0.9.0)

### Phase 7: Verification ✅

- [x] `cargo build --workspace --all-features` — clean
- [x] `cargo test --workspace` — 722 passed, 0 failed
- [x] `cargo clippy --workspace --all-features --locked` — no errors
- [x] `dprint check` / `mdt check` (devenv mdt 0.9.0) — clean
- [x] `cargo build -p pina --no-default-features` — clean
- [x] Miri (`miri_loader_guards`) — 5/5 pass
- [x] Merged with latest `origin/main` (incl. #194 `#[pda]`)
- [x] Codama clients regenerated with current toolchain; idl drift check passes (0 diff)
- [x] Kani workflow removed (no proofs remain after pina_pod_primitives deletion)
- [x] Create PR: https://github.com/pina-rs/pina/pull/195
- [x] **All 24 CI checks pass** (lint, test, miri, idl, changeset-policy, semver, coverage, binary-size, feature-matrix, mutants, program-e2e, compute-units, surfpool)
- [x] PR is MERGEABLE (mergeStateStatus: CLEAN)

## Notes

- zeropod 0.3.5 on crates.io; `From<ZeroPodError> for ProgramError` via `solana-program-error` feature
- pinocchio's `ProgramError` IS `solana_program_error::ProgramError` — conversion works directly
- zeropod numeric API ≈ pina's (checked/saturating/wrapping, bitwise, MAX/MIN/ZERO) — only `from_primitive` and `From<&T>` differ
