# `pina_pod_primitives`

<br>

Alignment-safe primitive POD wrappers and fixed-capacity collection types used by Pina and generated Codama Rust clients.

[![Crates.io][crate-image]][crate-link] [![Docs.rs][docs-image]][docs-link] [![CI][ci-status-image]][ci-status-link] [![License][unlicense-image]][unlicense-link] [![codecov][codecov-image]][codecov-link]

This crate provides `PodBool`, `PodU16`, `PodI16`, `PodU32`, `PodI32`, `PodU64`, `PodI64`, `PodU128`, and `PodI128` for use in `#[repr(C)]` zero-copy layouts, plus fixed-capacity collection types `PodOption<T>`, `PodString<N, PFX>`, and `PodVec<T, N, PFX>`.

## Arithmetic

<!-- {=podArithmeticDescription} -->

Arithmetic operators (`+`, `-`, `*`) on Pod **integer** types use **wrapping** semantics in release builds for CU efficiency and **panic on overflow** in debug builds. Use `checked_add`, `checked_sub`, `checked_mul`, `checked_div` where overflow must be detected in all build profiles.

Each Pod integer type provides `ZERO`, `MIN`, and `MAX` constants.

<!-- {/podArithmeticDescription} -->

## Installation

<br>

```bash
cargo add pina_pod_primitives
```

## Integer types

<br>

<!-- {=podTypesTable} -->

| Type      | Wraps  | Size     |
| --------- | ------ | -------- |
| `PodBool` | `bool` | 1 byte   |
| `PodU16`  | `u16`  | 2 bytes  |
| `PodI16`  | `i16`  | 2 bytes  |
| `PodU32`  | `u32`  | 4 bytes  |
| `PodI32`  | `i32`  | 4 bytes  |
| `PodU64`  | `u64`  | 8 bytes  |
| `PodI64`  | `i64`  | 8 bytes  |
| `PodU128` | `u128` | 16 bytes |
| `PodI128` | `i128` | 16 bytes |

All types are `#[repr(transparent)]` over byte arrays (or `u8` for `PodBool`) and implement `bytemuck::Pod` + `bytemuck::Zeroable`.

<!-- {/podTypesTable} -->

## Collection types

<br>

<!-- {=podCollectionTypesTable} -->

| Type                       | Purpose                | Layout                                    |
| -------------------------- | ---------------------- | ----------------------------------------- |
| `PodOption<T: Pod>`        | Fixed-size `Option<T>` | 1-byte discriminant + `T`                 |
| `PodString<N, PFX=1>`      | Fixed-capacity string  | `PFX`-byte length prefix + `N` data bytes |
| `PodVec<T: Pod, N, PFX=2>` | Fixed-capacity vec     | `PFX`-byte length prefix + `N` elements   |

All collection types are `#[repr(C)]`, alignment-1, and implement `bytemuck::Pod` + `bytemuck::Zeroable`. Length prefixes (`PFX`) default to 1 byte for strings (max 255) and 2 bytes for vectors (max 65 535 elements).

<!-- {/podCollectionTypesTable} -->

<!-- {=podCollectionDescription} -->

Collection types store data inline with a length prefix, enabling zero-copy access inside `#[repr(C)]` account structs. Overflow is detected at insertion time — `try_set` / `try_push` return `Err(PodCollectionError::Overflow)` when capacity is exceeded.

`PodString` provides UTF-8 validation via `try_as_str()`, while `PodVec` offers slice-based access via `as_slice()` / `as_mut_slice()`. `PodOption` mirrors the `Option<T>` API with `get()`, `set()`, and `clear()`.

<!-- {/podCollectionDescription} -->

[crate-image]: https://img.shields.io/crates/v/pina_pod_primitives.svg?style=flat-square
[crate-link]: https://crates.io/crates/pina_pod_primitives
[docs-image]: https://docs.rs/pina_pod_primitives/badge.svg
[docs-link]: https://docs.rs/pina_pod_primitives/
[ci-status-image]: https://github.com/pina-rs/pina/workflows/ci/badge.svg
[ci-status-link]: https://github.com/pina-rs/pina/actions?query=workflow:ci
[unlicense-image]: https://img.shields.io/badge/license-Unlicense-blue.svg?style=flat-square
[unlicense-link]: https://opensource.org/license/unlicense
[codecov-image]: https://codecov.io/github/pina-rs/pina/graph/badge.svg?token=87K799Q78I
[codecov-link]: https://codecov.io/github/pina-rs/pina
