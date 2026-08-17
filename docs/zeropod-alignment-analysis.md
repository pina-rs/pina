# Zeropod Alignment Analysis: Verifiable Security for Pina's Primitives

_Exploration branch: `explore/zeropod-alignment`_ _Date: 2026-08-17_

This document is a deep-dive comparison of pina's primitives library (`pina_pod_primitives` + the `pina` account/instruction layer) against [Zeropod](https://github.com/blueshift-gg/zeropod), focused on the three questions asked:

1. How can we use testing to **verifiably prove** that a program built on pina is secure?
2. How can we build that into the framework so **the framework itself** is secure?
3. How can we make that available to **users of the codebase** so they get the same guarantees?

---

## 1. What Zeropod does that pina doesn't (the "Zeropod way")

Zeropod's core insight is that **`Pod` (bytemuck) guarantees memory safety but not semantic validity**. A `&PodBool` formed from byte `0x02` is memory-safe but semantically wrong. Zeropod closes this gap with a two-trait safety model:

### 1.1 `ZcValidate` — every type validates itself

```rust
pub trait ZcValidate: Copy {
	fn validate_ref(value: &Self) -> Result<(), ZeroPodError>;
}
```

Every pod type implements this:

- `PodBool` → rejects bytes `> 1` (`InvalidBool`)
- `PodString<N, PFX>` → rejects `len > N` (`InvalidLength`) and invalid UTF-8 (`InvalidUtf8`)
- `PodVec<T, N, PFX>` → rejects `len > N`, recurses into each element
- `PodOption<T>` → rejects tag `> 1` (`InvalidTag`), recurses into inner if `Some`
- Numeric types / `[u8; N]` → trivially `Ok(())`

### 1.2 `ZcElem` — an unsafe trait with 4 documented safety requirements

```rust
/// # Safety
/// Implementors MUST guarantee all four of the following:
/// 1. Alignment == 1
/// 2. No padding
/// 3. Validity invariant holds for every bit pattern (no bare `bool`, `char`, `NonZero*`)
/// 4. `ZcValidate::validate_ref` is load-bearing — it must reject every bit
///    pattern whose reading through a safe accessor could cause UB or violate
///    the type's documented invariants.
pub unsafe trait ZcElem: Copy + ZcValidate {}
```

Requirement 4 is the key: **validation is load-bearing**. The deserialization path (`from_bytes`) is:

```rust
fn from_bytes(data: &[u8]) -> Result<&Self::Zc, ZeroPodError> {
	Self::validate(data)?; // 1. validate FIRST
	Ok(unsafe { &*(data.as_ptr() as *const Self::Zc) }) // 2. then cast
}
```

There is **no safe way** to obtain a `&Zc` from invalid bytes.

### 1.3 The derive macro generates validation

`#[derive(ZeroPod)]` generates a `ZcValidate` impl that delegates to each field's `validate_ref`:

```rust
impl zeropod::ZcValidate for #zc_name {
    fn validate_ref(value: &Self) -> Result<(), ZeroPodError> {
        #(<#pod_field_types as zeropod::ZcValidate>::validate_ref(&value.#field_names)?;)*
        Ok(())
    }
}
```

So a user's `#[derive(ZeroPod)] struct TokenAccount { ... }` automatically gets recursive validation of every field, for free.

### 1.4 The test suite is organized around proving safety

| Test file                                               | What it proves                                                                                                                                                                                                                      |
| ------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `validation.rs` (381 lines)                             | Every invalid input class is rejected: bad bool, bad tag, overlength string, overlength vec, invalid UTF-8, truncated buffer, invalid inner values in `Option<Enum>`, invalid `Vec<PodBool>` elements, compact-layout tail overruns |
| `layout_golden.rs`                                      | Exact sizes, alignments, and **field offsets** are pinned (e.g. `GoldenFixed` fields at offsets 0, 1, 9, 10)                                                                                                                        |
| `roundtrip.rs`                                          | Write-then-read roundtrips AND `fixed_byte_stability` — identical writes produce identical bytes                                                                                                                                    |
| `instruction_abi_parity.rs`                             | Account and instruction compact layouts produce **identical tail bytes** — the same compact format regardless of context                                                                                                            |
| `pod_types.rs`, `pod_richness.rs`, `type_tightening.rs` | API ergonomics and type-level guarantees                                                                                                                                                                                            |
| `compact_backend.rs`, `compact_commit_shift.rs`         | Compact layout mutation/commit semantics                                                                                                                                                                                            |

