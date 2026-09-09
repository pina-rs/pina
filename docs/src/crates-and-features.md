# Crates and Features

<!-- {=pinaWorkspacePackages} -->

| Package                 | Path                          | Description                                                                   |
| ----------------------- | ----------------------------- | ----------------------------------------------------------------------------- |
| `pina`                  | `crates/pina`                 | Core framework: traits, account loaders, CPI helpers, and Pod types.          |
| `pina_macros`           | `crates/pina_macros`          | Proc macros: `#[account]`, `#[instruction]`, `#[event]`, and others.          |
| `pina_cli`              | `crates/pina_cli`             | CLI for building, testing, inspecting, and generating Pina program artifacts. |
| `pina_codama_renderer`  | `crates/pina_codama_renderer` | Repository-local Codama Rust renderer for Pina-style clients.                 |
| `pina_cpi_renderer`     | `crates/pina_cpi_renderer`    | Standalone Codama renderer generating Pina CPI client crates.                 |
| `pina_lints`            | `crates/pina_lints`           | Pina security lints and the driver behind `pina lint`.                        |
| `pina_test`             | `crates/pina_test`            | Surfpool-backed program test harness.                                         |
| `pina_profile`          | `crates/pina_profile`         | Static CU profiler for compiled SBF programs.                                 |
| `pina_sdk_ids`          | `crates/pina_sdk_ids`         | Typed constants for well-known Solana program/sysvar IDs.                     |
| `@pina-rs/codama-nodes` | `packages/nodes-from-pina`    | Pina IDL conversion and normalization for Codama root nodes.                  |
| `@pina-rs/cli`          | `packages/pina__cli`          | npm launcher for the prebuilt platform-specific CLI packages.                 |
| `@pina-rs/skill`        | `packages/pina__skill`        | Agent guidance and a non-destructive local skill installer.                   |

<!-- {/pinaWorkspacePackages} -->

## `crates/pina`

Core runtime crate for on-chain program logic.

Includes:

- `AccountView` and validation chain helpers.
- Typed account loaders and discriminator checks.
- CPI/system/token helper utilities.
- `nostd_entrypoint!` and instruction parsing helpers.
- Instruction introspection (program-ID checks, sandwich detection).
- Pod types with full arithmetic operator support.

Feature flags:

<!-- {=pinaFeatureFlags} -->

| Feature          | Default | Description                                                  |
| ---------------- | ------- | ------------------------------------------------------------ |
| `derive`         | Yes     | Enables proc macros (`#[account]`, `#[instruction]`, etc.)   |
| `logs`           | Yes     | Enables on-chain logging via `solana-program-log`            |
| `compact`        | No      | Enables compact schemas, checked loaders, and typed APIs     |
| `validation`     | No      | Enables declarative, allocation-free application validation  |
| `token`          | No      | Enables SPL token / token-2022 helpers and ATA utilities     |
| `memo`           | No      | Enables memo program helpers via `pina::memo`                |
| `account-resize` | No      | Enables raw account reallocation and safe Pinocchio resizing |

<!-- {/pinaFeatureFlags} -->

## Feature selection tips

<!-- {=pinaFeatureSelectionTips} -->

- `derive` is the normal choice for program crates; disable it only when you want the low-level runtime traits without the proc macros.
- `compact` enables `#[account(compact)]`, `PinaCompactAccount`, generated patch types, checked compact loaders, and `pina::String` and `pina::Vec`. It also enables `derive`.
- `validation` enables `PinaValidate` and `#[pina(validate(...))]` rules on accounts, instructions, events, and derived account lists. It also enables `derive`.
- `logs` is useful during **initial development and debugging**, testing, and audits. Disable it when you want the smallest possible binary or completely silent runtime failures.
- `token` enables `pina::token`, `pina::token_2022`, `pina::associated_token_account`, and the `TokenAccount` compatibility aliases over the upstream renamed account types.
- `memo` is separate from `token`, so memo CPI support can be enabled without pulling in the token helper surface.
- `account-resize` enables `ReallocAccount` and `ReallocAccountZeroed`. Enable it together with `compact` for `UpdateResizableAccount`, `ReallocCompactAccount`, and the compact creation builders. Close helpers still do not implicitly resize or zero account data.

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

