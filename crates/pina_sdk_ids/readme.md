# `pina_sdk_ids`

<p align="center">
	<img src="https://raw.githubusercontent.com/pina-rs/pina/main/.github/assets/logo.png" alt="The Pina logo: a low-poly origami pineapple" width="140">
</p>

<br>

Typed constants for well-known Solana program IDs and sysvar IDs.

Each module exposes an `ID` constant declared via `solana_address::declare_id!`.

<!-- {=crateReadmeBadgeRow:"pina_sdk_ids"} -->

[![Crates.io](https://img.shields.io/badge/crates.io-pina**sdk**ids-orange?logo=rust)](https://crates.io/crates/pina_sdk_ids) [![Docs.rs](https://img.shields.io/badge/docs.rs-pina**sdk**ids-1f425f?logo=docs.rs)](https://docs.rs/pina_sdk_ids/) [![CI](https://github.com/pina-rs/pina/actions/workflows/ci.yml/badge.svg)](https://github.com/pina-rs/pina/actions/workflows/ci.yml) [![Coverage](https://codecov.io/gh/pina-rs/pina/branch/main/graph/badge.svg)](https://codecov.io/gh/pina-rs/pina) [![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://opensource.org/license/apache-2.0)

<!-- {/crateReadmeBadgeRow} -->

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

// Validation chains read best with named IDs:
self.system_program.assert_address(&system_program::ID)?;
self.clock.assert_sysvar(&sysvar::clock::ID)?;
// Signature-verification programs stay typed during CPI checks:
self.ed25519_program.assert_address(&pina_sdk_ids::ed25519_program::ID)?;
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
