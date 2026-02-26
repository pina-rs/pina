# Migrating from Anchor

<br>

This guide maps common Anchor patterns to their Pina equivalents. If you have an existing Anchor program and want to rewrite it with Pina for lower compute usage and smaller binaries, this is the reference to follow.

The repository includes several `anchor_*` example programs that demonstrate direct parity with Anchor's own test suite. These are referenced throughout this guide.

## Program structure

<br>

### Anchor

```rust
use anchor_lang::prelude::*;

declare_id!("Fg6PaFpoGXk...");

#[program]
pub mod my_program {
	use super::*;

	pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
		// ...
		Ok(())
	}
}

#[derive(Accounts)]
pub struct Initialize<'info> {
	#[account(mut)]
	pub user: Signer<'info>,
	#[account(init, payer = user, space = 8 + MyAccount::INIT_SPACE)]
	pub my_account: Account<'info, MyAccount>,
	pub system_program: Program<'info, System>,
}
```

### Pina

```rust
use pina::*;

declare_id!("Fg6PaFpoGXk...");

#[discriminator]
pub enum MyInstruction {
	Initialize = 0,
}

#[instruction(discriminator = MyInstruction, variant = Initialize)]
pub struct InitializeInstruction {}

#[derive(Accounts, Debug)]
pub struct InitializeAccounts<'a> {
	pub user: &'a AccountView,
	pub my_account: &'a AccountView,
	pub system_program: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let _ = InitializeInstruction::try_from_bytes(data)?;
		self.user.assert_signer()?.assert_writable()?;
		self.my_account.assert_empty()?.assert_writable()?;
		self.system_program.assert_address(&system::ID)?;
		// ...
		Ok(())
	}
}
```

Key differences:

- **No `#[program]` module.** Pina uses explicit discriminator enums and a manual `match` in the entrypoint.
- **No `Context<T>`.** Each accounts struct receives `&[AccountView]` and the processor receives raw `data: &[u8]`.
- **Constraints are code, not attributes.** Validation happens inside `process` via chained assertions rather than `#[account(...)]` attribute directives.

## Account constraints to validation chains

<br>

Anchor expresses constraints as attributes on account fields. Pina uses explicit method calls on `AccountView` references.

| Anchor attribute                  | Pina equivalent                                                        |
| --------------------------------- | ---------------------------------------------------------------------- |
| `Signer<'info>`                   | `account.assert_signer()?`                                             |
| `#[account(mut)]`                 | `account.assert_writable()?`                                           |
| `#[account(owner = program)]`     | `account.assert_owner(&program_id)?`                                   |
| `#[account(address = KEY)]`       | `account.assert_address(&KEY)?`                                        |
| `#[account(seeds = [...], bump)]` | `account.assert_seeds_with_bump(seeds, &ID)?`                          |
| `#[account(init, ...)]`           | `account.assert_empty()?` then `create_program_account_with_bump(...)` |
| `#[account(constraint = expr)]`   | Write the check directly in `process` and return an error              |
| `Account<'info, T>` (type check)  | `account.assert_type::<T>(&owner)?`                                    |

Pina's assertion methods return `Result<&AccountView>`, so they chain naturally:

```rust
self.counter
	.assert_not_empty()?
	.assert_writable()?
	.assert_type::<CounterState>(&ID)?;
```

See `examples/counter_program` for a complete PDA creation and validation example, and `examples/anchor_duplicate_mutable_accounts` for explicit duplicate-account safety checks.

## Account data: Borsh to Pod

<br>

### Anchor (Borsh)

```rust
#[account]
pub struct MyAccount {
	pub authority: Pubkey,
	pub value: u64,
	pub active: bool,
}
```

Anchor uses Borsh serialization by default. The `#[account]` macro adds an 8-byte discriminator (SHA-256 hash prefix) and derives `BorshSerialize`/`BorshDeserialize`.

