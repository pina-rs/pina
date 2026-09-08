# pina

<p align="center">
	<img src="./.github/assets/logo.png" alt="The Pina logo: a low-poly origami pineapple" width="180">
</p>

<br>

<!-- {=pinaProjectDescription} -->

A Solana smart contract framework built on [pinocchio](https://github.com/anza-xyz/pinocchio), a zero-dependency alternative to `solana-program` that reduces compute usage and dependency size.

<!-- {/pinaProjectDescription} -->

<!-- {=crateReadmeBadgeRow:"pina"} -->

[![Crates.io](https://img.shields.io/badge/crates.io-pina-orange?logo=rust)](https://crates.io/crates/pina) [![Docs.rs](https://img.shields.io/badge/docs.rs-pina-1f425f?logo=docs.rs)](https://docs.rs/pina/) [![CI](https://github.com/pina-rs/pina/actions/workflows/ci.yml/badge.svg)](https://github.com/pina-rs/pina/actions/workflows/ci.yml) [![Coverage](https://codecov.io/gh/pina-rs/pina/branch/main/graph/badge.svg)](https://codecov.io/gh/pina-rs/pina) [![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://opensource.org/license/apache-2.0)

<!-- {/crateReadmeBadgeRow} -->

> Pina is currently unaudited and still hardening. See [SECURITY.md](./SECURITY.md) for the current readiness statement, supported versions, and private vulnerability reporting instructions.

## Features

<br>

<!-- {=pinaFeatureHighlights} -->

- **Validated zero-copy deserialization**: PinaPod validates account data before Pina returns an in-place view, with no heap allocation.
- **`no_std` compatible**: all crates compile to the `bpfel-unknown-none` SBF target for on-chain deployment.
- **Low compute units**: built on `pinocchio` instead of `solana-program`, saving thousands of CU per instruction.
- **Discriminator system**: every account, instruction, and event type carries a typed discriminator as its first field.
- **Validation chaining**: chain assertions on `AccountView` references.
- **Proc-macro sugar**: `#[account]`, `#[instruction]`, `#[event]`, `#[error]`, `#[discriminator]`, and `#[derive(Accounts)]` eliminate boilerplate.
- **CPI helpers**: PDA account creation, lamport transfers, and token operations.

<!-- {/pinaFeatureHighlights} -->

## Workspace packages

<br>

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

## Installation

<br>

<!-- {=pinaInstallation} -->

```sh
cargo add pina
```

To enable SPL token support:

```sh
cargo add pina --features token
```

<!-- {/pinaInstallation} -->

Install the prebuilt CLI from npm on macOS, Linux, Windows, or FreeBSD:

```sh
npm install --global @pina-rs/cli
pina --help
```

The Rust-native installation remains available with `cargo install pina_cli`. Agent tooling can install the bundled project skill separately:

```sh
npm install --global @pina-rs/skill
pina-skill --install
```

## Codama IDL Support

<br>

Pina ships with first-class Codama integration through the `pina` CLI and the `codama/` test harness in this repository.

The CLI command below generates a Codama-compatible IDL from a Pina program:

```sh
pina idl --path examples/counter_program --output codama/idls/counter_program.json
```

With `devenv`, the full workflow is available via built-in scripts:

<!-- {=codamaWorkflowCommands} -->

```bash
# Generate Codama IDLs for all examples.
codama:idl:all

# Generate Rust + CPI + JS + Dart clients.
codama:clients:generate

# Generate IDLs + Rust/CPI/JS/Dart clients in one command.
pina codama generate

# Run the complete Codama pipeline.
codama:test

# Run IDL fixture drift + validation checks used by CI.
test:idl

# Run Quasar SVM generated-client e2e checks alongside LiteSVM.
pnpm run test:quasar-svm
```

<!-- {/codamaWorkflowCommands} -->

Rust client generation in this repository uses the custom `pina_codama_renderer` crate (`crates/pina_codama_renderer`) instead of Codama's default Rust renderer. Generated Rust models are native PinaPod schemas with discriminator-first storage views and recursive content validation. Instruction builders own and consume an initialized wire buffer; they do not expose a whole-object `to_bytes()` API. Unsupported variable-size or noncanonical layouts fail generation explicitly.

`pina codama generate` also augments the stock JavaScript output with PinaPod boundary checks. Generated encoders reject values that exceed fixed or compact field capacity rather than truncating them. Decoders enforce discriminators, prefixes, strict UTF-8, and canonical boolean and option tags. Rendering a Pina IDL with the stock Codama JavaScript visitor alone does not add those runtime checks.

End-to-end setup steps:

1. Enter the dev environment: `devenv shell`
2. Install pinned binaries and external tools: `install:all`
3. Generate all IDLs: `codama:idl:all`
4. Generate clients from the IDLs: `codama:clients:generate`
5. Run the full validation pipeline: `codama:test`

If `pnpm-workspace.yaml` sets `useNodeVersion`, pnpm-run scripts automatically use that pinned Node toolchain; the shell itself provides the Nix-managed Node 24 pair (`node`/`npm`/`npx`/`corepack`).

### Using Codama in separate projects

<br>

You can use `pina idl` outside this repository to bootstrap clients in another codebase.

```sh
# Generate a Codama JSON file from your Pina program crate.
pina idl --path ./programs/my_program --output ./idls/my_program.json
```

Then render clients in your destination project:

```sh
pnpm add @pina-rs/codama-nodes codama
pnpm add -D @codama/renderers-js
```

`@pina-rs/codama-nodes` converts Pina IDLs to Codama root nodes and applies Pina's default normalization visitor. Use it when a custom client pipeline needs the same Pina-aware starting point as this repository.

```js
import { renderVisitor as renderJsVisitor } from "@codama/renderers-js";
import { createFromFile } from "codama";

const codama = await createFromFile("./idls/my_program.json");
await codama.accept(renderJsVisitor("./clients/js/my_program"));
```

For Pina-style Rust client generation with discriminator-first, validated PinaPod types, use this repository's renderer:

```sh
cargo run --manifest-path ./crates/pina_codama_renderer/Cargo.toml -- \
  --idl ./idls/my_program.json \
  --output ./clients/rust
```

<!-- {=pinaIdlVerificationContract} -->

`test:idl` treats the generated IDL as an API contract. It checks that:

- every example regenerates deterministically into `codama/idls`, `codama/clients/js`, `codama/clients/rust`, `codama/clients/cpi`, and `codama/clients/dart`
- generated JSON passes Codama's JS validator
- generated JS clients typecheck
- generated Rust and CPI clients compile
- generated Dart clients resolve with the lockfile, format cleanly, pass static analysis, and pass codec contract tests
- for every example, generated instruction/account/error counts match the source declarations:
  - `#[instruction]`
  - `#[account]`
  - `#[error]`

That last count-parity check is important because it catches silent extraction regressions where a program still produces valid JSON, but one or more instruction surfaces disappear.

<!-- {/pinaIdlVerificationContract} -->

### Crate features

<br>

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

<br>

<!-- {=pinaFeatureSelectionTips} -->

- `derive` is the normal choice for program crates; disable it only when you want the low-level runtime traits without the proc macros.
- `compact` enables `#[account(compact)]`, `PinaCompactAccount`, generated patch types, checked compact loaders, and `pina::String` and `pina::Vec`. It also enables `derive`.
- `validation` enables `PinaValidate` and `#[pina(validate(...))]` rules on accounts, instructions, events, and derived account lists. It also enables `derive`.
- `logs` is useful during **initial development and debugging**, testing, and audits. Disable it when you want the smallest possible binary or completely silent runtime failures.
- `token` enables `pina::token`, `pina::token_2022`, `pina::associated_token_account`, and the `TokenAccount` compatibility aliases over the upstream renamed account types.
- `memo` is separate from `token`, so memo CPI support can be enabled without pulling in the token helper surface.
- `account-resize` enables `ReallocAccount` and `ReallocAccountZeroed`. Enable it together with `compact` for `UpdateResizableAccount`, `ReallocCompactAccount`, and the compact creation builders. Close helpers still do not implicitly resize or zero account data.

<!-- {/pinaFeatureSelectionTips} -->

## Documentation

<br>

Project documentation lives in the mdBook under `docs/`.

For repository-level security posture and reporting guidance, see [SECURITY.md](./SECURITY.md). For example-driven guidance, see [security/readme.md](./security/readme.md). Programs upgrading from PinaPod v0.1 can follow the [PinaPod v0.2 migration guide](./docs/src/migrations/pinapod-v0.2.md). Programs updating token-loader calls can follow the [safe token loader migration guide](./docs/src/migrations/safe-token-loaders.md).

<!-- {=docsBuildCommand} -->

```bash
docs:build
```

<!-- {/docsBuildCommand} -->

Use `verify:docs` to validate documentation structure and build output in CI. Use `test:idl` to regenerate and verify `codama/idls/*.json` plus `codama/clients/{rust,js,dart}/*` against all examples. Reusable command snippets are managed by `mdt`; run `docs:sync` after changing files in `templates/`.

## Quick start

<br>

You can scaffold a new project with the CLI:

```sh
pina init my_program
```

Or add Pina to an existing crate:

```rust
use pina::*;

// 1. Declare your program ID.
declare_id_program!("YourProgramId11111111111111111111111111111111");

// 2. Define a discriminator enum for your instructions.
#[discriminator]
pub enum MyInstruction {
	Initialize = 0,
	Update = 1,
}

// 3. Define instruction data.
#[instruction(discriminator = MyInstruction)]
pub struct Initialize {
	pub value: u8,
}

// 4. Define your accounts struct.
#[derive(Accounts)]
pub struct InitializeAccounts<'a> {
	pub payer: &'a AccountView,
	pub state: &'a mut AccountView,
	pub system_program: &'a AccountView,
}

// 5. Wire up the entrypoint.
nostd_entrypoint!(process_instruction);

fn process_instruction(
	program_id: &Address,
	accounts: &mut [AccountView],
	data: &[u8],
) -> ProgramResult {
	let instruction: MyInstruction = parse_instruction(program_id, &ID, data)?;
	match instruction {
		MyInstruction::Initialize => InitializeAccounts::try_from(accounts)?.process(data),
		MyInstruction::Update => {
			// ...
			Ok(())
		}
	}
}
```

## Core concepts

<br>

### Entrypoint

<br>

The `nostd_entrypoint!` macro sets up the BPF entrypoint, disables the default allocator, and installs a minimal panic handler:

```rust
nostd_entrypoint!(process_instruction);

fn process_instruction(
	program_id: &Address,
	accounts: &mut [AccountView],
	data: &[u8],
) -> ProgramResult {
	// Your instruction dispatch logic here
	Ok(())
}
```

An optional second argument overrides the maximum number of transaction accounts (defaults to `pinocchio::MAX_TX_ACCOUNTS`).

### Discriminators

<br>

Every account, instruction, and event type carries a discriminator enum as its first field. This enables safe type identification at runtime.

```rust
use pina::*;

// Define the discriminator enum with a primitive backing type.
// Supported: u8, u16, u32, u64.
#[discriminator]
pub enum MyAccount {
	Config = 0,
	Game = 1,
}
```

### Discriminator layout guidance

Pina supports the same discriminator-first layout in both account and instruction types. For migration planning and width tradeoffs, use this matrix:

<!-- {=pinaDiscriminatorLayoutDecisionMatrix} -->

### Discriminator layout decision matrix

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

<!-- {=pinaDiscriminatorVersionCompatibility} -->

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

The `#[discriminator]` macro generates:

- `TryFrom<primitive>` and `Into<primitive>` conversions
- `IntoDiscriminator` implementation (read/write/match discriminator bytes)

Optional attributes:

- `primitive = u16` — override the backing type (default: `u8`)
- `final` — omits the `#[non_exhaustive]` attribute so the enum must be matched exhaustively

### Accounts (on-chain state)

<br>

The `#[account]` macro treats the struct as a native schema. It injects the discriminator, derives `PinaPod`, and exposes validated `ConfigZc` views over caller-owned account bytes:

```rust
use pina::*;

#[discriminator]
pub enum MyAccount {
	Config = 0,
}

#[account(discriminator = MyAccount)]
pub struct Config {
	pub authority: Address,
	pub value: u64,
	pub label: String<32>,
	pub checkpoints: Vec<u64, 8>,
	pub delegate: Option<Address>,
	pub bump: u8,
}
```

The generated struct has an auto-injected `discriminator` field as the first field. This ordinary account is fixed-size: bounded strings, vectors, and options reserve their complete capacity in `Config::SIZE`. Use `PodString<N, PFX>` or `PodVec<T, N, PFX>` when the layout needs an explicit prefix width.

<!-- {=compactAccountQuickstart} -->

Compact mode stores a fixed header followed by one or more bounded tails. Enable `compact` for the schema and checked loaders. Enable `account-resize` to apply a patch and adjust rent in one operation:

```toml
[dependencies]
pina = { version = "...", features = ["compact", "account-resize"] }
```

The `compact` feature also enables `derive`. Declare fixed fields first, then the compact tails. `String<N>` uses a one-byte length prefix, and `Vec<T, N>` uses a two-byte length prefix. Use `PodString<N, PFX>` or `PodVec<T, N, PFX>` when the schema needs an explicit prefix width. `PFX` is a byte count and must be `1`, `2`, `4`, or `8`.

```rust
#[account(discriminator = AccountType, compact)]
pub struct Journal {
	pub bump: u8,
	pub authority: Address,
	pub revision: u32,
	pub featured_entry: Option<u64>,
	pub title: String<24>,
	pub entries: Vec<u64, 8>,
	pub markers: PodVec<u8, 8, 8>,
	pub note: Option<String<64>>,
}
```

The compact grammar accepts these tail forms:

- `String<N>`
- `Vec<T, N>` where `T` has a fixed PinaPod representation
- `Option<T>` where `T` has a fixed PinaPod representation
- `Option<String<N>>`
- `Option<Vec<T, N>>` where `T` has a fixed PinaPod representation
- `Vec<String<M>, N>`

`Option<T>` for fixed `T` stays in the header. The other forms use tail storage. A compact schema can contain several tails, but it cannot place a fixed field after the first tail. The macro rejects unsupported nesting and prints the accepted forms in its error.

The macro generates `JournalHeader`, `JournalRef`, and `JournalPatch`. It also generates `HEADER_SIZE`, `MIN_SIZE`, `MAX_SIZE`, checked reads, initialization, projected-size calculation, and atomic updates. `MIN_SIZE` equals `HEADER_SIZE`. Tail prefixes live in the header except for a present `Option<String<N>>` or `Option<Vec<T, N>>`, whose payload retains its own prefix. Each active element of `Vec<String<M>, N>` occupies the fixed `String<M>` footprint, although each string keeps its own logical length.

Pina uses PinaPod for validated alignment-one storage. PinaPod initializes inactive collection capacity and validates each active nested value before Pina returns safe access.

When a compact account also declares `#[pda(..., bump = bump)]`, the macro generates `Type::with_pda`. This closure-scoped loader checks owner, compact data, the canonical stored bump, and the derived account address while one runtime borrow remains active.

<!-- {/compactAccountQuickstart} -->

<!-- {=compactAccountResizeOrdering} -->

Apply all compact changes through one patch:

```rust
UpdateResizableAccount {
	account: self.journal,
	rent_account: self.authority,
	program_id: &ID,
	patch: JournalPatch::new()
		.revision(next_revision)
		.replace_entries(&entries)
		.note(Some("Updated")),
}
.invoke::<Journal>()?;
```

`UpdateResizableAccount` preflights the patch's structural representation and calculates the final encoded length before it changes the account. It grows the allocation before applying a longer representation. For a shorter representation, it applies the patch before shrinking the allocation. If the allocation stays the same size, the builder skips the resize. It adjusts the rent balance through `rent_account` and clears bytes removed by the patch. A structural or size preflight failure leaves account data and lamports unchanged.

With the `validation` feature, Pina checks application rules on the completed compact representation after applying the patch. Always propagate an update error with `?`; Solana transaction rollback is what restores the previous bytes and any rent moved earlier in the instruction.

Use `invoke_signed::<Journal>(signers)` when `rent_account` is a PDA that must sign the system transfer used for growth. The patch and resize ordering stay the same.

The `rent_account` field has the same meaning across `UpdateResizableAccount`, `ReallocAccount`, `ReallocAccountZeroed`, and `ReallocCompactAccount`: it funds growth and receives a shrink refund. The lower-level builders take an explicit `target_size`; the high-level builder derives it from the patch.

The generated patch owns the update plan, so callers do not coordinate `set_*`, `commit`, and `ReallocCompactAccount`. Use `Journal::with_pda` to read a stored-bump compact PDA without a separate `assert_compact_type` or `assert_seeds` pass. End the closure before invoking `UpdateResizableAccount`.

<!-- {/compactAccountResizeOrdering} -->

### Instructions

<br>

The `#[instruction]` macro works the same as `#[account]` but for instruction data:

```rust
use pina::*;

#[discriminator]
pub enum MyInstruction {
	Initialize = 0,
}

#[instruction(discriminator = MyInstruction)]
pub struct Initialize {
	pub value: PodU64,
	pub bump: u8,
}
```

### Events

<br>

```rust
use pina::*;

#[discriminator]
pub enum MyEvent {
	Transfer = 0,
}

#[event(discriminator = MyEvent)]
pub struct Transfer {
	pub amount: PodU64,
}
```

### Errors

<br>

The `#[error]` macro creates a custom error enum that converts to `ProgramError::Custom(code)`:

```rust
use pina::*;

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MyError {
	InvalidAuthority = 0,
	InsufficientFunds = 1,
}
```

### Account validation chains

<br>

Chain assertions on `AccountView` references. Each method returns the same reference type it received, so shared borrows stay shared and mutable borrows stay mutable:

```rust
// Validate an account is a signer, writable, and owned by our program.
account.assert_signer()?.assert_writable()?.assert_owner(&program_id)?;

// Validate a PDA with seeds and bump.
escrow.assert_seeds_with_bump(&[b"escrow", maker_key], &program_id)?;

// Validate an associated token account.
vault.assert_associated_token_address(wallet, mint, token_program)?;

// Validate a fixed account without loading its fields.
state.assert_type::<Config>(&program_id)?;
```

Available assertions:

- `assert_signer()` — account is a signer
- `assert_writable()` — account is writable
- `assert_executable()` — account is executable
- `assert_data_len(len)` — data length check
- `assert_empty()` / `assert_not_empty()` — data emptiness
- `assert_type::<T>(program_id)` — validation-only owner, discriminator, exact-size, and nested-value check
- `assert_program(program_id)` — is a program account
- `assert_sysvar(sysvar_id)` — is a system variable
- `assert_address(address)` — exact address match
- `assert_addresses(addresses)` — address is one of the given set
- `assert_owner(owner)` — owned by the given program
- `assert_owners(owners)` — owned by one of the given programs
- `assert_seeds(seeds, program_id)` — PDA with canonical bump
- `assert_seeds_with_bump(seeds, program_id)` — PDA with explicit bump
- `assert_canonical_bump(seeds, program_id)` — returns the canonical bump
- `assert_associated_token_address(wallet, mint, token_program)` — ATA check (requires `token` feature)

Use `assert_program()` when you explicitly validate a program account. Static `.invoke()` and `.invoke_signed()` builders encode their program ID. Pinocchio Token's `.invoke_with_program()` and `.invoke_signed_with_program()` methods validate their supplied ID with `Program::verify()`. Neither form needs a preceding account assertion.

If you call `.invoke_with_unverified_program()` or `.invoke_signed_with_unverified_program()`, validate the exact supplied account first with Pina's assertion API and a compile-time program ID. An instruction argument is not a trusted expected ID. Prefer `assert_program()`, propagate assertion failure on every continuing path, and call the method directly. You can bind or chain from the account value returned by the assertion. Success-side `Result` callbacks and closures that may replace the validated binding invalidate the proof. Do not store an unverified CPI method as a function value.

When you need sysvar data, prefer Pinocchio's checked typed loaders such as `Clock::from_account_view()`, `Rent::from_account_view()`, and `Instructions::try_from()`. The sysvar lint rejects `Clock` and `Rent` byte constructors, `Instructions::new_unchecked`, and `SlotHashes::new` or `new_unchecked` because they do not validate sysvar identity.

For deliberate raw access, successfully call Pina's `assert_sysvar()` with the matching `pina_sdk_ids::sysvar::<name>::ID` on every continuing path. The canonical ID also supports generic bindings such as `epoch_sysvar` whose name does not identify a specific sysvar. You can bind or chain from the account value returned by the assertion. Success-side `Result` callbacks and closures that may replace the asserted binding invalidate the proof. Call a reviewed unchecked constructor directly after that assertion. Place a narrow lint allowance on the constructor. Identity-unchecked constructors cannot be hidden in function values.

When you need token data, use the checked loader instead of an assertion followed by a second parse:

```rust
let legacy = account.as_token_account()?;
let token_2022 = account_2022.as_token_2022_account()?;
let selected = account.as_token_account_for_program(token_program)?;
let ata = vault.as_associated_token_account(wallet, mint, token_program)?;
```

The unqualified loaders delegate owner and layout validation to their canonical program's checked upstream parser. The `*_for_program()` loaders accept only SPL Token or Token-2022 and select the corresponding checked parser. `as_associated_token_account()` also verifies the derived address and the current authority and mint stored in account data; initialization, frozen state, delegates, close authority, and Token-2022 extension policy remain explicit caller checks.

### Typed account loading

<br>

When code needs account fields, load the account directly instead of calling `assert_type` first. `as_account()` performs the same fixed-account validation and returns a guard-backed `Ref<T>` (also available as the `LoadedAccount<T>` alias). `as_account_mut()` also checks writability and returns `RefMut<T>` (also available as the `LoadedAccountMut<T>` alias):

```rust
let state = account.as_account::<Config>(&program_id)?;
state.assert(|s| s.value > PodU64::from(0))?;
state.assert_msg(|s| s.bump == 255, "bump must be 255")?;
```

For fixed accounts with a stored `#[pda(bump = ...)]`, prefer the generated one-pass loader when the handler needs state immediately:

```rust
let mut state = Config::load_pda_mut(account, authority.address(), &program_id)?;
state.value.set(42);
```

`load_pda` and `load_pda_mut` validate the typed representation and stored-bump PDA address before returning a guard. The mutable form also enforces writability. This avoids repeating recursive `String`, `Vec`, and `Option` validation through separate `assert_type`, `assert_seeds`, and `as_account_mut` calls.

Use `assert_type::<T>()` only when the handler needs to validate an existing fixed account but will not read or write its typed fields, such as a validation-only close path. The check releases its data borrow before returning, so it is not a persistent proof for a later raw cast. Never follow it with `as_account*` or `load_pda*`; that only repeats the same validation.

### `#[derive(Accounts)]`

<br>

Automatically destructures a mutable slice of `AccountView` into a named struct:

```rust
use pina::*;

#[derive(Accounts)]
pub struct MyAccounts<'a> {
	pub payer: &'a AccountView,
	pub state: &'a AccountView,
	pub system_program: &'a AccountView,
}
```

The derive generates `TryFromAccountInfos` and `TryFrom<&mut [AccountView]>` implementations. Internally it uses `AccountsCursor` to walk the account slice left-to-right and reject writable aliases for mutable accounts parsed individually through `next_mut()`, without heap allocation. It validates that the exact number of accounts is provided unless the final field captures the remaining accounts, and it supports `&'a AccountView`, `&'a mut AccountView`, `&'a [AccountView]`, and `&'a mut [AccountView]` fields.

Use the `#[pina(remaining)]` attribute on the last field to capture trailing accounts:

```rust
#[derive(Accounts)]
pub struct MyAccounts<'a> {
	pub payer: &'a AccountView,
	#[pina(remaining)]
	pub remaining: &'a [AccountView],
}
```

Remaining-account fields preserve account order. Mutable `#[pina(remaining)]` fields reject duplicate addresses by default; use `#[pina(remaining, distinct = false)]` only when aliases are intentional, and add a field doc comment explaining the invariant that makes them safe.

### Instruction authoring tips

<br>

<!-- {=pinaInstructionAuthoringTips} -->

- Entry points should accept `&mut [AccountView]` and dispatch with `Accounts::try_from((program_id, accounts))?.process(data)`.
- Use `&AccountView` for read-only accounts and `&mut AccountView` only when you need mutable loaders, direct lamport mutation, `close_*` helpers, or writable IDL inference.
- `&mut AccountView` declares and enforces a writable slot. Use `assert_writable()` or `#[pina(validate(writable))]` only when a shared `&AccountView` must arrive writable.
- `as_account()` / `as_account_mut()` return `Ref<T>` / `RefMut<T>` borrow guards. Copy out the fields you need and `drop(...)` the guard before CPIs or later mutable borrows.
- Prefer generated `load_pda*` methods for stored-bump fixed PDAs, then `as_account*` for other fixed accounts. Use `assert_type` only when no typed fields are needed; do not call it before a typed loader.
- Keep validation chains direct inside `process(self, ...)` when possible. That makes audits easier and gives `pina idl` the clearest signal for signer, writable, PDA, and default-account inference.

<!-- {/pinaInstructionAuthoringTips} -->

### Pod types

<br>

Alignment-safe primitive wrappers for use in `#[repr(C)]` account structs. Solana account data is byte-aligned, so standard Rust integers cannot be placed directly in `Pod` structs.

<!-- {=podTypesTable} -->

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

Usage:

```rust
use pina::*;

#[account(discriminator = MyAccount)]
pub struct State {
    pub amount: PodU64,
    pub count: PodU32,
    pub active: PodBool,
}

// Create values.
let amount = PodU64::from(1_000_000);

// Convert back.
let raw: u64 = amount.into();

// Ergonomic arithmetic (debug: checked, release: wrapping).
let mut count = PodU64::from(0u64);
count += 1u64;

// Checked/saturating variants for explicit overflow handling.
let fee = amount.checked_mul(3u64);
let clamped = amount.saturating_add(PodU64::MAX);
```

<!-- {=podArithmeticDescription} -->

Arithmetic operators (`+`, `-`, `*`) on Pod **integer** types use **wrapping** semantics in release builds for CU efficiency and **panic on overflow** in debug builds. Use `checked_add`, `checked_sub`, `checked_mul`, `checked_div` where overflow must be detected in all build profiles.

Each Pod integer type provides `ZERO`, `MIN`, and `MAX` constants.

<!-- {/podArithmeticDescription} -->

### Pod collection types

<br>

Fixed-capacity collections that store fully initialized data inline without allocation.

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

### CPI helpers

<br>

#### Account creation

```rust
use pina::*;

// Create a simple account (non-PDA).
CreateAccount { from, to, space, owner: &owner }.invoke()?;

// Create a PDA account (finds canonical bump automatically).
let (address, bump) = CreateProgramAccount {
    account: target,
    payer,
    owner: &program_id,
    seeds: &[b"seed"],
}
.invoke::<MyState>()?;

// Create a PDA account with a supplied canonical bump.
CreateProgramAccountWithBump {
    account: target,
    payer,
    owner: &program_id,
    seeds: &[b"seed"],
    bump,
}
.invoke::<MyState>()?;

// Derive once, then store the same canonical bump during initialization.
CreateProgramAccount {
    account: target,
    payer,
    owner: &program_id,
    seeds: &[b"seed"],
}
.invoke_with_bump::<MyState>(|state, bump| {
    state.bump = bump;
    state.authority = authority;
    state.name.try_set("Alice")?;
    Ok(())
})?;
```

`invoke` and `invoke_signed` leave every field other than the discriminator at zero. Use them only when that is a valid completed representation. `invoke_with` and `invoke_signed_with` accept a `Result<(), PinaPodError>` initializer and validate after it configures the generated zero-copy view. This is required for an advanced fixed schema with a nonzero-only storage enum, and it also avoids a separate mutable borrow for ordinary initial values.

`invoke_with_bump` and `invoke_signed_with_bump` pass the derived canonical bump into the initializer. Explicit-bump creation builders verify canonicality themselves. Do not call `assert_canonical_bump` or `assert_seeds_with_bump` immediately before a creation builder, because that repeats PDA derivation.

#### Lamport transfers

```rust
use pina::*;

// Direct debit: verifies that this program owns the sender.
source.send_owned(&ID, 1_000_000, destination)?;
// System-program CPI credit: the source account must sign.
destination.collect(1_000_000, source)?;

// Close an account and return rent to recipient.
account.close_with_recipient(&ID, recipient)?;
```

#### Closing safety

<br>

<!-- {=pinaCloseAccountGuidance} -->

Closing guidance under Pinocchio 0.11:

- `close_with_recipient(&ID, recipient)` verifies that `ID` owns the account, transfers its lamports, and closes the account handle. It does not zero or resize account data.
- When stale bytes must be invalidated, use `CloseAccountZeroed { account, recipient, program_id: &ID }.invoke()` or manually call `zeroed()` before `close_with_recipient(&ID, recipient)`.
- The `account-resize` feature only affects realloc helpers; it does not change close semantics.

<!-- {/pinaCloseAccountGuidance} -->

#### PDA seed combination

```rust
use pina::*;

// Combine seeds with a bump for PDA signing.
let seeds = &[b"escrow", maker_key];
let bump = [255u8; 1];
let combined = combine_seeds_with_bump(seeds, &bump)?;
let signer = Signer::from(&combined[..=seeds.len()]);
```

### Logging

<br>

The `log!` macro logs messages to the Solana runtime (requires the `logs` feature):

```rust
use pina::*;

log!("simple message");
```

When the `logs` feature is disabled, `log!` compiles to nothing.

## Building for SBF (on-chain)

<br>

<!-- {=sbfBuildInstructions} -->

Programs are compiled to the `bpfel-unknown-none` target using `sbpf-linker`:

```sh
cargo build --release --target bpfel-unknown-none -p my_program -Z build-std=core,alloc -F bpf-entrypoint
```

The pinned nightly toolchain from `rust-toolchain.toml` runs the build; the `bpf-entrypoint` feature gate separates the on-chain entrypoint from the library code used in tests.

<!-- {/sbfBuildInstructions} -->

## Testing

<br>

<!-- {=pinaTestingInstructions} -->

Programs are tested as regular Rust libraries (without the `bpf-entrypoint` feature) using [mollusk-svm](https://docs.rs/mollusk-svm) for Solana VM simulation:

```sh
cargo test
cargo nextest run  # Faster parallel test execution
```

<!-- {/pinaTestingInstructions} -->

For host-side undefined-behavior and borrow-model checks on the loader layer, run the dedicated Miri suite:

```sh
rustup component add miri --toolchain nightly-2026-02-20
MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check" \
  cargo +nightly-2026-02-20 miri test -p pina --test miri_loader_guards --all-features

# Or, inside `devenv shell`:
test:miri
```

## Static CU Profiling

<br>

<!-- {=pinaProfileDescription} -->

The `pina profile` command analyzes compiled SBF `.so` binaries to estimate per-function compute unit costs without requiring a running validator.

```sh
pina profile target/deploy/my_program.so          # text summary
pina profile target/deploy/my_program.so --json    # JSON for CI
pina profile target/deploy/my_program.so -o r.json # write to file
```

The profiler decodes each SBF instruction opcode and assigns costs: regular instructions cost 1 CU, syscalls cost 100 CU.

<!-- {/pinaProfileDescription} -->

### CLI configuration

The `pina docs` subcommand renders built-in reference topics. Set the `PINA_TEMPLATES_DIR` environment variable to a directory containing `<topic>.t.md` template files to override or extend the default topics with your own content.

## Packages

<br>

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

## Ideology

<br>

- Macros are minimal syntactic sugar to reduce repetition of code.
- IDL generation is automated based on code you write, rather than annotations. So `payer.assert_signer()?` will generate an IDL that specifies that the account is a signer.
- One language end to end — from the on-chain program to the browser client — in whichever language you prefer.

## Examples

<br>

| Example                                                                             | Description                                                                                   |
| ----------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| [`hello_solana_program`](examples/hello_solana_program)                             | Minimal program — entrypoint, accounts, logging                                               |
| [`counter_program`](examples/counter_program)                                       | PDA state management with initialize and increment                                            |
| [`transfer_sol_program`](examples/transfer_sol_program)                             | CPI and direct lamport transfers                                                              |
| [`escrow_program`](examples/escrow_program)                                         | Full token escrow with SPL token operations                                                   |
| [`vesting_program`](examples/vesting_program)                                       | Schedule-state and vault-ATA scaffold; does not transfer tokens or enforce time-based vesting |
| [`role_registry_program`](examples/role_registry_program)                           | Role-based configuration and registry PDAs                                                    |
| [`staking_rewards_program`](examples/staking_rewards_program)                       | Staking pool and user-position accounting scaffold                                            |
| [`pina_bpf_program`](examples/pina_bpf_program)                                     | Minimal pina-native BPF hello world (nightly + `build-std=core,alloc`)                        |
| [`declare_id_program`](examples/declare_id_program)                                 | Anchor `declare-id` test parity port for program-id mismatch                                  |
| [`declare_program`](examples/declare_program)                                       | Anchor `declare-program` parity port for external-program ID checks                           |
| [`duplicate_mutable_accounts_program`](examples/duplicate_mutable_accounts_program) | Anchor duplicate mutable account checks adapted to explicit pina validation                   |
| [`custom_errors_program`](examples/custom_errors_program)                           | Anchor custom error-code parity and guard helper checks                                       |
| [`events_program`](examples/events_program)                                         | Anchor event schema parity via deterministic event serialization                              |
| [`float_accounts_program`](examples/float_accounts_program)                         | Anchor float account/update behavior with authority checks                                    |
| [`system_accounts_program`](examples/system_accounts_program)                       | Anchor system-owned account constraint parity                                                 |
| [`sysvar_checks_program`](examples/sysvar_checks_program)                           | Anchor sysvar account validation parity                                                       |
| [`account_realloc_program`](examples/account_realloc_program)                       | Dynamic compact account lifecycle with typed, rent-adjusted patches                           |
| [`compact_accounts_program`](examples/compact_accounts_program)                     | Atomic compact patches, rent adjustment, and generated clients                                |
| [`todo_program`](examples/todo_program)                                             | PDA-backed state with boolean and digest updates                                              |
| [`profile_program`](examples/profile_program)                                       | User profile registry with bounded UTF-8 and tag fields                                       |
| [`prop_amm_program`](examples/prop_amm_program)                                     | Anchor `prop-amm` port focused on authority-controlled oracle updates                         |
| [`optional_accounts_program`](examples/optional_accounts_program)                   | Optional-account slots with explicit presence handling                                        |

## Security

<br>

Pina provides strong built-in protections against common Solana vulnerabilities through its validation chain API, discriminator system, and CPI helpers. Follow these best practices:

<!-- {=pinaSecurityBestPractices} -->

- **Always call `assert_signer()`** before trusting authority accounts
- **Use Pina's token loaders directly** because they delegate canonical owner and layout validation to the corresponding checked upstream parser before returning typed state
- **Use `as_associated_token_account()`** when reading a canonical ATA because it validates the runtime owner, derived address, stored current authority, and stored mint together; enforce state, delegate, close-authority, and extension policy separately
- **Always call `assert_empty()`** before account initialization to prevent reinitialization attacks
- **Use `invoke_with` or `invoke_signed_with`** when fixed-account creation must establish nonzero values before final PinaPod validation
- **Use generated `load_pda` or `load_pda_mut`** when a fixed stored-bump PDA handler needs a typed guard, so recursive content and the PDA address are validated once
- **Use generated `with_pda`** when a compact stored-bump PDA handler needs a compact view, so the layout, canonical bump, and PDA address are validated during the same borrow
- **Validate dynamic program accounts** with Pina's `assert_program()` before explicitly unverified CPI invocations; static and self-verifying CPI APIs need no redundant assertion
- **Use `as_account::<T>()` or `as_account_mut::<T>()`** when a handler needs fixed-account fields; these guard-backed loaders check the owner, discriminator, exact size, and nested values
- **Reserve `assert_type::<T>()` for validation-only paths** that do not need typed fields, and never treat it as proof for a later raw cast
- **Use `send_owned(&ID, amount, recipient)`** for direct lamport debits; it verifies that the program owns the sender before mutation
- **Use `CloseAccountZeroed { account, recipient, program_id: &ID }.invoke()` or `zeroed()` + `close_with_recipient(&ID, recipient)`** when stale account bytes must be invalidated before close
- **Use `CreateProgramAccount` or `CreateCompactProgramAccount` for canonical PDA creation**; their explicit-bump variants also reject noncanonical bumps without separate seed assertions
- **Keep `assert_seeds()` / `assert_canonical_bump()` for validation-only paths** that are not immediately followed by a checked creation builder
- **Give each account type its own seed namespace** so PDAs cannot collide across account types

<!-- {/pinaSecurityBestPractices} -->

See the [security guide](security/) for detailed examples of all 11 common Solana attack categories with vulnerable and secure code patterns.

### Custom lints

<br>

Enable Pina's official security lints to catch common mistakes at compile time:

```sh
pina lint
```

Every lint lives in the [`pina_lints`](crates/pina_lints) crate and is statically compiled into the `pina_lint_driver` binary. `pina lint` installs that driver for the active toolchain below Cargo home on first use and runs `cargo check` with the driver as `RUSTC_WRAPPER`, so no external lint tooling or precompiled lint bundles are downloaded. Lint levels are configured through the `[lints]` table of the project's `pina.toml`.

See the [complete lint reference](crates/pina_lints/readme.md) for every lint's severity, detected patterns, compliant examples, and analysis limitations. This repository runs the entire catalog over every program under `examples/` and every secure security fixture with `devenv shell -- security:pina-lint`.

## Contributing

<br>

Contributions are welcome! Please open an issue or pull request on [GitHub](https://github.com/pina-rs/pina).

## License

<br>

Licensed under the [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0).
