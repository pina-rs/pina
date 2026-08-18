---
pina: major
pina_macros: major
pina_pod_primitives: major
---

# Revert zeropod port; sound pod primitives; compact accounts

Reverts the zeropod-based primitives port and returns to bytemuck as the zero-copy backing layer, with three improvements:

## 1. Sound pod primitives (no `MaybeUninit`)

`PodString`, `PodVec`, and `PodOption` no longer use `MaybeUninit` capacity arrays. The data is stored in plain `[u8; N]` / `[T; N]` arrays where every byte is always initialized (capacity slots beyond the logical length hold stale-but-initialized bytes that validation never interprets). This makes the `bytemuck::Pod` / `Zeroable` impls sound and makes `to_bytes` / `bytes_of` safe to call on structs containing these types.

- `PodString<N, PFX>`: `data: [u8; N]`
- `PodVec<T, N, PFX>`: `data: [T; N]`
- `PodOption<T>`: `value: T` (zeroed when `None`), tag byte unchanged
- `get` / `as_ref` / `as_mut` / `set` / `assume_init` are now safe
- New `From<&str>` for `PodString` and `From<&[T]>` for `PodVec` (panic on capacity overflow; use `try_set` / `try_push` for untrusted input)

## 2. `zeroed()` preserves the discriminator

`zeroed()` now zeroes only the bytes after the discriminator field instead of the whole struct, so a closed account still carries its type discriminator. This is more performant than save/restore and keeps the account deserializable as its own type after zeroing.

## 3. Compact accounts (`#[account(compact)]`)

New compact account mode: a fixed-size header (discriminator + inline pod fields) followed by variable-length tail segments (`PodString`, `PodVec`, `PodOption`) in the account buffer. The on-chain size is `HEADER_SIZE + used tail bytes`, so rent is paid only for stored data.

- `{Name}Header`: `#[repr(C)]` `Pod` struct of the inline fields
- `impl CompactAccount`: `validate` (walks the buffer with length-prefix bounds checks), `header`, `header_mut`
- `{Name}Ref` / `{Name}RefMut`: validated views with tail accessors
- `to_compact_bytes` / `compact_size`: compact serialization

## 4. Security fix

Removed the unsound `unsafe impl Pod` / `Zeroable` from discriminator enums. A fieldless enum with explicit discriminants has restricted validity, so casting arbitrary bytes to it was undefined behavior. Discriminators are read via `IntoDiscriminator`, which validates through `TryFrom`.
