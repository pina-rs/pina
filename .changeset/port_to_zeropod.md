---
pina: major
pina_cli: major
pina_codama_renderer: major
pina_macros: major
---

# Port to zeropod

Replace pina's own pod primitives (`pina_pod_primitives`, based on `bytemuck`) with [zeropod](https://crates.io/crates/zeropod) as the primitives library. This is a major change to the account model:

- **`pina_pod_primitives` is deleted.** Its types (`PodU64`, `PodBool`, `PodString`, `PodVec`, `PodOption`) are re-exported from `zeropod` with the same names.
- **`bytemuck::Pod` and `Zeroable` are removed from Pina's public account model.** `pina::Pod` / `pina::Zeroable` re-exports are gone; transitive Solana dependencies may still use bytemuck internally.
- **`AccountDeserialize` is replaced by `PinaAccount`.** Account schemas derive `zeropod::ZeroPod`; loaders return the generated `AccountZc` companion rather than reinterpreting the native schema as account memory.
- **Content validation at the deserialization boundary.** Non-canonical `PodBool` bytes, invalid UTF-8 in `PodString`, and overlength `PodVec` prefixes are now rejected by `try_from_bytes` / `as_account` instead of silently accepted.
- **`PodString::as_str()` is now safe** (validated at the boundary); `as_str_unchecked` / `try_as_str` are gone.
- **`PodU64::from_primitive` is replaced by `From<u64>`** (zeropod's API). `from_primitive` callers should use `PodU64::from(n)` or `n.into()`.
- **`PodBool::from_bool` is replaced by `From<bool>`**.
- **`PodVec::as_mut_slice` is renamed to `as_slice_mut`** (zeropod's API).
- The `#[account]`, `#[instruction]`, and `#[event]` macros derive `zeropod::ZeroPod` and expose checked `try_from_bytes` / `initialize` helpers. Pina no longer generates `to_bytes()` or any whole-object serialization API.
- Native schema fields use ordinary Rust values. Runtime loaders return the generated zero-copy representation, whose fields are read and changed with zeropod's `get` / `set` and collection accessors.
- `PinaSerialize`, Pina's generic `InstructionBuilder`, the custom `PodEnum` derive, and Pina's generic raw-cast helpers are removed. Zeropod owns the byte-to-view boundary.
- The `#[discriminator]` macro no longer emits `unsafe impl Pod` / `unsafe impl Zeroable` for enums.
- Codama-generated clients validate zeropod fields before zero-copy access. Instruction construction owns a fully initialized buffer and never exposes a schema or storage view as raw bytes.
