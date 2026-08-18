# Security Audit — bytemuck Revert, No-MaybeUninit Primitives, Compact Mode

_Date: 2026-08-18_ _Scope: the `explore/bytemuck-revert` branch (revert of the zeropod port, pod primitives without `MaybeUninit`, discriminator-preserving `zeroed()`, compact account mode)._

## Summary

The audit reviewed every `unsafe` block and `unsafe impl` in the workspace, the macro-generated code for `#[account]` / `#[instruction]` / `#[event]` / `#[discriminator]` / `#[pda]` / `#[account(compact)]`, the account loaders, the token loaders, and the pod primitives. Miri was run on the pod primitives and the compact account tests.

**One soundness bug was found and fixed** (unsound `Pod` impl on discriminator enums). All other findings are either sound-by-construction, documented semantic limitations, or recommended practices.

## Findings

### FIXED — [High] Unsound `unsafe impl Pod` on discriminator enums

**Location:** `pina_macros` `#[discriminator]` attribute macro.

**Issue:** The macro generated `unsafe impl Pod` and `unsafe impl Zeroable` for the discriminator enum. A fieldless enum with explicit discriminants (e.g. `#[repr(u8)] enum MyAccount { ConfigState = 0, GameState = 1 }`) has **restricted validity** — only the declared discriminants are valid values. bytemuck's `Pod` requires _every_ bit pattern to be a valid value, so `bytemuck::from_bytes::<MyAccount>(&[2])` would construct an invalid enum value, which is undefined behavior on read.

**Impact:** Any user code calling `bytemuck::from_bytes` (or `pod_from_bytes`) on a discriminator enum with an undeclared discriminant would trigger UB. The pina codebase itself never cast discriminator enums from bytes (they are read via `IntoDiscriminator`, which validates through `TryFrom`), so the bug was latent, but the public `unsafe impl` was a soundness hole.

**Fix:** Removed the `Pod`/`Zeroable` impls. Discriminators are read via `IntoDiscriminator::discriminator_from_bytes`, which validates the primitive through `TryFrom<primitive>` and returns `Err(InvalidDiscriminator)` for undeclared values. A comment documents why the impls must not be added back.

### [Info] `PodBool` accepts non-canonical values

**Location:** `pina_pod_primitives/src/pod_bool.rs`.

**Issue:** `PodBool` is `#[repr(transparent)]` over `u8`. Values 2–255 are valid `PodBool` values (sound — no UB), but `bool::from(pod_bool)` maps any non-zero byte to `true`. Two non-canonical values representing the same logical boolean may fail `PartialEq`.

**Assessment:** Sound (no UB). The `is_canonical()` method is provided for boundary validation. **Recommended practice:** call `is_canonical()` on `PodBool` fields loaded from untrusted account data. This is a semantic validation gap inherent to the bytemuck model (no load-bearing content validation), not a soundness issue.

### [Info] `PodString::as_str_unchecked` requires caller validation

**Location:** `pina_pod_primitives/src/string.rs`.

**Issue:** `as_str_unchecked` returns `&str` without UTF-8 validation. If a `PodString` loaded from untrusted account data contains invalid UTF-8 and `as_str_unchecked` is called, the resulting `&str` is invalid (UB on use).

**Assessment:** Documented unsafe contract (same pattern as `str::from_utf8_unchecked`). The safe `try_as_str()` validates. The `profile_program` example demonstrates the correct pattern (validate UTF-8 before storing). **Recommended practice:** always use `try_as_str()` for account-derived strings.

### [Info] Release-mode arithmetic wraps silently

**Location:** `pina_pod_primitives/src/macros.rs` (`impl_pod_arithmetic`).

**Issue:** In release builds, `+`/`-`/`*` use `wrapping_*` arithmetic (a deliberate CU-efficiency choice for Solana); debug builds panic on overflow. `/` panics on division by zero in both modes.

