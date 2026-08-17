---
pina: major
pina_cli: minor
pina_codama_renderer: minor
pina_macros: major
---

# Port to zeropod

Replace pina's own pod primitives (`pina_pod_primitives`, based on `bytemuck`) with [zeropod](https://crates.io/crates/zeropod) as the primitives library. This is a major change to the account model:

- **`pina_pod_primitives` is deleted.** Its types (`PodU64`, `PodBool`, `PodString`, `PodVec`, `PodOption`) are re-exported from `zeropod` with the same names.
- **`bytemuck`, `Pod`, and `Zeroable` are removed** from the codebase. `pina::Pod` / `pina::Zeroable` re-exports are gone.
- **`AccountDeserialize` is replaced by `PinaAccount`** (`validate`, `try_from_bytes`, `try_from_bytes_mut`). The `#[account]` macro generates it, along with the zeropod trait impls (`ZcValidate`, `ZcElem`, `ZeroPodSchema`, `ZeroPodFixed` with `type Zc = Self`, `ZcField`) using the direct-struct pattern — no companion struct.
- **Content validation at the deserialization boundary.** Non-canonical `PodBool` bytes, invalid UTF-8 in `PodString`, and overlength `PodVec` prefixes are now rejected by `try_from_bytes` / `as_account` instead of silently accepted.
- **`PodString::as_str()` is now safe** (validated at the boundary); `as_str_unchecked` / `try_as_str` are gone.
- **`PodU64::from_primitive` is replaced by `From<u64>`** (zeropod's API). `from_primitive` callers should use `PodU64::from(n)` or `n.into()`.
- **`PodBool::from_bool` is replaced by `From<bool>`**.
- **`PodVec::as_mut_slice` is renamed to `as_slice_mut`** (zeropod's API).
- The `#[account]` macro no longer emits `Pod`/`Zeroable` derives or the `#[bytemuck(...)]` attribute; `zeroed()` / `to_bytes()` use unsafe pointer operations (sound: all fields are align-1 pod types, compile-time asserted).
- The `#[discriminator]` macro no longer emits `unsafe impl Pod` / `unsafe impl Zeroable` for enums.
- Codama-generated clients use manual zero-copy casts instead of `bytemuck`.
