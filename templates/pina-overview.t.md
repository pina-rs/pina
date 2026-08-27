<!-- {@pinaFeatureFlags} -->

| Feature          | Default | Description                                                     |
| ---------------- | ------- | --------------------------------------------------------------- |
| `derive`         | Yes     | Enables proc macros (`#[account]`, `#[instruction]`, etc.)      |
| `logs`           | Yes     | Enables on-chain logging via `solana-program-log`               |
| `token`          | No      | Enables SPL token / token-2022 helpers and ATA utilities        |
| `memo`           | No      | Enables memo program helpers via `pina::memo`                   |
| `account-resize` | No      | Enables account realloc helpers that call Pinocchio resize APIs |

<!-- {/pinaFeatureFlags} -->

<!-- {@pinaFeatureSelectionTips} -->

- `derive` is the normal choice for program crates; disable it only when you want the low-level runtime traits without the proc macros.
- `logs` is useful during **initial development and debugging**, testing, and audits. Disable it when you want the smallest possible binary or completely silent runtime failures.
- `token` enables `pina::token`, `pina::token_2022`, `pina::associated_token_account`, and the `TokenAccount` compatibility aliases over the upstream renamed account types.
- `memo` is separate from `token`, so memo CPI support can be enabled without pulling in the token helper surface.
- `account-resize` only enables realloc helpers such as `realloc_account()` and `realloc_account_zero()`. Close helpers still do not implicitly resize or zero account data.

<!-- {/pinaFeatureSelectionTips} -->

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

All types are alignment-1 byte-backed values that implement zeropod's `ZcElem` and `ZcValidate` contracts.

<!-- {/podTypesTable} -->

<!-- {@podCollectionTypesTable} -->

| Type        | Purpose                | Layout                                    |
| ----------- | ---------------------- | ----------------------------------------- |
| `PodOption` | Fixed-size `Option<T>` | 1-byte discriminant + `T`                 |
| `PodString` | Fixed-capacity string  | `PFX`-byte length prefix + `N` data bytes |
| `PodVec`    | Fixed-capacity vec     | `PFX`-byte length prefix + `N` elements   |

The full generic forms are `PodOption<T: ZcElem>`, `PodString<N, PFX = 1>`, and `PodVec<T: ZcElem, N, PFX = 2>`. All collection layouts are alignment 1 and padding-free when `T: ZcElem`. `ZcValidate` checks tags, length prefixes, active elements, and UTF-8 before safe access. Length prefixes (`PFX`) default to 1 byte for strings (max 255) and 2 bytes for vectors (max 65 535 elements).

<!-- {/podCollectionTypesTable} -->

<!-- {@podCollectionDescription} -->

Collection types store data inline without allocation for advanced direct zeropod use. Pina's `#[account]`, `#[instruction]`, and `#[event]` macros reject `PodString`/`String` and `PodVec`/`Vec` fields because their inactive capacity is not guaranteed to be initialized after every upstream construction path. Use fully initialized fixed byte arrays plus checked semantic helpers in macro-generated schemas. Semantic `Option<scalar>` remains supported because Pina proves its exact `PodOption` mapping and scalar storage contract.

For direct zeropod integrations, zeropod boundary validation must establish the active `PodString` bytes are valid UTF-8 before callers use `as_str()`. `PodVec` offers slice-based access via `as_slice()` / `as_slice_mut()`, and `PodOption` mirrors the `Option<T>` API with `get()`, `set()`, and `clear()`. Those direct integrations are outside Pina's audited macro-generated contract and must uphold zeropod's complete safety invariants.

<!-- {/podCollectionDescription} -->

<!-- {@podArithmeticDescription} -->

Arithmetic operators (`+`, `-`, `*`) on Pod **integer** types use **wrapping** semantics in release builds for CU efficiency and **panic on overflow** in debug builds. Use `checked_add`, `checked_sub`, `checked_mul`, `checked_div` where overflow must be detected in all build profiles.

Each Pod integer type provides `ZERO`, `MIN`, and `MAX` constants.

<!-- {/podArithmeticDescription} -->

<!-- {@pinaWorkspacePackages} -->

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

<!-- {@pinaFeatureHighlights} -->

- **Validated zero-copy deserialization** — zeropod validates fixed-layout account data before Pina reinterprets it in place, with no heap allocation.
- **`no_std` compatible** — all crates compile to the `bpfel-unknown-none` SBF target for on-chain deployment.
- **Low compute units** — built on `pinocchio` instead of `solana-program`, saving thousands of CU per instruction.
- **Discriminator system** — every account, instruction, and event type carries a typed discriminator as its first field.
- **Validation chaining** — chain assertions on `AccountView` references.
- **Proc-macro sugar** — `#[account]`, `#[instruction]`, `#[event]`, `#[error]`, `#[discriminator]`, and `#[derive(Accounts)]` eliminate boilerplate.
- **CPI helpers** — PDA account creation, lamport transfers, and token operations.