**Assessment:** Documented behavior. The `checked_*`/`saturating_*` methods are provided for explicit handling. **Recommended practice:** use `checked_*` for untrusted arithmetic; avoid `Div` on untrusted denominators (a panic aborts the program).

### [Info] Fixed-layout accounts do not validate field content

**Location:** `AccountDeserialize` (blanket impl over `HasDiscriminator + Pod`).

**Issue:** `try_from_bytes` validates the discriminator and exact size, but not field content (e.g. a `PodString` with invalid UTF-8, a `PodVec` with a length prefix exceeding capacity, a non-canonical `PodBool`). This is the inherent tradeoff of the bytemuck model: `Pod` guarantees bit-pattern validity, not semantic validity.

**Assessment:** Sound (no UB — all fields are `Pod` with no uninitialized memory after the `MaybeUninit` removal). Semantic validation is the caller's responsibility via `try_as_str`, `is_canonical`, etc. The compact mode (`#[account(compact)]`) **does** validate tail content at the boundary (length prefixes, UTF-8, option tags), which is the load-bearing-validation model for variable-length data.

### [Info] `From<&str>` / `From<&[T]>` panic on capacity overflow

**Location:** `pina_pod_primitives/src/string.rs`, `vec.rs`.

**Issue:** The new `From` impls panic if the input exceeds capacity.

**Assessment:** These are construction conveniences for developer-known data; the panic is a programmer error, not reachable from untrusted input. The `try_set`/`try_push` variants are the fallible path for untrusted data.

## Soundness verification

### Pod primitives (no `MaybeUninit`)

- `PodString<N, PFX>` = `len: [u8; PFX]` + `data: [u8; N]` — every byte initialized, align 1, no padding. `unsafe impl Pod`/`Zeroable` sound.
- `PodVec<T, N, PFX>` = `len: [u8; PFX]` + `data: [T; N]` (`T: Pod`) — same.
- `PodOption<T>` = `tag: u8` + `value: T` (`T: Pod`) — `None` stores `T::zeroed()` (valid because `T: Pod` implies `T: Zeroable`).
- `as_slice`/`as_bytes`/`get`/`get_mut` read only `[..len]` (in bounds, initialized). `try_set`/`try_push` bounds-check before writing.
- **Miri: 117 lib tests pass with no UB.**

### Compact mode (`#[account(compact)]`)

- `validate` walks the buffer with bounds checks on every segment: length prefix in range, `len <= max`, `payload + len <= data.len()`, UTF-8 for strings, tag ∈ {0,1} for options.
- `header`/`header_mut` cast only after `validate` guarantees `data.len() >= HEADER_SIZE`; the header is `#[repr(C)]` align-1 pod fields.
- `{Name}Ref`/`{Name}RefMut` constructors validate before exposing accessors; accessors re-read the same length prefixes `validate` checked (data is immutable for `Ref`, and `RefMut` accessors take `&self`).
- `to_compact_bytes` serializes only used bytes; `compact_size` matches (verified by tests).
- **Miri: 12 compact tests pass with no UB.**

### Loaders and validation

- `as_account`/`as_account_mut`: owner + exact-length checks, then `try_from_bytes` (discriminator + size). Borrow guards keep the runtime borrow alive.
- Token loaders: owner-set + length checks (exact for SPL, minimum for token-2022) before the unchecked cast.
- `parse_instruction`: program-id check, length check, then `discriminator_from_bytes` (validates enums via `TryFrom`).
- `assert_seeds`/`validate_seeds`: PDA derivation + address comparison.
- `zeroed()`: zeroes only bytes after the discriminator (all-zero is valid for every pod field type); the discriminator is preserved.

## Conclusion

The `MaybeUninit` removal makes the pod primitives sound under bytemuck's `Pod` contract (no uninitialized memory, every bit pattern valid), which was the audit's original complaint. The discriminator-enum `Pod` impl was the one genuine soundness bug found in this review and is fixed. The remaining findings are documented semantic limitations of the bytemuck model (content validation is caller-side) and recommended practices, none of which are undefined behavior.
