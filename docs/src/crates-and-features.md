# Crates and Features

## `crates/pina`

Core runtime crate for on-chain program logic.

Includes:

- `AccountView` and validation chain helpers.
- Typed account loaders and discriminator checks.
- CPI/system/token helper utilities.
- `nostd_entrypoint!` and instruction parsing helpers.

Feature flags:

<!-- {=pinaFeatureFlags} -->

| Feature  | Default | Description                                                |
| -------- | ------- | ---------------------------------------------------------- |
| `derive` | Yes     | Enables proc macros (`#[account]`, `#[instruction]`, etc.) |
| `logs`   | Yes     | Enables on-chain logging via `solana-program-log`          |
| `token`  | No      | Enables SPL token / token-2022 helpers and ATA utilities   |

<!-- {/pinaFeatureFlags} -->

## `crates/pina_macros`

Proc-macro crate used by `pina`.

Provides:

- `#[discriminator]`
- `#[account]`
- `#[instruction]`
- `#[event]`
- `#[error]`
- `#[derive(Accounts)]`

## `crates/pina_cli`

Developer CLI and library.

Commands:

- `pina init`: scaffold a new Pina program crate.
- `pina idl`: parse a Pina program and output Codama JSON.
- `pina codama generate`: generate Codama IDLs plus Rust/JS clients for examples.

Library surface:

- `pina_cli::generate_idl(program_path, name_override)`
- `pina_cli::init_project(path, package_name, force)`

## `crates/pina_sdk_ids`

`no_std` crate that exports well-known Solana program/sysvar IDs as typed constants.

Use this crate to avoid hardcoded base58 literals in validation logic.

## `crates/pina_pod_primitives`

`no_std` crate containing alignment-safe POD primitive wrappers (`PodBool`, `PodU*`, `PodI*`) and conversion macro helpers shared by `pina` and generated clients.

## `crates/pina_codama_renderer`

Repository-local renderer binary that generates Pina-style Rust client code from Codama JSON.

Use this when you want generated Rust models to match Pina's fixed-size discriminator-first/bytemuck conventions.