<!-- {/pinaFeatureHighlights} -->

<!-- {@pinaInstructionAuthoringTips} -->

- Entry points should accept `&mut [AccountView]` and dispatch with `Accounts::try_from((program_id, accounts))?.process(data)`.
- Use `&AccountView` for read-only accounts and `&mut AccountView` only when you need mutable loaders, direct lamport mutation, `close_*` helpers, or writable IDL inference.
- Keep `assert_writable()` explicit even on `&mut AccountView`. Type-level mutability enables mutable APIs, but the runtime still decides whether the account is writable for the current instruction.
- `as_account()` / `as_account_mut()` return `Ref<T>` / `RefMut<T>` borrow guards. Copy out the fields you need and `drop(...)` the guard before CPIs or later mutable borrows.
- Keep validation chains direct inside `process(self, ...)` when possible. That makes audits easier and gives `pina idl` the clearest signal for signer, writable, PDA, and default-account inference.

<!-- {/pinaInstructionAuthoringTips} -->

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
[license-image]: https://img.shields.io/badge/license-Apache--2.0-blue.svg?style=flat-square
[license-link]: https://www.apache.org/licenses/LICENSE-2.0
[codecov-image]: https://codecov.io/github/pina-rs/pina/graph/badge.svg?token=87K799Q78I
[codecov-link]: https://codecov.io/github/pina-rs/pina

<!-- {/pinaBadgeLinks} -->

<!-- {@pinaCliCommands} -->

| Command                  | Description                                    |
| ------------------------ | ---------------------------------------------- |
| `pina init <name>`       | Scaffold a project-aware Pina program          |
| `pina build`             | Build SBF and publish the program IDL           |
| `pina generate`          | Generate configured Rust/TypeScript/Dart clients |
| `pina idl --path <dir>`  | Generate a Codama IDL JSON from a Pina program |
| `pina docs [topic]`      | List or render bundled terminal documentation  |
| `pina profile <path.so>` | Static CU profiler for compiled SBF binaries   |
| `pina codama generate`   | Run the legacy repository-wide client workflow |

<!-- {/pinaCliCommands} -->

<!-- {@pinaIntrospectionDescription} -->

The `pina::introspection` module provides helpers for reading the Instructions sysvar at runtime. This enables:

- **Program checks** — verify that the transaction-level instruction at the current index targets the expected program (`assert_current_instruction_program_id`). The Instructions sysvar cannot distinguish self-CPI, so this is not a no-CPI or flash-loan guard.
- **Transaction inspection** — count instructions (`get_instruction_count`) or find the current index (`get_current_instruction_index`)
- **Sandwich detection** — check whether a specific program appears before or after the current instruction (`has_instruction_before`, `has_instruction_after`)

<!-- {/pinaIntrospectionDescription} -->

<!-- {@pinaProfileDescription} -->

The `pina profile` command analyzes compiled SBF `.so` binaries to estimate per-function compute unit costs without requiring a running validator.

```sh
pina profile target/deploy/my_program.so          # text summary
pina profile target/deploy/my_program.so --json    # JSON for CI
pina profile target/deploy/my_program.so -o r.json # write to file
```

The profiler decodes each SBF instruction opcode and assigns costs: regular instructions cost 1 CU, syscalls cost 100 CU.

<!-- {/pinaProfileDescription} -->

<!-- {@pinaSecurityBestPractices} -->

- **Always call `assert_signer()`** before trusting authority accounts
- **Always call `assert_owner()` / `assert_owners()`** before `as_token_*()` methods
- **Always call `assert_empty()`** before account initialization to prevent reinitialization attacks
- **Always verify program accounts** with `assert_address()` / `assert_program()` before CPI invocations
- **Use `assert_type::<T>()`** to prevent type cosplay — it checks discriminator, owner, and data size
- **Use `close_account_zeroed()` or `zeroed()` + `close_with_recipient()`** when stale account bytes must be invalidated before close
- **Prefer `assert_seeds()` / `assert_canonical_bump()`** over `assert_seeds_with_bump()` to enforce canonical PDA bumps
- **Namespace PDA seeds** with type-specific prefixes to prevent PDA sharing across account types

<!-- {/pinaSecurityBestPractices} -->

<!-- {@pinaCloseAccountGuidance} -->

Closing guidance under Pinocchio 0.11:

- `close_with_recipient()` transfers lamports and closes the account handle, but it does not zero or resize account data for you.
- When stale bytes must be invalidated, use `close_account_zeroed()` or manually call `zeroed()` before `close_with_recipient()`.
- The `account-resize` feature only affects realloc helpers; it does not change close semantics.

<!-- {/pinaCloseAccountGuidance} -->
