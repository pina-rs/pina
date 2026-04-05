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

<!-- {@pinaCliCommands} -->

| Command                  | Description                                           |
| ------------------------ | ----------------------------------------------------- |
| `pina init <name>`       | Scaffold a new Pina program project                   |
| `pina idl --path <dir>`  | Generate a Codama IDL JSON from a Pina program        |
| `pina profile <path.so>` | Static CU profiler for compiled SBF binaries          |
| `pina codama generate`   | Generate Codama IDLs and Rust/JS clients for examples |

<!-- {/pinaCliCommands} -->

<!-- {@pinaIntrospectionDescription} -->

The `pina::introspection` module provides helpers for reading the Instructions sysvar at runtime. This enables:

- **Flash loan guards** — verify the current instruction is not being invoked via CPI (`assert_no_cpi`)
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
- **Use `close_with_recipient()` with `zeroed()`** to safely close accounts and prevent revival attacks
- **Prefer `assert_seeds()` / `assert_canonical_bump()`** over `assert_seeds_with_bump()` to enforce canonical PDA bumps
- **Namespace PDA seeds** with type-specific prefixes to prevent PDA sharing across account types

<!-- {/pinaSecurityBestPractices} -->

<!-- {@pinaIdlCanonicalExamples} -->

### Multi-file layout

```rust
// src/lib.rs
use pina::*;

mod accounts;
mod instructions;
mod pda;
mod state;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");
```

### Canonical dispatch

```rust
#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use super::*;

	nostd_entrypoint!(process_instruction);

	pub fn process_instruction(
		program_id: &Address,
		accounts: &[AccountView],
		data: &[u8],
	) -> ProgramResult {
		let ix: MyInstruction = parse_instruction(program_id, &ID, data)?;

		// Add one arm per instruction variant.
		match ix {
			MyInstruction::Initialize => InitializeAccounts::try_from(accounts)?.process(data),
			MyInstruction::Update => UpdateAccounts::try_from(accounts)?.process(data),
		}
	}
}
```

### Validation chains

```rust
impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = InitializeInstruction::try_from_bytes(data)?;
		let seeds = my_seeds!(self.authority.address().as_ref(), args.bump);

		self.authority.assert_signer()?;
		self.system_program.assert_address(&system::ID)?;
		self.token_program.assert_address(&token::ID)?;
		self.ata_program
			.assert_address(&associated_token_account::ID)?;
		self.state
			.assert_empty()?
			.assert_writable()?
			.assert_seeds_with_bump(seeds, &ID)?;

		Ok(())
	}
}
```

### PDA seed helpers

```rust
const MY_SEED: &[u8] = b"my";

#[macro_export]
macro_rules! my_seeds {
	($authority:expr) => {
		&[MY_SEED, $authority]
	};
	($authority:expr, $bump:expr) => {
		&[MY_SEED, $authority, &[$bump]]
	};
}
```

### Discriminators and account layouts

```rust
#[discriminator]
pub enum MyInstruction {
	Initialize = 0,
	Update = 1,
}

#[discriminator]
pub enum MyAccountType {
	MyState = 1,
}

#[instruction(discriminator = MyInstruction, variant = Initialize)]
pub struct InitializeInstruction {
	pub bump: u8,
}

#[instruction(discriminator = MyInstruction, variant = Update)]
pub struct UpdateInstruction {
	pub value: PodU64,
}

#[account(discriminator = MyAccountType)]
pub struct MyState {
	pub bump: u8,
	pub value: PodU64,
}
```

<!-- {/pinaIdlCanonicalExamples} -->

<!-- {@pinaDiscriminatorLayoutDecisionMatrix} -->

## Discriminator layout decision matrix

The discriminator strategy determines byte layout, parser guarantees, and cross-protocol compatibility.

| Goal                                                                                 | Recommended layout                                                                                                                     |
| ------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------- |
| Keep layout **minimal and zero-copy** while staying explicit                         | **Current Pina model**: discriminator bytes are the first field inside `#[account]`, `#[instruction]`, and `#[event]` structs.         |
| Preserve compatibility with existing Anchor-account payloads (SHA-256 hash prefixes) | **Legacy adapter model**: custom raw wrapper types parse/write the existing 8-byte external prefix before converting to typed structs. |
| Minimize account size growth when you have many types                                | **Use `u8`** (default) discriminator width.                                                                                            |
| You need more than 256 route variants                                                | **Use `u16` / `u32` / `u64`** by setting `#[discriminator(primitive = ...)]`.                                                          |
| Avoid schema migrations across existing serialized data                              | Keep existing field order and discriminator values; only append fields.                                                                |

### Raw discriminator width by use-case

| Width | Max variants               | Storage cost (bytes) | Recommended when                                              |
| ----- | -------------------------- | -------------------- | ------------------------------------------------------------- |
| `u8`  | 256                        | 1                    | Most programs and instructions                                |
| `u16` | 65,536                     | 2                    | Medium-large routing tables and explicit version partitioning |
| `u32` | 4,294,967,296              | 4                    | Very large enums, rarely needed                               |
| `u64` | 18,446,744,073,709,551,616 | 8                    | Legacy interoperability shims or reserved growth              |

- Discriminator width only affects the first field bytes.
- Widths above 8 are rejected at macro expansion time.
- Wider discriminators improve variant space, but increase CPI payload and account rent by the exact number of bytes.

<!-- {/pinaDiscriminatorLayoutDecisionMatrix} -->

<!-- {@pinaDiscriminatorVersionCompatibility} -->

## Discriminator and payload versioning

| Change                                      | Compatibility impact                                               |
| ------------------------------------------- | ------------------------------------------------------------------ |
| Add a new enum variant                      | Usually backward-compatible if old clients ignore unknown variants |
| Change an existing variant value            | **Breaking** for every historical byte slice                       |
| Reorder or remove struct fields             | **Breaking** (offsets change)                                      |
| Append fields to a struct                   | Mostly non-breaking, but consumers must accept the larger size     |
| Switch primitive width (`u8` → `u16`, etc.) | **Breaking** for serialized payloads at that boundary              |

For on-chain accounts, treat layout as part of protocol ABI:

- Keep field order stable.
- Introduce optional `version` fields at the tail for in-place migration strategies.
- Never change existing discriminator values in place.
- When incompatible layout changes are required, perform explicit migration with a new account version and an operator upgrade flow.

For instruction payloads:

- Prefer additive migration: add a new variant and keep legacy handlers for a release cycle.
- Reject stale payload shapes with explicit errors rather than silently reinterpreting bytes.

<!-- {/pinaDiscriminatorVersionCompatibility} -->
