# `pina_pod_primitives`

Alignment-safe primitive POD wrappers used by Pina and generated Codama Rust clients.

[![Crates.io][crate-image]][crate-link] [![Docs.rs][docs-image]][docs-link] [![CI][ci-status-image]][ci-status-link] [![License][unlicense-image]][unlicense-link] [![codecov][codecov-image]][codecov-link]

This crate provides `PodBool`, `PodU16`, `PodI16`, `PodU32`, `PodI32`, `PodU64`, `PodI64`, `PodU128`, and `PodI128` for use in `#[repr(C)]` zero-copy layouts.

## Installation

```bash
cargo add pina_pod_primitives
```

## Types

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
