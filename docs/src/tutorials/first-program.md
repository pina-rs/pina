# Your First Program

<br>

This tutorial walks through building a minimal Solana program from scratch using Pina. By the end you will have a working on-chain program that logs a greeting, complete with tests.

## Prerequisites

<br>

- A working development environment (see [Getting Started](../getting-started.md)).
- Basic familiarity with Rust and the Solana account model.

## Project setup

<br>

Create a new crate inside the workspace (or standalone):

```toml
# Cargo.toml
[package]
name = "hello_solana"
version = "0.0.0"
edition = "2024"

[lib]
crate-type = ["cdylib", "lib"]

[features]
bpf-entrypoint = []

[dependencies]
pina = { version = "...", features = ["logs", "derive"] }
```

The `cdylib` crate type is required for building a shared library that the Solana runtime can load. The `lib` type lets tests and other crates consume the program as a regular Rust library.

The `bpf-entrypoint` feature gates the on-chain entrypoint so that test builds do not pull in BPF-specific machinery.

## Step 1: Declare a program ID

<br>

Every Solana program has a unique address. `declare_id!` parses a base58 string into a constant `ID` of type `Address`:

```rust
#![no_std]

use pina::*;

declare_id!("DCF5KBmtQ9ryDC7mQezKLwuJHem6coVUCmKkw37M9J4A");
```

The `#![no_std]` attribute is required for on-chain programs. Pina is designed to work without the standard library so the resulting binary stays small and does not depend on a heap allocator.

For native (non-BPF) builds outside of tests you need a small shim to provide the standard library:

```rust
#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;
```

## Step 2: Define an instruction discriminator

<br>

Pina programs use discriminator enums to identify instruction variants. The `#[discriminator]` macro generates `TryFrom<u8>` and the framework's `IntoDiscriminator` trait:

```rust
#[discriminator]
pub enum HelloInstruction {
	Hello = 0,
}
```

The numeric value (`0`) becomes the first byte of the serialized instruction data. Clients send this byte so the program knows which handler to invoke.

## Step 3: Define instruction data

<br>

The `#[instruction]` macro creates a `Pod`/`Zeroable` struct whose first field is an auto-injected discriminator byte. It also generates a `TypedBuilder` for ergonomic construction in tests:

```rust
#[instruction(discriminator = HelloInstruction, variant = Hello)]
pub struct HelloInstructionData {}
```

This instruction has no extra payload -- it only needs the discriminator byte to be identified.

## Step 4: Define an accounts struct

<br>

`#[derive(Accounts)]` generates a `TryFromAccountInfos` implementation that maps positional accounts from the transaction into named fields:

```rust
#[derive(Accounts, Debug)]
pub struct HelloAccounts<'a> {
	pub user: &'a AccountView,
}
```

If a transaction supplies fewer accounts than the struct declares, `TryFrom` returns `ProgramError::NotEnoughAccountKeys`.

## Step 5: Implement the processor

<br>

The `ProcessAccountInfos` trait defines the `process` method that contains your instruction logic:

```rust
impl<'a> ProcessAccountInfos<'a> for HelloAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let _ = HelloInstructionData::try_from_bytes(data)?;
		self.user.assert_signer()?;
		log!("Hello, Solana!");
		Ok(())
	}
}
```

`try_from_bytes` validates that the raw instruction data is the correct size and layout. `assert_signer()` verifies the user actually signed the transaction. If any check fails the program returns an error and the transaction is rejected.

## Step 6: Wire up the entrypoint

<br>

The entrypoint module is gated behind `bpf-entrypoint` so it only compiles for on-chain builds:

```rust
#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use pina::*;

	use super::*;

	nostd_entrypoint!(process_instruction);

	#[inline(always)]
	pub fn process_instruction(
		program_id: &Address,
		accounts: &[AccountView],
		data: &[u8],
	) -> ProgramResult {
		let instruction: HelloInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			HelloInstruction::Hello => HelloAccounts::try_from(accounts)?.process(data),
		}
	}
}
```

`nostd_entrypoint!` wires up the BPF entrypoint, a minimal panic handler, and a no-allocation stub. `parse_instruction` reads the discriminator byte, verifies the program ID matches, and returns the typed enum variant.

## The complete program

<br>

Putting it all together (this matches `examples/hello_solana/src/lib.rs` in the repository):

```rust
#![allow(clippy::inline_always)]
#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("DCF5KBmtQ9ryDC7mQezKLwuJHem6coVUCmKkw37M9J4A");

#[discriminator]
pub enum HelloInstruction {
	Hello = 0,
}

#[instruction(discriminator = HelloInstruction, variant = Hello)]
pub struct HelloInstructionData {}

#[derive(Accounts, Debug)]
pub struct HelloAccounts<'a> {
	pub user: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for HelloAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let _ = HelloInstructionData::try_from_bytes(data)?;
		self.user.assert_signer()?;
		log!("Hello, Solana!");
		Ok(())
	}
}

#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use pina::*;

	use super::*;

	nostd_entrypoint!(process_instruction);

	#[inline(always)]
	pub fn process_instruction(
		program_id: &Address,
		accounts: &[AccountView],
		data: &[u8],
	) -> ProgramResult {
		let instruction: HelloInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			HelloInstruction::Hello => HelloAccounts::try_from(accounts)?.process(data),
		}
	}
}
```

## Building for SBF

<br>

To compile the program for the Solana BPF target:

```bash
cargo build --release --target bpfel-unknown-none -p hello_solana -Z build-std -F bpf-entrypoint
```

The workspace `.cargo/config.toml` already sets the required linker flags for `bpfel-unknown-none`. The `-Z build-std` flag rebuilds `core` and `alloc` for the BPF target.

## Writing tests

<br>

Tests run against the native Rust library (without `bpf-entrypoint`). You can verify discriminator values, instruction serialization, and program ID validity without needing a full Solana validator:

```rust
#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn discriminator_hello_value() {
		assert_eq!(HelloInstruction::Hello as u8, 0);
	}

	#[test]
	fn discriminator_roundtrip() {
		let parsed = HelloInstruction::try_from(0u8);
		assert!(parsed.is_ok());
	}

	#[test]
	fn discriminator_invalid_byte_fails() {
		let result = HelloInstruction::try_from(99u8);
		assert!(result.is_err());
	}

	#[test]
	fn instruction_data_has_discriminator() {
		assert!(HelloInstructionData::matches_discriminator(&[0u8]));
		assert!(!HelloInstructionData::matches_discriminator(&[1u8]));
	}

	#[test]
	fn program_id_is_valid() {
		assert_ne!(ID, Address::default());
	}
}
```

For full integration tests that simulate the Solana runtime, add `mollusk-svm` as a dev-dependency and use its transaction builder to invoke your program's `process_instruction` function.

## Next steps

<br>

- Add on-chain state with `#[account]` -- see the `counter_program` example.
- Handle multiple instructions by adding more variants to your discriminator enum.
- Add PDA-based accounts with `create_program_account_with_bump`.
- Follow the [Token Escrow Tutorial](./token-escrow.md) for a real-world program with token transfers and CPI.
