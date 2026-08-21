# `pina_codama_renderer`

<p align="center">
	<img src="https://raw.githubusercontent.com/pina-rs/pina/main/.github/assets/logo.png" alt="The Pina logo: a low-poly origami pineapple" width="140">
</p>

<br>

Codama Rust renderer that generates Pina-style validated zeropod models and discriminator-first layouts from Codama JSON IDLs. It is not published to crates.io and is used internally by the `pina codama generate` workflow.

[![CI][ci-status-image]][ci-status-link] [![License][license-image]][license-link]

## Usage

<br>

```bash
cargo run --manifest-path ./crates/pina_codama_renderer/Cargo.toml -- \
  --idl ./idls/my_program.json \
  --output ./clients/rust
```

## What It Generates

<br>

- Native account, instruction, event, and defined-type schemas deriving `pina::ZeroPod`
- Discriminator-first generated storage views with recursive zeropod validation
- Checked account initialization from caller-owned buffers
- Type-safe instruction builders that own and consume their initialized wire buffers without exposing object representations

## Constraints

<br>

The renderer only supports fixed-size layouts. The following Codama patterns will produce explicit errors:

- Variable-length strings/bytes
- Big-endian numbers
- Floats
- Non-UTF8 constant byte seeds
- Non-fixed arrays

[ci-status-image]: https://github.com/pina-rs/pina/workflows/ci/badge.svg
[ci-status-link]: https://github.com/pina-rs/pina/actions?query=workflow:ci
[license-image]: https://img.shields.io/badge/license-Apache--2.0-blue.svg?style=flat-square
[license-link]: https://www.apache.org/licenses/LICENSE-2.0