---

## 2. Pina's current state

### 2.1 What pina already does well

- **Compile-time layout assertions** — every pod type has `const _: () = assert!(align_of::<T>() == 1)` and size asserts. The `#[account]` macro generates per-field alignment asserts and a no-padding size assert.
- **Kani model-checking proofs** — in-crate `option.rs`/`string.rs`/`vec.rs` plus `tests/kani_harnesses.rs` (roundtrips, checked/saturating arithmetic, bitwise ops, ordering, overflow rejection, len clamping). Wired into CI via `.github/workflows/kani.yml` → `kani:proofs`.
- **Property-based tests** — `tests/fuzz_pod.rs` (531 lines) proptests all numeric types and `PodBool` canonicality; `crates/pina/tests/fuzz_discriminator.rs` proptests discriminator parsing; `prop_utils.rs` covers utils.
- **Miri** — dedicated `miri_loader_guards` regression suite for the guard-backed loader redesign (the F1/F2 escaped-borrow findings from `security/loaders-audit.md`).
- **Mutation testing** — `mutants.toml` + `mutants-pr.yml` / `mutants-nightly.yml`.
- **Layered CI** — ADR 0006 codifies standard tests, feature-matrix, compile-fail, Miri, IDL verification, security verification, binary-size/CU reporting.

### 2.2 The gaps (where Zeropod is ahead)

**Gap 1 — No content validation at the deserialization boundary.**

`AccountDeserialize::try_from_bytes` (crates/pina/src/traits.rs:55-81):

```rust
fn try_from_bytes(data: &[u8]) -> Result<&Self, ProgramError> {
	if !Self::matches_discriminator(data) {
		return Err(ProgramError::InvalidAccountData);
	}
	bytemuck::try_from_bytes::<Self>(data).or(Err(ProgramError::InvalidAccountData))
}
```

This checks the **discriminator** and the **size**, then pointer-casts. It does **not** validate the _contents_. Consequences:

- A `PodBool(2)` (non-canonical) deserializes fine. `bool::from(PodBool(2)) == true`, but `PodBool(2) != PodBool(1)` — `PartialEq` is inconsistent with the logical value. The pina docs even document this: _"two non-canonical PodBool values representing the same logical boolean may fail PartialEq comparison"_ (pod_bool.rs:20-23).
- A `PodString` with invalid UTF-8 deserializes fine. `try_as_str()` then fails at _use time_, not at the boundary. Worse, `as_str_unchecked()` (unsafe) is one bad call away from UB.
- A `PodVec` with an overlength length prefix deserializes fine. `len()` silently clamps via `.min(N)` — corrupted data is silently accepted rather than rejected.

Zeropod rejects all three at the boundary. This is the single biggest difference.

**Gap 2 — No `ZcValidate`-style trait.**

Validation logic is scattered and inconsistent:

- `PodBool::is_canonical()` exists but is opt-in (caller must remember).
- `PodString::try_as_str()` validates UTF-8 but only at use time.
- `PodVec` has **no validation method at all** — overlength is clamped, not rejected.
- `PodOption::get()` returns `None` for invalid tags (good) but there's no reject path.

There is no trait that says "this type knows how to validate itself", so the `#[account]` macro and `AccountDeserialize` have nothing to call.

**Gap 3 — `PodString::as_str_unchecked` is a footgun.**

It's `unsafe` and requires the caller to know the bytes are valid UTF-8. But because `PodString` implements `Pod`, anyone can obtain a `&PodString` from arbitrary bytes via bytemuck and then call `as_str_unchecked` — the unsafe contract is trivially violated. Zeropod's model makes `as_str()` safe _after_ `from_bytes` because `validate_ref` gated UTF-8 at the boundary.

**Gap 4 — No golden layout tests for derived structs.**

Pina has compile-time asserts, but no tests that pin the _derived_ `#[account]` struct layouts (sizes, alignments, field offsets) as golden values. A field reorder or type-width change that keeps the asserts passing (e.g. swapping two same-size fields) would silently change the on-chain ABI. Zeropod's `layout_golden.rs` pins exact offsets.

**Gap 5 — No roundtrip byte-stability tests.**

