<!-- {@pinaFeatureFlags} -->

| Feature          | Default | Description                                                  |
| ---------------- | ------- | ------------------------------------------------------------ |
| `derive`         | Yes     | Enables proc macros (`#[account]`, `#[instruction]`, etc.)   |
| `logs`           | Yes     | Enables on-chain logging via `solana-program-log`            |
| `compact`        | No      | Enables compact schemas, checked loaders, and typed APIs     |
| `token`          | No      | Enables SPL token / token-2022 helpers and ATA utilities     |
| `memo`           | No      | Enables memo program helpers via `pina::memo`                |
| `account-resize` | No      | Enables raw account reallocation and safe Pinocchio resizing |

<!-- {/pinaFeatureFlags} -->

<!-- {@pinaFeatureSelectionTips} -->

- `derive` is the normal choice for program crates; disable it only when you want the low-level runtime traits without the proc macros.
- `compact` enables `#[account(compact)]`, `PinaCompactAccount`, generated patch types, checked compact loaders, and `pina::String` and `pina::Vec`. It also enables `derive`.
- `logs` is useful during **initial development and debugging**, testing, and audits. Disable it when you want the smallest possible binary or completely silent runtime failures.
- `token` enables `pina::token`, `pina::token_2022`, `pina::associated_token_account`, and the `TokenAccount` compatibility aliases over the upstream renamed account types.
- `memo` is separate from `token`, so memo CPI support can be enabled without pulling in the token helper surface.
- `account-resize` enables `ReallocAccount` and `ReallocAccountZeroed`. Enable it together with `compact` for `UpdateResizableAccount`, `ReallocCompactAccount`, and the compact creation builders. Close helpers still do not implicitly resize or zero account data.

<!-- {/pinaFeatureSelectionTips} -->

<!-- {@pinaProjectDescription} -->

