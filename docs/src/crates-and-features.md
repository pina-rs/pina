# Crates and Features

## `crates/pina`

Core runtime crate with:

- Validation traits and helper methods
- PDA and CPI helpers
- `nostd_entrypoint!` and dispatch helpers
- Optional token integrations

### Main features

- `derive` (default): enables proc-macro integration
- `logs` (default): on-chain log support
- `token`: SPL token/token-2022 helpers and typed conversions

## `crates/pina_macros`

Proc-macro crate that defines:

- `#[account]`
- `#[instruction]`
- `#[event]`
- `#[error]`
- `#[discriminator]`
- `#[derive(Accounts)]`

## `crates/pina_sdk_ids`

Shared Solana IDs for known programs/sysvars to keep address usage centralized and typed.