### Pina (Pod / zero-copy)

```rust
#[account(discriminator = MyAccountType)]
pub struct MyAccount {
	pub authority: Address,
	pub value: PodU64,
	pub active: PodBool,
}
```

Pina uses zero-copy (`bytemuck::Pod`) layouts. Every field must be a fixed-size, `Copy` type. This means:

| Anchor type | Pina type       | Notes                                    |
| ----------- | --------------- | ---------------------------------------- |
| `Pubkey`    | `Address`       | Both are `[u8; 32]`                      |
| `u64`       | `PodU64`        | Little-endian, alignment-safe            |
| `u32`       | `PodU32`        | Little-endian, alignment-safe            |
| `u16`       | `PodU16`        | Little-endian, alignment-safe            |
| `i64`       | `PodI64`        | Little-endian, alignment-safe            |
| `bool`      | `PodBool`       | Single byte                              |
| `String`    | `[u8; N]`       | Fixed-size byte arrays only              |
| `Vec<T>`    | Not supported   | Use fixed-size arrays                    |
| `Option<T>` | Manual encoding | Use a sentinel value or a `PodBool` flag |

Pod wrappers are needed because `#[repr(C)]` structs require all fields to have alignment 1 for bytemuck compatibility. Converting to and from native types:

```rust
// Creating Pod values
let value = PodU64::from_primitive(42);
let active = PodBool::from(true);

// Reading Pod values
let n: u64 = value.into();
let b: bool = active.into();
```

The `#[account]` macro's discriminator is a single `u8` (or configurable width) rather than Anchor's 8-byte hash. This saves 7 bytes per account.

## Discriminators

<br>

### Anchor

Anchor generates 8-byte discriminators from `sha256("account:<StructName>")` or `sha256("global:<method_name>")`. These are implicit -- you never write them manually.

### Pina

Pina uses explicit discriminator enums with numeric values:

```rust
#[discriminator]
pub enum MyInstruction {
	Initialize = 0,
	Update = 1,
}

#[discriminator]
pub enum MyAccountType {
	MyAccount = 1,
}
```

Each `#[instruction]` or `#[account]` macro references its discriminator enum and variant:

```rust
#[instruction(discriminator = MyInstruction, variant = Initialize)]
pub struct InitializeInstruction {
	// ...
}

#[account(discriminator = MyAccountType)]
pub struct MyAccount {
	// ...
}
```

Benefits of explicit discriminators:

- Stable, human-readable values (not hash-dependent).
- Single byte by default (configurable to u16/u32/u64), saving space.
- No hidden behavior -- you control the exact values.

## Errors

<br>

### Anchor

```rust
#[error_code]
pub enum MyError {
	#[msg("Value is too large")]
	ValueTooLarge,
}
```

Anchor assigns error codes starting at 6000 and provides `#[msg]` for error messages.

### Pina

```rust
#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MyError {
	ValueTooLarge = 6000,
}
```

Pina's `#[error]` macro generates `From<MyError> for ProgramError` using `ProgramError::Custom(code)`. You choose the numeric code explicitly. To return an error:

```rust
return Err(MyError::ValueTooLarge.into());
```

See `examples/anchor_errors` for a complete parity port of Anchor's error handling, including guard helpers like `require_eq` and `require_gt`.

## Events

<br>

### Anchor

```rust
#[event]
pub struct MyEvent {
	pub data: u64,
	pub label: String,
}

emit!(MyEvent {
	data: 5,
	label: "hello".into()
});
```

### Pina

```rust
#[discriminator]
pub enum EventDiscriminator {
	MyEvent = 1,
}

#[event(discriminator = EventDiscriminator)]
#[derive(Debug)]
pub struct MyEvent {
	pub data: PodU64,
	pub label: [u8; 8],
}
```