- `pina init <name>`: scaffold a project-aware Pina program
- `pina build`: build SBF and publish the program IDL
- `pina generate`: generate configured CPI, Rust, TypeScript, or Dart clients
- `pina test [--unit]`: run native/Mollusk or SBF/Surfpool tests
- `pina dev [--yes]`: run Surfpool's persistent watch/redeploy loop
- `pina verify`: compare deployments and record verified source
- `pina idl --path <dir>`: generate a Codama IDL JSON from a Pina program
- `pina docs [topic]`: list or render bundled terminal documentation
- `pina keys [show|sync|new]`: inspect or explicitly update program identity
- `pina doctor [--json]`: diagnose project and toolchain readiness
- `pina completions <shell>`: generate a shell completion script
- `pina profile [path.so]`: profile a compiled or discovered SBF binary statically
- `pina deploy`: plan and execute an explicit cluster deployment
- `pina codama generate`: run the legacy repository-wide client workflow

<!-- {/pinaCliCommands} -->

The IDL parser supports multi-file programs — it follows `mod` declarations from `src/lib.rs` to discover accounts, instructions, and discriminators across all source files.

Library surface:

- `pina_cli::generate_idl(program_path, name_override)`
- `pina_cli::init_project(path, package_name, force)`

Generated program feature flags:

| Feature          | Default | Description                                                 |
| ---------------- | ------- | ----------------------------------------------------------- |
| `bpf-entrypoint` | No      | Compiles the on-chain entrypoint for SBF deployment builds. |

CPI clients are standalone generated crates rather than a feature of the deployed program crate. Add `cpi` to `clients.languages` in `pina.toml`, then run `pina generate`:

```toml
[clients]
output = "clients"
languages = ["cpi", "rust", "typescript"]
mode = "auto"
scaffold = true
```

The consuming program depends on the generated crate directly. This avoids coupling a program's deployable feature graph to downstream CPI consumers. Auto updates preserve customized manifests and entrypoints; set `scaffold = false` for generated sources only or `mode = "overwrite"` for an explicit clean sweep.

## Pod types

The `pina::pod` module re-exports PinaPod's alignment-safe POD primitive wrappers (`PodBool`, `PodU*`, `PodI*`) and fixed-capacity collection types (`PodOption`, `PodString`, `PodVec`), shared by `pina` and generated clients.

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

The full generic forms are `PodOption<T: ZcElem, PFX = 1>`, `PodString<N, PFX = 1>`, and `PodVec<T, N, PFX = 2>`. `PFX` is the prefix width in bytes and must be `1`, `2`, `4`, or `8`. Strings default to one byte and vectors default to two bytes. `ZcValidate` checks tags, prefixes, active elements, and UTF-8 before safe access.

<!-- {/podCollectionTypesTable} -->

<!-- {=podCollectionDescription} -->

Fixed account, instruction, and event schemas can use `String<N>`, `Vec<T, N>`, and `Option<T>` when every nested `T` has a fixed PinaPod representation. These values occupy their full capacity in the wire layout. PinaPod initializes inactive capacity, clears removed values, and validates active nested values before safe access.

Use `PodString<N, PFX>` and `PodVec<T, N, PFX>` when the default prefix width does not fit the declared capacity or the wire protocol specifies another width. The const generic is explicit: write `PodVec<u64, 1024, 2>`, not a macro attribute that selects `u16`.

Compact accounts store supported top-level strings, vectors, and dynamic options in tails, so unused capacity does not consume rent. See the compact-account guide for the accepted nesting forms and atomic patch API.

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

Use this when you want generated Rust models to match Pina's discriminator-first, PinaPod-validated conventions for fixed and compact accounts.

## `crates/pina_sdk_ids`

`no_std` crate that exports well-known Solana program/sysvar IDs as typed constants.

Use this crate to avoid hardcoded base58 literals in validation logic.
