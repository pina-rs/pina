# Crates and Features

<!-- {=pinaWorkspacePackages} -->

| Package                 | Path                          | Description                                                       |
| ----------------------- | ----------------------------- | ----------------------------------------------------------------- |
| `pina`                  | `crates/pina`                 | Core framework — traits, account loaders, CPI helpers, Pod types. |
| `pina_macros`           | `crates/pina_macros`          | Proc macros — `#[account]`, `#[instruction]`, `#[event]`, etc.    |
| `pina_cli`              | `crates/pina_cli`             | CLI/library for IDL generation, Codama integration, scaffolding.  |
| `pina_codama_renderer`  | `crates/pina_codama_renderer` | Repository-local Codama Rust renderer for Pina-style clients.     |
| `pina_profile`          | `crates/pina_profile`         | Static CU profiler for compiled SBF programs.                     |
| `pina_sdk_ids`          | `crates/pina_sdk_ids`         | Typed constants for well-known Solana program/sysvar IDs.         |
| `@pina-rs/codama-nodes` | `packages/nodes-from-pina`    | Pina IDL conversion and normalization for Codama root nodes.      |
| `@pina-rs/cli`          | `packages/pina__cli`          | npm launcher for the prebuilt platform-specific CLI packages.     |
| `@pina-rs/skill`        | `packages/pina__skill`        | Agent guidance and a non-destructive local skill installer.       |

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

| Feature          | Default | Description                                                     |
| ---------------- | ------- | --------------------------------------------------------------- |
| `derive`         | Yes     | Enables proc macros (`#[account]`, `#[instruction]`, etc.)      |
| `logs`           | Yes     | Enables on-chain logging via `solana-program-log`               |
| `token`          | No      | Enables SPL token / token-2022 helpers and ATA utilities        |
| `memo`           | No      | Enables memo program helpers via `pina::memo`                   |
| `account-resize` | No      | Enables account realloc helpers that call Pinocchio resize APIs |

<!-- {/pinaFeatureFlags} -->

## Feature selection tips

<!-- {=pinaFeatureSelectionTips} -->

- `derive` is the normal choice for program crates; disable it only when you want the low-level runtime traits without the proc macros.
- `logs` is useful during **initial development and debugging**, testing, and audits. Disable it when you want the smallest possible binary or completely silent runtime failures.
- `token` enables `pina::token`, `pina::token_2022`, `pina::associated_token_account`, and the `TokenAccount` compatibility aliases over the upstream renamed account types.
- `memo` is separate from `token`, so memo CPI support can be enabled without pulling in the token helper surface.
- `account-resize` only enables realloc helpers such as `realloc_account()` and `realloc_account_zero()`. Close helpers still do not implicitly resize or zero account data.

<!-- {/pinaFeatureSelectionTips} -->

See [ADR 0004](./adrs/0004-no-std-and-no-allocator-boundary.md) and [ADR 0005](./adrs/0005-token-feature-boundaries.md) for the architectural rationale behind these feature and runtime boundaries. For concrete token CPI patterns, see [Token CPI Recipes](./tutorials/token-cpi-recipes.md).

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

- `pina init <name>` — scaffold a project-aware Pina program
- `pina build` — build SBF and publish the program IDL
- `pina generate` — generate configured Rust, TypeScript, or Dart clients
- `pina idl --path <dir>` — generate a Codama IDL JSON from a Pina program
- `pina docs [topic]` — list or render bundled terminal documentation
- `pina profile <path.so>` — profile a compiled SBF binary statically
- `pina codama generate` — run the legacy repository-wide client workflow

<!-- {/pinaCliCommands} -->

The IDL parser supports multi-file programs — it follows `mod` declarations from `src/lib.rs` to discover accounts, instructions, and discriminators across all source files.

Library surface:

- `pina_cli::generate_idl(program_path, name_override)`
- `pina_cli::init_project(path, package_name, force)`

Generated program feature flags:

| Feature          | Default | Description                                                                 |
| ---------------- | ------- | --------------------------------------------------------------------------- |
| `bpf-entrypoint` | No      | Compiles the on-chain entrypoint for SBF deployment builds.                 |
| `cpi`            | No      | Exposes that program's typed on-chain CPI builders for downstream programs. |

Keep `cpi` disabled for deployed programs that do not need to export their CPI surface. Consumer programs can opt in explicitly:

```toml
other_program = { version = "...", default-features = false, features = ["cpi"] }
```

## Pod types

The `pina::pod` module re-exports zeropod's alignment-safe POD primitive wrappers (`PodBool`, `PodU*`, `PodI*`) and fixed-capacity collection types (`PodOption`, `PodString`, `PodVec`), shared by `pina` and generated clients.

<!-- {=podArithmeticDescription} -->

Arithmetic operators (`+`, `-`, `*`) on Pod **integer** types use **wrapping** semantics in release builds for CU efficiency and **panic on overflow** in debug builds. Use `checked_add`, `checked_sub`, `checked_mul`, `checked_div` where overflow must be detected in all build profiles.

Each Pod integer type provides `ZERO`, `MIN`, and `MAX` constants.

<!-- {/podArithmeticDescription} -->

<!-- {=podCollectionTypesTable} -->

| Type        | Purpose                | Layout                                    |
| ----------- | ---------------------- | ----------------------------------------- |
| `PodOption` | Fixed-size `Option<T>` | 1-byte discriminant + `T`                 |
| `PodString` | Fixed-capacity string  | `PFX`-byte length prefix + `N` data bytes |
| `PodVec`    | Fixed-capacity vec     | `PFX`-byte length prefix + `N` elements   |

The full generic forms are `PodOption<T: ZcElem>`, `PodString<N, PFX = 1>`, and `PodVec<T: ZcElem, N, PFX = 2>`. All collection layouts are alignment 1 and padding-free when `T: ZcElem`. `ZcValidate` checks tags, length prefixes, active elements, and UTF-8 before safe access. Length prefixes (`PFX`) default to 1 byte for strings (max 255) and 2 bytes for vectors (max 65 535 elements).

<!-- {/podCollectionTypesTable} -->

<!-- {=podCollectionDescription} -->

Collection types store data inline without allocation for advanced direct zeropod use. Pina's `#[account]`, `#[instruction]`, and `#[event]` macros reject `PodString`/`String` and `PodVec`/`Vec` fields because their inactive capacity is not guaranteed to be initialized after every upstream construction path. Use fully initialized fixed byte arrays plus checked semantic helpers in macro-generated schemas. Semantic `Option<scalar>` remains supported because Pina proves its exact `PodOption` mapping and scalar storage contract.

For direct zeropod integrations, zeropod boundary validation must establish the active `PodString` bytes are valid UTF-8 before callers use `as_str()`. `PodVec` offers slice-based access via `as_slice()` / `as_slice_mut()`, and `PodOption` mirrors the `Option<T>` API with `get()`, `set()`, and `clear()`. Those direct integrations are outside Pina's audited macro-generated contract and must uphold zeropod's complete safety invariants.

<!-- {/podCollectionDescription} -->

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

Use this when you want generated Rust models to match Pina's fixed-size, discriminator-first, zeropod-validated conventions.

## `crates/pina_sdk_ids`

`no_std` crate that exports well-known Solana program/sysvar IDs as typed constants.

Use this crate to avoid hardcoded base58 literals in validation logic.
