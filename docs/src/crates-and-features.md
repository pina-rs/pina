# Crates and Features

<!-- {=pinaWorkspacePackages} -->

| Crate                  | Path                          | Description                                                       |
| ---------------------- | ----------------------------- | ----------------------------------------------------------------- |
| `pina`                 | `crates/pina`                 | Core framework — traits, account loaders, CPI helpers, Pod types. |
| `pina_macros`          | `crates/pina_macros`          | Proc macros — `#[account]`, `#[instruction]`, `#[event]`, etc.    |
| `pina_cli`             | `crates/pina_cli`             | CLI/library for IDL generation, Codama integration, scaffolding.  |
| `pina_codama_renderer` | `crates/pina_codama_renderer` | Repository-local Codama Rust renderer for Pina-style clients.     |
| `pina_pod_primitives`  | `crates/pina_pod_primitives`  | Alignment-safe `no_std` POD primitive wrappers.                   |
| `pina_profile`         | `crates/pina_profile`         | Static CU profiler for compiled SBF programs.                     |
| `pina_sdk_ids`         | `crates/pina_sdk_ids`         | Typed constants for well-known Solana program/sysvar IDs.         |

<!-- {/pinaWorkspacePackages} -->

## `crates/pina`

Core runtime crate for on-chain program logic.

Includes:

- `AccountView` and validation chain helpers.
- Typed account loaders and discriminator checks.
- CPI/system/token helper utilities.
- `nostd_entrypoint!` and instruction parsing helpers.
- Instruction introspection (flash loan guards, sandwich detection).
- Pod types with full arithmetic operator support.

Feature flags:

<!-- {=pinaFeatureFlags} -->

| Feature  | Default | Description                                                |
| -------- | ------- | ---------------------------------------------------------- |
| `derive` | Yes     | Enables proc macros (`#[account]`, `#[instruction]`, etc.) |
| `logs`   | Yes     | Enables on-chain logging via `solana-program-log`          |
| `token`  | No      | Enables SPL token / token-2022 helpers and ATA utilities   |

<!-- {/pinaFeatureFlags} -->

See [ADR 0004](./adrs/0004-no-std-and-no-allocator-boundary.md) and [ADR 0005](./adrs/0005-token-feature-boundaries.md) for the architectural rationale behind these feature and runtime boundaries.

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

<!-- {=pinaCliCommands} -->

| Command                  | Description                                           |
| ------------------------ | ----------------------------------------------------- |
| `pina init <name>`       | Scaffold a new Pina program project                   |
| `pina idl --path <dir>`  | Generate a Codama IDL JSON from a Pina program        |
| `pina profile <path.so>` | Static CU profiler for compiled SBF binaries          |
| `pina codama generate`   | Generate Codama IDLs and Rust/JS clients for examples |

<!-- {/pinaCliCommands} -->

The IDL parser supports multi-file programs — it follows `mod` declarations from `src/lib.rs` to discover accounts, instructions, and discriminators across all source files.

Library surface:

- `pina_cli::generate_idl(program_path, name_override)`
- `pina_cli::init_project(path, package_name, force)`

## `crates/pina_pod_primitives`

`no_std` crate containing alignment-safe POD primitive wrappers (`PodBool`, `PodU*`, `PodI*`) and conversion macro helpers shared by `pina` and generated clients.

<!-- {=podArithmeticDescription} -->

Arithmetic operators (`+`, `-`, `*`) use **wrapping** semantics in release builds for CU efficiency and **panic on overflow** in debug builds. Use `checked_add`, `checked_sub`, `checked_mul`, `checked_div` where overflow must be detected in all build profiles.

Each Pod integer type provides `ZERO`, `MIN`, and `MAX` constants.

<!-- {/podArithmeticDescription} -->

## `crates/pina_profile`

<!-- {=pinaProfileDescription} -->

The `pina profile` command analyzes compiled SBF `.so` binaries to estimate per-function compute unit costs without requiring a running validator.

```sh
pina profile target/deploy/my_program.so          # text summary
pina profile target/deploy/my_program.so --json    # JSON for CI
pina profile target/deploy/my_program.so -o r.json # write to file
```

The profiler decodes each SBF instruction opcode and assigns costs: regular instructions cost 1 CU, syscalls cost 100 CU.

<!-- {/pinaProfileDescription} -->

## `crates/pina_codama_renderer`

Repository-local renderer that generates Pina-style Rust client code from Codama JSON IDLs. The renderer is organized into focused modules under `src/render/`:

- `accounts.rs` — account page and PDA helpers
- `instructions.rs` — instruction page, account metas
- `types.rs` — Pod type rendering, defined types
- `errors.rs` — error page rendering
- `discriminator.rs` — discriminator rendering
- `seeds.rs` — seed parameter/constant rendering

Use this when you want generated Rust models to match Pina's fixed-size discriminator-first/bytemuck conventions.

## `crates/pina_sdk_ids`

`no_std` crate that exports well-known Solana program/sysvar IDs as typed constants.

Use this crate to avoid hardcoded base58 literals in validation logic.
