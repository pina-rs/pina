# `pina_sdk_ids`

<p align="center">
	<img
		src="https://raw.githubusercontent.com/pina-rs/pina/main/.github/assets/logo.png"
		alt="Pina: a pineapple made from interlocking ribbons"
		width="140"
	>
</p>

<br>

Typed constants for well-known Solana program IDs and sysvar IDs.

Each module exposes an `ID` constant declared via `solana_address::declare_id!`.

<!-- {=crateReadmeBadgeRow:"pina_sdk_ids"} -->

<p align="center">
	<a href="https://crates.io/crates/pina_sdk_ids"><img alt="Crates.io" src="https://img.shields.io/badge/crates.io-pina__sdk__ids-orange?logo=rust&style=flat-square"></a>
	<a href="https://docs.rs/pina_sdk_ids/"><img alt="Docs.rs" src="https://img.shields.io/badge/docs.rs-pina__sdk__ids-1f425f?logo=docs.rs&style=flat-square"></a>
	<a href="https://github.com/pina-rs/pina/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/pina-rs/pina/ci.yml?label=CI&logo=githubactions&style=flat-square"></a>
	<a href="https://codecov.io/gh/pina-rs/pina"><img alt="Coverage" src="https://img.shields.io/codecov/c/github/pina-rs/pina?label=coverage&logo=codecov&style=flat-square"></a>
	<a href="https://opensource.org/license/apache-2.0"><img alt="License" src="https://img.shields.io/badge/license-Apache--2.0-blue?logo=apache&style=flat-square"></a>
</p>

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
self.system_program.assert_program(&system_program::ID)?;
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