A Solana smart contract framework built on [pinocchio](https://github.com/anza-xyz/pinocchio), a zero-dependency alternative to `solana-program` that reduces compute usage and dependency size.

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

All types are alignment-one byte-backed values that implement PinaPod's `ZcElem` and `ZcValidate` contracts.

<!-- {/podTypesTable} -->

<!-- {@podCollectionTypesTable} -->

| Type        | Purpose                | Layout                                    |
| ----------- | ---------------------- | ----------------------------------------- |
| `PodOption` | Fixed-size `Option<T>` | 1-byte discriminant + `T`                 |
| `PodString` | Fixed-capacity string  | `PFX`-byte length prefix + `N` data bytes |
| `PodVec`    | Fixed-capacity vec     | `PFX`-byte length prefix + `N` elements   |

The full generic forms are `PodOption<T: ZcElem, PFX = 1>`, `PodString<N, PFX = 1>`, and `PodVec<T, N, PFX = 2>`. `PFX` is the prefix width in bytes and must be `1`, `2`, `4`, or `8`. Strings default to one byte and vectors default to two bytes. `ZcValidate` checks tags, prefixes, active elements, and UTF-8 before safe access.

<!-- {/podCollectionTypesTable} -->

<!-- {@podCollectionDescription} -->

Fixed account, instruction, and event schemas can use `String<N>`, `Vec<T, N>`, and `Option<T>` when every nested `T` has a fixed PinaPod representation. These values occupy their full capacity in the wire layout. PinaPod initializes inactive capacity, clears removed values, and validates active nested values before safe access.

Use `PodString<N, PFX>` and `PodVec<T, N, PFX>` when the default prefix width does not fit the declared capacity or the wire protocol specifies another width. The const generic is explicit: write `PodVec<u64, 1024, 2>`, not a macro attribute that selects `u16`.

Compact accounts store supported top-level strings, vectors, and dynamic options in tails, so unused capacity does not consume rent. See the compact-account guide for the accepted nesting forms and atomic patch API.

<!-- {/podCollectionDescription} -->

<!-- {@podArithmeticDescription} -->

Arithmetic operators (`+`, `-`, `*`) on Pod **integer** types use **wrapping** semantics in release builds for CU efficiency and **panic on overflow** in debug builds. Use `checked_add`, `checked_sub`, `checked_mul`, `checked_div` where overflow must be detected in all build profiles.

Each Pod integer type provides `ZERO`, `MIN`, and `MAX` constants.

<!-- {/podArithmeticDescription} -->

<!-- {@pinaWorkspacePackages} -->

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

<!-- {@pinaFeatureHighlights} -->

- **Validated zero-copy deserialization**: PinaPod validates account data before Pina returns an in-place view, with no heap allocation.
- **`no_std` compatible**: all crates compile to the `bpfel-unknown-none` SBF target for on-chain deployment.
- **Low compute units**: built on `pinocchio` instead of `solana-program`, saving thousands of CU per instruction.
- **Discriminator system**: every account, instruction, and event type carries a typed discriminator as its first field.
- **Validation chaining**: chain assertions on `AccountView` references.
- **Proc-macro sugar**: `#[account]`, `#[instruction]`, `#[event]`, `#[error]`, `#[discriminator]`, and `#[derive(Accounts)]` eliminate boilerplate.
- **CPI helpers**: PDA account creation, lamport transfers, and token operations.

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
cargo build --release --target bpfel-unknown-none -p my_program -Z build-std=core,alloc -F bpf-entrypoint
```

The pinned nightly toolchain from `rust-toolchain.toml` runs the build; the `bpf-entrypoint` feature gate separates the on-chain entrypoint from the library code used in tests.

<!-- {/sbfBuildInstructions} -->

<!-- {@pinaTestingInstructions} -->

Programs are tested as regular Rust libraries (without the `bpf-entrypoint` feature) using [mollusk-svm](https://docs.rs/mollusk-svm) for Solana VM simulation:

```sh
cargo test
cargo nextest run  # Faster parallel test execution
```

<!-- {/pinaTestingInstructions} -->

<!-- {@pinaCliCommands} -->

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

<!-- {@pinaIntrospectionDescription} -->

The `pina::introspection` module provides helpers for reading the Instructions sysvar at runtime. This enables:

- **Program checks**: verify that the transaction-level instruction at the current index targets the expected program (`assert_current_instruction_program_id`). The Instructions sysvar cannot distinguish self-CPI, so this is not a no-CPI or flash-loan guard.
- **Transaction inspection**: count instructions (`get_instruction_count`) or find the current index (`get_current_instruction_index`)
- **Sandwich detection**: check whether a specific program appears before or after the current instruction (`has_instruction_before`, `has_instruction_after`)

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
- **Use `invoke_with` or `invoke_signed_with`** when fixed-account creation must establish nonzero values before final PinaPod validation
- **Use generated `load_pda` or `load_pda_mut`** when a fixed stored-bump PDA handler needs a typed guard, so recursive content and the PDA address are validated once
- **Always verify program accounts** with `assert_address()` / `assert_program()` before CPI invocations
- **Use `assert_type::<T>()`** to prevent type cosplay: it checks discriminator, owner, and data size
- **Use `CloseAccountZeroed { account, recipient }.invoke()` or `zeroed()` + `close_with_recipient()`** when stale account bytes must be invalidated before close
- **Prefer `assert_seeds()` / `assert_canonical_bump()`** over `assert_seeds_with_bump()` to enforce canonical PDA bumps
- **Give each account type its own seed namespace** so PDAs cannot collide across account types

<!-- {/pinaSecurityBestPractices} -->

<!-- {@pinaCloseAccountGuidance} -->

Closing guidance under Pinocchio 0.11:

- `close_with_recipient()` transfers lamports and closes the account handle, but it does not zero or resize account data for you.
- When stale bytes must be invalidated, use `CloseAccountZeroed { account, recipient }.invoke()` or manually call `zeroed()` before `close_with_recipient()`.
- The `account-resize` feature only affects realloc helpers; it does not change close semantics.

<!-- {/pinaCloseAccountGuidance} -->
