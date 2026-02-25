# `pina_codama_renderer`

Repository-local Codama Rust renderer that generates Pina-style bytemuck models and discriminator-first layouts from Codama JSON IDLs.

[![CI][ci-status-image]][ci-status-link] [![License][unlicense-image]][unlicense-link]

> **Note:** This crate is not published to crates.io. It is used internally by the `pina codama generate` workflow.

## Usage

```bash
cargo run --manifest-path ./crates/pina_codama_renderer/Cargo.toml -- \
  --idl ./idls/my_program.json \
  --output ./clients/rust
```

## What It Generates

- Discriminator-first `#[repr(C)]` account/instruction/event structs
- `bytemuck::Pod` + `bytemuck::Zeroable` derives (no `borsh` serialization)
- `pina_pod_primitives` types for alignment-safe integer fields
- Type-safe instruction builders

## Constraints

The renderer only supports fixed-size layouts. The following Codama patterns will produce explicit errors:

- Variable-length strings/bytes
- Big-endian numbers
- Floats
- Non-UTF8 constant byte seeds
- Non-fixed arrays

[ci-status-image]: https://github.com/pina-rs/pina/workflows/ci/badge.svg
[ci-status-link]: https://github.com/pina-rs/pina/actions?query=workflow:ci
[unlicense-image]: https://img.shields.io/badge/license-Unlicense-blue.svg?style=flat-square
[unlicense-link]: https://opensource.org/license/unlicense