Zeropod's `fixed_byte_stability` proves identical writes produce identical bytes. Pina has roundtrip tests but nothing that pins byte-for-byte stability across two independent write passes.

**Gap 6 — No instruction ABI parity tests.**

Zeropod proves account and instruction compact layouts produce identical tail bytes. Pina's `#[account]` and `#[instruction]` macros generate separate layouts with no cross-check that a field serialized in an account matches the same field serialized in an instruction.

**Gap 7 — Fuzz targets are shallow.**

`pina_fuzz` targets only check "doesn't panic":

```rust
fuzz_target!(|data: &[u8]| {
	let _ = CounterState::try_from_bytes(data);
});
```

They don't fuzz the pod primitives themselves (`PodString`, `PodVec`, `PodOption`), and they don't assert semantic properties (e.g. "if `try_from_bytes` returns `Ok`, the result must be canonical").

(Note: `crates/pina_pod_primitives/tests/fuzz_pod.rs` does proptest the numeric types and `PodBool` canonicality — but not `PodString`/`PodVec`/ `PodOption`, and it asserts "never panics" rather than "rejects invalid".)

**Gap 8 — Kani proofs don't cover validation.**

The existing Kani proofs (in-crate `option.rs`/`string.rs`/`vec.rs` plus `tests/kani_harnesses.rs`) cover roundtrips, checked/saturating arithmetic, bitwise ops, ordering, overflow rejection, and len clamping. There is no Kani proof that:

- `validate` rejects every invalid input class,
- `from_bytes` never returns a reference to invalid data,
- `PodBool` canonicality is preserved through roundtrips.

**Gap 9 — No Miri coverage for the pod primitives' unsafe code.**

Miri runs only on `miri_loader_guards`. The primitives' `unsafe` code (`MaybeUninit` handling in `PodOption`/`PodString`/`PodVec`, raw-pointer slices in `as_str_unchecked`/`as_bytes`) is not Miri-checked.

**Gap 10 — No golden layout tests for derived structs.**

Pina has compile-time asserts, but no tests that pin the _derived_ `#[account]` struct layouts (sizes, alignments, field offsets) as golden values. A field reorder or type-width change that keeps the asserts passing (e.g. swapping two same-size fields) would silently change the on-chain ABI. Zeropod's `layout_golden.rs` pins exact offsets.

**Gap 11 — No roundtrip byte-stability tests.**

Zeropod's `fixed_byte_stability` proves identical writes produce identical bytes. Pina has roundtrip tests but nothing that pins byte-for-byte stability across two independent write passes.

**Gap 12 — No instruction ABI parity tests.**

Zeropod proves account and instruction compact layouts produce identical tail bytes. Pina's `#[account]` and `#[instruction]` macros generate separate layouts with no cross-check that a field serialized in an account matches the same field serialized in an instruction.

---

## 3. Recommendations

All recommendations are **additive** (new traits, new methods, new tests) so they cannot break existing behavior. The one behavior-changing item (Recommendation 2) is explicitly gated behind an opt-in flag.

### Layer 1 — Verifiably proving a program is secure (testing)

**R1. Add a `PodValidate` trait to `pina_pod_primitives` (mirror of `ZcValidate`).**

```rust
/// Validation trait for stored (pod) types.
/// Each pod type knows how to validate itself.
pub trait PodValidate: Copy {
	fn validate_ref(&self) -> Result<(), PodCollectionError>;
}
```

Implement for every existing type:

- `PodBool` → reject `self.0 > 1` (new `InvalidBool` error variant)
- `PodString<N, PFX>` → reject `len > N`, reject invalid UTF-8
- `PodVec<T, N, PFX>` → reject `len > N`, recurse into elements
- `PodOption<T>` → reject tag `> 1`, recurse into inner if `Some`
- Numeric types, `[u8; N]` → trivially `Ok(())`

This is purely additive: existing code that never calls `validate_ref` is unaffected.

**R2. Add a validated deserialization path (opt-in, non-breaking).**

Add `AccountDeserialize::try_from_bytes_validated` (or a `validate()` method on account types) that calls `PodValidate::validate_ref` after the discriminator check and before returning the reference:

```rust
fn try_from_bytes_validated(data: &[u8]) -> Result<&Self, ProgramError> {
	let account = Self::try_from_bytes(data)?;
	<Self as PodValidate>::validate_ref(account)?; // NEW
	Ok(account)
}
```

