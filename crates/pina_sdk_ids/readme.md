# `pina_sdk_ids`

<br>

Typed constants for well-known Solana program IDs and sysvar IDs.

Each module exposes an `ID` constant declared via `solana_address::declare_id!`.

[![Crates.io][crate-image]][crate-link] [![Docs.rs][docs-image]][docs-link] [![CI][ci-status-image]][ci-status-link] [![License][unlicense-image]][unlicense-link] [![codecov][codecov-image]][codecov-link]

## Installation

<br>

```bash
cargo add pina_sdk_ids
```

## Usage

<br>

```rust
use pina_sdk_ids::system_program;
use pina_sdk_ids::sysvar;

let system_program_id = system_program::ID;
let clock_sysvar_id = sysvar::clock::ID;
```

## Included IDs

<br>

- Core programs: `system_program`, `stake`, `vote`, `config`, `feature`, loaders.
- Signature verification programs: `ed25519_program`, `secp256k1_program`, `secp256r1_program`.
- Sysvars: `sysvar::clock`, `sysvar::rent`, `sysvar::stake_history`, and more.
- Utility addresses: `incinerator`, compute budget, lookup table, and zk proof programs.

## Why Use This Crate

<br>

- Avoid hard-coded base58 strings across codebases.
- Keep ID imports centralized and typed.
- Make account/program validation checks more readable.

## `no_std`

<br>

`pina_sdk_ids` is `#![no_std]` and safe for on-chain program crates.

[crate-image]: https://img.shields.io/crates/v/pina_sdk_ids.svg?style=flat-square
[crate-link]: https://crates.io/crates/pina_sdk_ids
[docs-image]: https://docs.rs/pina_sdk_ids/badge.svg
[docs-link]: https://docs.rs/pina_sdk_ids/
[ci-status-image]: https://github.com/pina-rs/pina/workflows/ci/badge.svg
[ci-status-link]: https://github.com/pina-rs/pina/actions?query=workflow:ci
[unlicense-image]: https://img.shields.io/badge/license-Unlicense-blue.svg?style=flat-square
[unlicense-link]: https://opensource.org/license/unlicense
[codecov-image]: https://codecov.io/github/pina-rs/pina/graph/badge.svg?token=87K799Q78I
[codecov-link]: https://codecov.io/github/pina-rs/pina