Pina events are `Pod` structs with explicit discriminators, just like accounts and instructions. They do not have a built-in `emit!` macro -- event emission is handled by writing bytes to the transaction log or via CPI patterns. The `#[event]` macro gives you `HasDiscriminator`, `Pod`, `Zeroable`, and `TypedBuilder`.

See `examples/anchor_events` for the full parity port.

## CPI (Cross-Program Invocation)

<br>

### Anchor

```rust
let cpi_accounts = Transfer {
	from: ctx.accounts.from.to_account_info(),
	to: ctx.accounts.to.to_account_info(),
	authority: ctx.accounts.authority.to_account_info(),
};
let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
token::transfer(cpi_ctx, amount)?;
```

### Pina

```rust
token_2022::instructions::TransferChecked {
	from: self.from,
	to: self.to,
	authority: self.authority,
	amount,
	mint: self.mint,
	decimals,
	token_program: self.token_program.address(),
}
.invoke()?;
```

Pina's CPI helpers (enabled with `features = ["token"]`) are typed instruction builders. Fill in the struct and call `.invoke()` or `.invoke_signed(&signers)` for PDA-authorized calls. No `CpiContext` wrapper is needed.

See `examples/escrow_program` for CPI usage with both token transfers and ATA creation.

## Account creation

<br>

### Anchor

```rust
#[account(init, payer = user, space = 8 + 32 + 8)]
pub my_account: Account<'info, MyData>,
```

### Pina

```rust
// For PDA accounts:
create_program_account_with_bump::<MyData>(
	self.my_account,
	self.payer,
	&ID,
	seeds,
	bump,
)?;

// For regular accounts:
create_account(
	self.payer,
	self.my_account,
	size_of::<MyData>(),
	&ID,
)?;
```

Space is automatically computed from `size_of::<MyData>()` for the PDA helper. For `create_account` you pass the size explicitly. In both cases, rent-exemption lamports are calculated and transferred automatically.

## no_std and the entrypoint

<br>

Anchor programs use `#[program]` which generates the entrypoint. Pina programs are `#![no_std]` and use a feature-gated entrypoint module:

```rust
#![no_std]

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
		let instruction: MyInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			MyInstruction::Initialize => InitializeAccounts::try_from(accounts)?.process(data),
		}
	}
}
```

The feature gate means tests compile without BPF entrypoint overhead. The `nostd_entrypoint!` macro wires up the BPF program entrypoint, a minimal panic handler, and a no-allocation stub.

## Testing

<br>

### Anchor

Anchor programs are typically tested with TypeScript/Mocha tests that run against a local validator via `anchor test`.

### Pina

Pina programs are tested as regular Rust libraries:

```rust
#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn discriminator_roundtrip() {
		assert!(MyInstruction::try_from(0u8).is_ok());
		assert!(MyInstruction::try_from(99u8).is_err());
	}
}
```

For integration tests, use `mollusk-svm` (a Solana SVM simulator) instead of a full validator:

```toml
[dev-dependencies]
mollusk-svm = { workspace = true }
```

This gives you fast, deterministic tests without network I/O.

## Migration checklist

<br>

1. Replace `anchor_lang::prelude::*` with `use pina::*`.
2. Convert `#[account]` structs from Borsh to Pod types (`PodU64`, `PodBool`, `Address`, fixed-size arrays).
3. Define explicit `#[discriminator]` enums for instructions and accounts.
4. Replace `#[account(...)]` constraint attributes with validation chain calls in `process`.
5. Replace `Context<T>` with `#[derive(Accounts)]` structs and `ProcessAccountInfos`.
6. Replace `CpiContext` patterns with Pina's typed CPI instruction builders.
7. Replace `#[error_code]` with `#[error]` and explicit numeric codes.
8. Replace `#[event]` + `emit!` with Pina's Pod-based event structs.
9. Add `#![no_std]` and the `bpf-entrypoint` feature gate.
10. Port TypeScript tests to Rust using `mollusk-svm` or native unit tests.