Keep the existing `try_from_bytes` behavior unchanged for compatibility. Programs that want boundary validation opt in. (See §4 for the migration path to making this the default.)

**R3. Golden layout tests for derived account/instruction structs.**

Add a `layout_golden` test module (mirroring Zeropod's) that pins:

- `size_of::<T>()` for every `#[account]` type in the examples,
- `align_of::<T>() == 1`,
- exact field offsets via pointer arithmetic (as Zeropod's `golden_fixed_field_offsets` does).

This catches silent ABI changes (field reorder, width change) that compile-time asserts miss.

**R4. Roundtrip byte-stability tests.**

For each pod type and each example `#[account]` type: write the same values into two independent buffers and assert the buffers are byte-identical. This proves serialization is deterministic — critical for on-chain state.

**R5. Property-based tests for the primitives (use the already-declared `proptest`).**

Add `proptest` cases to `pina_pod_primitives/src/tests/`:

- Arbitrary byte slices never panic when cast to each pod type and validated.
- `validate_ref` accepts iff the value is canonical (for `PodBool`).
- `try_set`/`push` roundtrips: `set(s)` then `as_bytes() == s.as_bytes()`.
- UTF-8 boundary cases for `PodString` (multi-byte chars, truncation at char boundaries — Zeropod's `podstring_truncate_snaps_to_char_boundary`).

**R6. Fuzz the primitives directly.**

Add `pina_fuzz` targets for `PodString`, `PodVec`, `PodOption`, `PodBool` that assert semantic properties, not just "no panic":

- "if `try_from_bytes` returns `Ok`, then `validate_ref` must return `Ok`" (this is the key invariant — it's the fuzzable form of "validation is load-bearing").

**R7. Kani proofs for validation rejection.**

Add `#[kani::proof]` harnesses proving:

- `PodBool::validate_ref` rejects every byte `> 1` and accepts `0`/`1`,
- `PodString::validate_ref` rejects every `len > N`,
- `PodVec::validate_ref` rejects every `len > N`,
- `PodOption::validate_ref` rejects every tag `> 1`,
- `try_from_bytes_validated` never returns `Ok` for invalid input.

These are small, bounded proofs — ideal for Kani.

### Layer 2 — Building it into the framework (framework security)

**R8. Make the `#[account]` macro generate validation.**

Extend `pina_macros` so `#[account]` generates a `PodValidate` impl that delegates to each field's `validate_ref` (exactly as Zeropod's derive does). This is the mechanism that makes validation _automatic_ for every account type — users can't forget it.

**R9. Wire validation into the loader path.**

The guard-backed loaders in `crates/pina/src/impls.rs` (`Ref<T>`/`RefMut<T>`) should call `PodValidate::validate_ref` when constructing typed references. This is where the framework's own security lives — the loaders are the chokepoint through which all account data flows.

**R10. Miri coverage for the primitives' unsafe code.**

Extend the Miri job (or add a second Miri test) to run the pod primitives' unsafe paths: `MaybeUninit` handling in `PodOption`/`PodString`/`PodVec`, `as_str_unchecked`, `as_bytes`, `try_set`/`push` with overlapping buffers. Miri with `-Zmiri-tree-borrows` will catch aliasing violations in the raw-pointer code.

**R11. Extend mutation testing to validation paths.**

`mutants.toml` already targets `pina_pod_primitives`. Ensure the new `validate_ref` impls are covered by tests that would fail if a validation check were removed (e.g. deleting the `byte > 1` check in `PodBool` must fail a test). This is the "does the test suite actually prove security" question — mutation testing answers it mechanically.

**R12. Document the safety contract (mirror Zeropod's `ZcElem` docs).**

Add a `# Safety` section to the `PodValidate` trait documenting the four requirements (alignment 1, no padding, validity for every bit pattern, validation is load-bearing). This makes the contract auditable and gives reviewers a checklist.

### Layer 3 — Making it available to users

**R13. Public `PodValidate` trait + `validate()` on account types.**

Users implement `PodValidate` for their own types (or get it for free from `#[account]`). Expose a `validate()` method on deserialized account references so users can call it explicitly in instruction handlers.

**R14. Example programs demonstrating the pattern.**

Add a `validated_accounts` example (or extend an existing one) showing:

- `#[account]` with a `PodBool` field — demonstrate that a non-canonical byte is rejected at the boundary,
- `#[account]` with a `PodString` — demonstrate invalid UTF-8 rejection,
- the `try_from_bytes_validated` pattern in an instruction handler.

**R15. Security-model documentation update.**

Update `docs/src/security-model.md` and the `security/` guide with a new section: "Content validation" — explaining that `Pod` guarantees memory safety, `PodValidate` guarantees semantic validity, and both are needed.

**R16. CI wiring.**

- Add the new Kani proofs to the existing `kani.yml` (it already triggers on `crates/pina_pod_primitives/**`).
- Add the primitives Miri test to the `miri` CI job.
- Add the golden-layout and byte-stability tests to the standard `test:all` lane.

---

## 4. BIS compatibility and migration safety

**Note on "BIS":** I could not find any reference to "BIS" in the repository (source, docs, CI, devenv, git history). If BIS is a downstream project or a specific feature, please point me at it and I will verify each recommendation against it. In the meantime, every recommendation above is designed to be non-breaking:

- **R1 (new trait)** — purely additive. No existing code changes behavior.
- **R2 (validated path)** — new method; existing `try_from_bytes` unchanged. This is the only recommendation that _could_ change behavior, and it is opt-in. The migration path to making it the default:
  1. Ship `PodValidate` + `try_from_bytes_validated` (additive).
  2. Add `#[account(validate)]` opt-in attribute that makes the macro generate a validating `try_from_bytes`.
  3. After a release cycle, flip the default and keep the non-validating path as `try_from_bytes_unchecked` for migration.
- **R3-R7 (tests)** — new test files only.
- **R8-R9 (macro + loaders)** — the macro change is additive (new impls); the loader change should be gated behind the same opt-in flag as R2 until the default flips.
- **R10-R16 (tooling/docs)** — no runtime behavior change.

The one semantic fix that deserves special care is **`PodBool` canonicality**. Today `PodBool(2)` is accepted and converts to `true`. If any existing program relies on that (e.g. writes `2` as a "truthy" value), making validation reject it would change behavior. The safe sequence is:

1. Add `validate_ref` rejecting non-canonical bytes (additive — nothing calls it yet).
2. Add the opt-in validated deserialization path.
3. Only after confirming no downstream program writes non-canonical bools, flip the default.

---

## 5. Priority order

| Priority | Item                                      | Effort       | Impact                    |
| -------- | ----------------------------------------- | ------------ | ------------------------- |
| P0       | R1: `PodValidate` trait + impls           | Small        | Unlocks everything else   |
| P0       | R7: Kani proofs for validation rejection  | Small        | Proves the gate works     |
| P0       | R5: proptest for primitives               | Small        | Uses already-declared dep |
| P1       | R2: `try_from_bytes_validated` (opt-in)   | Small        | Boundary validation       |
| P1       | R8: `#[account]` generates `PodValidate`  | Medium       | Automatic for users       |
| P1       | R3: golden layout tests                   | Medium       | Pins on-chain ABI         |
| P1       | R4: byte-stability tests                  | Small        | Determinism proof         |
| P2       | R6: fuzz primitives with semantic asserts | Medium       | Stronger fuzzing          |
| P2       | R10: Miri for primitives                  | Medium       | UB coverage               |
| P2       | R9: loaders call validate                 | Medium       | Framework chokepoint      |
| P3       | R11-R16: mutation, docs, examples, CI     | Small-Medium | Hardening + adoption      |

## 7. Appendix: the direct-struct pattern (no `Zc` companion)

**Verified experimentally** (see `/tmp/zeropod/zeropod/tests/direct_struct.rs`, 5/5 passing): the `Zc` companion struct is an _ergonomic convenience_ of the derive (so you can write `u64` instead of `PodU64`), **not a hard requirement** of the trait model. If you write pod types directly, `map_to_pod_type` is the identity mapping and the companion is a pure duplicate.

```rust
#[repr(C)]
#[derive(Copy, Clone)]
struct DirectAccount {
	pub amount: PodU64,
	pub active: PodBool,
	pub name: PodString<32>,
	pub tags: PodVec<u8, 4>,
}

// 1. ZcValidate — delegate to each field.
impl ZcValidate for DirectAccount {
	fn validate_ref(value: &Self) -> Result<(), ZeroPodError> {
		PodU64::validate_ref(&value.amount)?;
		PodBool::validate_ref(&value.active)?;
		PodString::<32>::validate_ref(&value.name)?;
		PodVec::<u8, 4>::validate_ref(&value.tags)?;
		Ok(())
	}
}

// 2. ZcElem — unsafe, with the 4 documented safety requirements.
// SAFETY: align 1 (const-asserted), no padding, every bit pattern valid,
// validate_ref is load-bearing.
unsafe impl ZcElem for DirectAccount {}

// 3. ZeroPodSchema.
impl ZeroPodSchema for DirectAccount {
	const LAYOUT: LayoutKind = LayoutKind::Fixed;
}

// 4. ZeroPodFixed — `type Zc = Self` is the key trick.
impl ZeroPodFixed for DirectAccount {
	type Zc = DirectAccount;

	const SIZE: usize = size_of::<DirectAccount>();

	fn from_bytes(data: &[u8]) -> Result<&Self::Zc, ZeroPodError> {
		Self::validate(data)?;
		Ok(unsafe { &*(data.as_ptr() as *const Self::Zc) })
	}
	// ... from_bytes_mut, validate, unchecked variants
}

// 5. ZcField — so DirectAccount can be nested in other zeropod structs.
impl ZcField for DirectAccount {
	type Pod = DirectAccount;

	const POD_SIZE: usize = size_of::<DirectAccount>();
}
```

`DirectAccount::from_bytes(&data)` returns `&DirectAccount` directly — no companion. Verified: roundtrip works, and invalid bool / invalid UTF-8 / overlength vec / truncated buffers are all rejected at the boundary.

**The catch:** ~50 lines of boilerplate per struct. Three ways to avoid writing it by hand:

1. **Pina's `#[account]` macro generates these impls** — the natural path. The macro already emits alignment asserts and derives; adding the zeropod trait impls is a direct extension.
2. **A small custom derive** in `pina_macros` (e.g. `#[derive(ZeroPodDirect)]`) that generates the impls for the struct itself.
3. **Ask zeropod to add a "direct" mode** to `#[derive(ZeroPod)]` — detect that all fields are already pod types and skip the companion.

**Compact-layout caveat:** the direct pattern works cleanly for the _fixed_ layout (inline pod types). Compact layout (header + tail) generates a separate header struct and tail accessors; a direct struct would need manual header/tail handling. Since the goal is to define `PodString`/`PodVec` inline, the fixed layout is the relevant one.

---

## 8. Appendix: key file references

| Concern          | Pina                                                                         | Zeropod                                                        |
| ---------------- | ---------------------------------------------------------------------------- | -------------------------------------------------------------- |
| Pod types        | `crates/pina_pod_primitives/src/{pod_bool,pod_numeric,option,string,vec}.rs` | `zeropod/src/pod/*.rs`                                         |
| Validation trait | _(missing)_                                                                  | `zeropod/src/traits.rs` (`ZcValidate`, `ZcElem`)               |
| Deserialization  | `crates/pina/src/traits.rs` (`AccountDeserialize`)                           | `zeropod-derive/src/fixed.rs` (`from_bytes` = validate + cast) |
| Account macro    | `crates/pina_macros/src/lib.rs` (`account_impl`)                             | `zeropod-derive/src/fixed.rs`                                  |
| Validation tests | _(missing — see `gap_demo.rs` evidence)_                                     | `zeropod/tests/validation.rs`                                  |
| Golden layout    | compile-time asserts only                                                    | `zeropod/tests/layout_golden.rs`                               |
| Roundtrip        | `crates/pina_pod_primitives/src/tests/*.rs`                                  | `zeropod/tests/roundtrip.rs`                                   |
| ABI parity       | _(missing)_                                                                  | `zeropod/tests/instruction_abi_parity.rs`                      |
| Kani proofs      | `option.rs`, `string.rs`, `vec.rs` + `tests/kani_harnesses.rs`               | _(none — Zeropod relies on tests)_                             |
| Proptest         | `tests/fuzz_pod.rs` (numeric + PodBool only)                                 | _(none)_                                                       |
| Fuzz             | `crates/pina_fuzz/fuzz/fuzz_targets/*.rs`                                    | _(none)_                                                       |
| Miri             | `crates/pina/tests/miri_loader_guards.rs`                                    | _(none)_                                                       |
| CI               | `.github/workflows/{kani,ci,mutants-*}.yml`                                  | _(none published)_                                             |
