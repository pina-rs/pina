# pina

A performant Solana smart contract framework built on top of [pinocchio](https://github.com/anza-xyz/pinocchio) — a zero-dependency alternative to `solana-program` that massively reduces compute units and dependency bloat.

## Features

- **Zero-copy deserialization** — account data is reinterpreted in place via `bytemuck`, with no heap allocation.
- **`no_std` compatible** — all crates compile to the `bpfel-unknown-none` SBF target for on-chain deployment.
- **Low compute units** — built on `pinocchio` instead of `solana-program`, saving thousands of CU per instruction.
- **Discriminator system** — every account, instruction, and event type carries a typed discriminator as its first field.
- **Validation chaining** — chain assertions on `AccountView` references:
  ```rust
  account.assert_signer()?.assert_writable()?.assert_owner(&program_id)?;
  ```
- **Proc-macro sugar** — `#[account]`, `#[instruction]`, `#[event]`, `#[error]`, `#[discriminator]`, and `#[derive(Accounts)]` eliminate boilerplate.
- **CPI helpers** — PDA account creation, lamport transfers, and token operations.

## Installation

```sh
cargo add pina
```

To enable SPL token support:

```sh
cargo add pina --features token
```

## Codama IDL Support

Pina ships with first-class Codama integration through the `pina` CLI and the `codama/` test harness in this repository.

The CLI command below generates a Codama-compatible IDL from a Pina program:

```sh
pina idl --path examples/counter_program --output codama/idls/counter_program.json
```

With `devenv`, the full workflow is available via built-in scripts:

```sh
# Generate IDLs for all example programs into codama/idls/.
codama:idl:all

# Generate Rust + JS clients from codama/idls/.
codama:clients:generate

# Run the full integration pipeline (build CLI, generate IDLs, generate clients, validate outputs).
codama:test
```

Rust client generation in this repository uses the custom `pina_codama_renderer` crate (`codama/pina_codama_renderer`) instead of Codama's default Rust renderer. The generated Rust models are Pina-compatible: discriminator-first layouts and bytemuck-based POD wrappers, without `borsh` serialization requirements. Because these clients are generated as fixed-size POD layouts, unsupported Codama patterns (e.g. variable-length strings/bytes, big-endian numbers, floats, non-UTF8 constant byte seeds, and non-fixed arrays) will fail generation with explicit renderer errors.

End-to-end setup steps:

1. Enter the dev environment: `devenv shell`
2. Install pinned binaries and external tools: `install:all`
3. Generate all IDLs: `codama:idl:all`
4. Generate clients from the IDLs: `codama:clients:generate`
5. Run the full validation pipeline: `codama:test`

### Crate features

| Feature  | Default | Description                                                |
| -------- | ------- | ---------------------------------------------------------- |
| `derive` | Yes     | Enables proc macros (`#[account]`, `#[instruction]`, etc.) |
| `logs`   | Yes     | Enables on-chain logging via `solana-program-log`          |
| `token`  | No      | Enables SPL token / token-2022 helpers and ATA utilities   |

## Documentation

Comprehensive project documentation now lives in the mdBook under `docs/`.

```sh
docs:build
```

Use `verify:docs` to validate documentation structure and build output in CI. Use `test:idl` to verify `codama/idls/anchor_*.json` against fresh output and Codama Rust/JS validators.

## Quick start

```rust
use pina::*;

// 1. Declare your program ID.
declare_id!("YourProgramId11111111111111111111111111111111");

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
	pub state: &'a AccountView,
	pub system_program: &'a AccountView,
}

// 5. Wire up the entrypoint.
nostd_entrypoint!(process_instruction);

fn process_instruction(
	program_id: &Address,
	accounts: &[AccountView],
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

### Entrypoint

The `nostd_entrypoint!` macro sets up the BPF entrypoint, disables the default allocator, and installs a minimal panic handler:

```rust
nostd_entrypoint!(process_instruction);

fn process_instruction(
	program_id: &Address,
	accounts: &[AccountView],
	data: &[u8],
) -> ProgramResult {
	// Your instruction dispatch logic here
	Ok(())
}
```

An optional second argument overrides the maximum number of transaction accounts (defaults to `pinocchio::MAX_TX_ACCOUNTS`).

### Discriminators

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

The `#[discriminator]` macro generates:

- `Pod` / `Zeroable` derives for the enum
- `TryFrom<primitive>` and `Into<primitive>` conversions
- `IntoDiscriminator` implementation (read/write/match discriminator bytes)

Optional attributes:

- `primitive = u16` — override the backing type (default: `u8`)
- `final` — marks the enum as a final discriminator (generates a `BYTES` constant)

### Accounts (on-chain state)

The `#[account]` macro wraps a struct with a discriminator field and derives `Pod`, `Zeroable`, `TypedBuilder`, and `HasDiscriminator`:

```rust
use pina::*;

#[discriminator]
pub enum MyAccount {
	Config = 0,
}

#[account(discriminator = MyAccount)]
pub struct Config {
	pub authority: Address,
	pub value: PodU64,
	pub bump: u8,
}
```

The generated struct has an auto-injected `discriminator` field as the first field.

### Instructions

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

Chain assertions on `AccountView` references. Each method returns `Result<&AccountView, ProgramError>` for fluent chaining:

```rust
// Validate an account is a signer, writable, and owned by our program.
account.assert_signer()?.assert_writable()?.assert_owner(&program_id)?;

// Validate a PDA with seeds and bump.
escrow.assert_seeds_with_bump(&[b"escrow", maker_key], &program_id)?;

// Validate an associated token account.
vault.assert_associated_token_address(wallet, mint, token_program)?;

// Validate account data matches a typed account.
state.assert_type::<Config>(&program_id)?;
```

Available assertions:

- `assert_signer()` — account is a signer
- `assert_writable()` — account is writable
- `assert_executable()` — account is executable
- `assert_data_len(len)` — data length check
- `assert_empty()` / `assert_not_empty()` — data emptiness
- `assert_type::<T>(program_id)` — discriminator + owner check
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

### Typed account assertion

On deserialized account data, chain assertions using the `AccountValidation` trait:

```rust
let state = account.as_account::<Config>(&program_id)?;
state.assert(|s| s.value > PodU64::from_primitive(0))?;
state.assert_msg(|s| s.bump == 255, "bump must be 255")?;
```

### `#[derive(Accounts)]`

Automatically destructures a slice of `AccountView` into a named struct:

```rust
use pina::*;

#[derive(Accounts)]
pub struct MyAccounts<'a> {
	pub payer: &'a AccountView,
	pub state: &'a AccountView,
	pub system_program: &'a AccountView,
}
```

The derive generates `TryFromAccountInfos` and `TryFrom<&[AccountView]>` implementations. It validates that the exact number of accounts is provided.

Use the `#[pina(remaining)]` attribute on the last field to capture trailing accounts:

```rust
#[derive(Accounts)]
pub struct MyAccounts<'a> {
	pub payer: &'a AccountView,
	#[pina(remaining)]
	pub remaining: &'a [AccountView],
}
```

### Pod types

Alignment-safe primitive wrappers for use in `#[repr(C)]` account structs. Solana account data is byte-aligned, so standard Rust integers cannot be placed directly in `Pod` structs.

| Type      | Wraps  | Size     |
| --------- | ------ | -------- |
| `PodBool` | `bool` | 1 byte   |
| `PodU16`  | `u16`  | 2 bytes  |
| `PodU32`  | `u32`  | 4 bytes  |
| `PodU64`  | `u64`  | 8 bytes  |
| `PodU128` | `u128` | 16 bytes |
| `PodI16`  | `i16`  | 2 bytes  |
| `PodI64`  | `i64`  | 8 bytes  |

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
let amount = PodU64::from_primitive(1_000_000);

// Convert back.
let raw: u64 = amount.into();
```

### CPI helpers

#### Account creation

```rust
use pina::*;

// Create a simple account (non-PDA).
create_account(from, to, space, &owner)?;

// Create a PDA account (finds canonical bump automatically).
let (address, bump) = create_program_account::<MyState>(
    target, payer, &program_id, &[b"seed"],
)?;

// Create a PDA account with a known bump.
create_program_account_with_bump::<MyState>(
    target, payer, &program_id, &[b"seed"], bump,
)?;
```

#### Lamport transfers

```rust
use pina::*;

// Direct lamport transfer between accounts.
source.send(1_000_000, destination)?;
destination.collect(1_000_000, source)?;

// Close an account and return rent to recipient.
account.close_with_recipient(recipient)?;
```

#### PDA seed combination

```rust
use pina::*;

// Combine seeds with a bump for PDA signing.
let bump = [255u8; 1];
let combined = combine_seeds_with_bump(&[b"escrow", maker_key], &bump);
let signer = Signer::from(&combined[..3]);
```

### Logging

The `log!` macro logs messages to the Solana runtime (requires the `logs` feature):

```rust
use pina::*;

log!("simple message");
```

When the `logs` feature is disabled, `log!` compiles to nothing.

## Building for SBF (on-chain)

Programs are compiled to the `bpfel-unknown-none` target using `sbpf-linker`:

```sh
cargo +nightly-2025-10-15 build --release --target bpfel-unknown-none -p my_program -Z build-std=core,alloc -F bpf-entrypoint
```

The `bpf-entrypoint` feature gate separates the on-chain entrypoint from the library code used in tests.

## Testing

Programs are tested as regular Rust libraries (without the `bpf-entrypoint` feature) using [mollusk-svm](https://docs.rs/mollusk-svm) for Solana VM simulation:

```sh
cargo test
cargo nextest run  # Faster parallel test execution
```

## Crates

| Crate                                 | Description                                                                |
| ------------------------------------- | -------------------------------------------------------------------------- |
| [`pina`](crates/pina)                 | Core framework — traits, account loaders, CPI helpers, Pod types, macros.  |
| [`pina_macros`](crates/pina_macros)   | Proc macros — `#[account]`, `#[instruction]`, `#[event]`, `#[error]`, etc. |
| [`pina_sdk_ids`](crates/pina_sdk_ids) | Well-known Solana program and sysvar IDs.                                  |

## Ideology

- Macros are minimal syntactic sugar to reduce repetition of code.
- IDL generation is automated based on code you write, rather than annotations. So `payer.assert_signer()?` will generate an IDL that specifies that the account is a signer.
- Everything in Rust from the on-chain program to the client code used on the browser — this project strives to make it possible to build everything in your favourite language.

## Examples

| Example                                                                           | Description                                                                 |
| --------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| [`hello_solana`](examples/hello_solana)                                           | Minimal program — entrypoint, accounts, logging                             |
| [`counter_program`](examples/counter_program)                                     | PDA state management with initialize and increment                          |
| [`transfer_sol`](examples/transfer_sol)                                           | CPI and direct lamport transfers                                            |
| [`escrow_program`](examples/escrow_program)                                       | Full token escrow with SPL token operations                                 |
| [`pina_bpf`](examples/pina_bpf)                                                   | Minimal pina-native BPF hello world (nightly + `build-std=core,alloc`)      |
| [`anchor_declare_id`](examples/anchor_declare_id)                                 | Anchor `declare-id` test parity port for program-id mismatch                |
| [`anchor_declare_program`](examples/anchor_declare_program)                       | Anchor `declare-program` parity port for external-program ID checks         |
| [`anchor_duplicate_mutable_accounts`](examples/anchor_duplicate_mutable_accounts) | Anchor duplicate mutable account checks adapted to explicit pina validation |
| [`anchor_errors`](examples/anchor_errors)                                         | Anchor custom error-code parity and guard helper checks                     |
| [`anchor_events`](examples/anchor_events)                                         | Anchor event schema parity via deterministic event serialization            |
| [`anchor_floats`](examples/anchor_floats)                                         | Anchor float account/update behavior with authority checks                  |
| [`anchor_system_accounts`](examples/anchor_system_accounts)                       | Anchor system-owned account constraint parity                               |
| [`anchor_sysvars`](examples/anchor_sysvars)                                       | Anchor sysvar account validation parity                                     |
| [`anchor_realloc`](examples/anchor_realloc)                                       | Anchor realloc constraint parity for growth-limit and duplicate checks      |

## Security

Pina provides strong built-in protections against common Solana vulnerabilities through its validation chain API, discriminator system, and CPI helpers. Follow these best practices:

- **Always call `assert_signer()`** before trusting authority accounts
- **Always call `assert_owner()` / `assert_owners()`** before `as_token_*()` methods — these perform layout casts without owner verification
- **Always call `assert_empty()`** before account initialization to prevent reinitialization attacks
- **Always verify program accounts** with `assert_address()` / `assert_program()` before CPI invocations
- **Use `assert_type::<T>()`** to prevent type cosplay — it checks discriminator, owner, and data size in one call
- **Use `close_with_recipient()` with `zeroed()`** to safely close accounts and prevent revival attacks
- **Prefer `assert_seeds()` / `assert_canonical_bump()`** over `assert_seeds_with_bump()` to enforce canonical PDA bumps
- **Namespace PDA seeds** with type-specific prefixes (e.g. `b"config"`, `b"vault"`) to prevent PDA sharing across account types

See the [security guide](security/) for detailed examples of all 11 common Solana attack categories with vulnerable and secure code patterns.

### Custom lints

Enable pina's custom dylint lints to catch common security mistakes at compile time:

- `require_owner_before_token_cast` — warns when `as_token_*()` is called without a preceding `assert_owner()`
- `require_empty_before_init` — warns when `create_program_account*()` is called without a preceding `assert_empty()`
- `require_program_check_before_cpi` — warns when `.invoke()` / `.invoke_signed()` is called without program address verification

## Contributing

Contributions are welcome! Please open an issue or pull request on [GitHub](https://github.com/pina-rs/pina).

## License

Licensed under the [Apache License, Version 2.0](license).
