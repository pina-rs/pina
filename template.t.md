<!-- {@devEnvironmentSetupCommands} -->

```bash
devenv shell
install:all
```

<!-- {/devEnvironmentSetupCommands} -->

<!-- {@buildAndTestCommands} -->

```bash
cargo build --all-features
cargo test
```

<!-- {/buildAndTestCommands} -->

<!-- {@commonQualityChecksCommands} -->

```bash
lint:clippy
lint:format
verify:docs
```

<!-- {/commonQualityChecksCommands} -->

<!-- {@docsBuildCommand} -->

```bash
docs:build
```

<!-- {/docsBuildCommand} -->

<!-- {@dailyDevelopmentLoop} -->

```bash
devenv shell
cargo build --all-features
cargo test
lint:all
verify:docs
verify:security
test:idl
```

<!-- {/dailyDevelopmentLoop} -->

<!-- {@codamaWorkflowCommands} -->

```bash
# Generate Codama IDLs for all examples.
codama:idl:all

# Generate Rust + JS clients.
codama:clients:generate

# Generate IDLs + Rust/JS clients in one command.
pina codama generate

# Run the complete Codama pipeline.
codama:test

# Run IDL fixture drift + validation checks used by CI.
test:idl
```

<!-- {/codamaWorkflowCommands} -->

<!-- {@releaseWorkflowCommands} -->

```bash
knope document-change
knope release
knope publish
```

<!-- {/releaseWorkflowCommands} -->

<!-- {@pinaFeatureFlags} -->

| Feature  | Default | Description                                                |
| -------- | ------- | ---------------------------------------------------------- |
| `derive` | Yes     | Enables proc macros (`#[account]`, `#[instruction]`, etc.) |
| `logs`   | Yes     | Enables on-chain logging via `solana-program-log`          |
| `token`  | No      | Enables SPL token / token-2022 helpers and ATA utilities   |

<!-- {/pinaFeatureFlags} -->

<!-- {@pinaProjectDescription} -->

A performant Solana smart contract framework built on top of [pinocchio](https://github.com/anza-xyz/pinocchio) — a zero-dependency alternative to `solana-program` that massively reduces compute units and dependency bloat.

<!-- {/pinaProjectDescription} -->

<!-- {@pinaInstallation} -->

```sh
cargo add pina
```

To enable SPL token support:

```sh
cargo add pina --features token
```

<!-- {/pinaInstallation} -->

<!-- {@podTypesTable} -->

| Type      | Wraps  | Size     |
| --------- | ------ | -------- |
| `PodBool` | `bool` | 1 byte   |
| `PodU16`  | `u16`  | 2 bytes  |
| `PodI16`  | `i16`  | 2 bytes  |
| `PodU32`  | `u32`  | 4 bytes  |
| `PodI32`  | `i32`  | 4 bytes  |
| `PodU64`  | `u64`  | 8 bytes  |
| `PodI64`  | `i64`  | 8 bytes  |
| `PodU128` | `u128` | 16 bytes |
| `PodI128` | `i128` | 16 bytes |

All types are `#[repr(transparent)]` over byte arrays (or `u8` for `PodBool`) and implement `bytemuck::Pod` + `bytemuck::Zeroable`.

<!-- {/podTypesTable} -->

<!-- {@podArithmeticDescription} -->

Arithmetic operators (`+`, `-`, `*`) use **wrapping** semantics in release builds for CU efficiency and **panic on overflow** in debug builds. Use `checked_add`, `checked_sub`, `checked_mul`, `checked_div` where overflow must be detected in all build profiles.

Each Pod integer type provides `ZERO`, `MIN`, and `MAX` constants.

<!-- {/podArithmeticDescription} -->

<!-- {@pinaWorkspacePackages} -->

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

<!-- {@pinaFeatureHighlights} -->

- **Zero-copy deserialization** — account data is reinterpreted in place via `bytemuck`, with no heap allocation.
- **`no_std` compatible** — all crates compile to the `bpfel-unknown-none` SBF target for on-chain deployment.
- **Low compute units** — built on `pinocchio` instead of `solana-program`, saving thousands of CU per instruction.
- **Discriminator system** — every account, instruction, and event type carries a typed discriminator as its first field.
- **Validation chaining** — chain assertions on `AccountView` references.
- **Proc-macro sugar** — `#[account]`, `#[instruction]`, `#[event]`, `#[error]`, `#[discriminator]`, and `#[derive(Accounts)]` eliminate boilerplate.
- **CPI helpers** — PDA account creation, lamport transfers, and token operations.

<!-- {/pinaFeatureHighlights} -->

<!-- {@sbfBuildInstructions} -->

Programs are compiled to the `bpfel-unknown-none` target using `sbpf-linker`:

```sh
cargo +nightly build --release --target bpfel-unknown-none -p my_program -Z build-std=core,alloc -F bpf-entrypoint
```

The `bpf-entrypoint` feature gate separates the on-chain entrypoint from the library code used in tests.

<!-- {/sbfBuildInstructions} -->

<!-- {@pinaTestingInstructions} -->

Programs are tested as regular Rust libraries (without the `bpf-entrypoint` feature) using [mollusk-svm](https://docs.rs/mollusk-svm) for Solana VM simulation:

```sh
cargo test
cargo nextest run  # Faster parallel test execution
```

<!-- {/pinaTestingInstructions} -->

<!-- {@pinaBadgeLinks} -->

[crate-image]: https://img.shields.io/crates/v/pina.svg?style=flat-square
[crate-link]: https://crates.io/crates/pina
[docs-image]: https://docs.rs/pina/badge.svg
[docs-link]: https://docs.rs/pina/
[ci-status-image]: https://github.com/pina-rs/pina/workflows/ci/badge.svg
[ci-status-link]: https://github.com/pina-rs/pina/actions?query=workflow:ci
[unlicense-image]: https://img.shields.io/badge/license-Unlicense-blue.svg?style=flat-square
[unlicense-link]: https://opensource.org/license/unlicense
[codecov-image]: https://codecov.io/github/pina-rs/pina/graph/badge.svg?token=87K799Q78I
[codecov-link]: https://codecov.io/github/pina-rs/pina

<!-- {/pinaBadgeLinks} -->

<!-- {@pinaSecurityBestPractices} -->

- **Always call `assert_signer()`** before trusting authority accounts
- **Always call `assert_owner()` / `assert_owners()`** before `as_token_*()` methods
- **Always call `assert_empty()`** before account initialization to prevent reinitialization attacks
- **Always verify program accounts** with `assert_address()` / `assert_program()` before CPI invocations
- **Use `assert_type::<T>()`** to prevent type cosplay — it checks discriminator, owner, and data size
- **Use `close_with_recipient()` with `zeroed()`** to safely close accounts and prevent revival attacks
- **Prefer `assert_seeds()` / `assert_canonical_bump()`** over `assert_seeds_with_bump()` to enforce canonical PDA bumps
- **Namespace PDA seeds** with type-specific prefixes to prevent PDA sharing across account types

<!-- {/pinaSecurityBestPractices} -->
