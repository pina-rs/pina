# Workspace Architecture

## Main crates

- `crates/pina` — core framework: traits, account loaders, CPI helpers, Pod types, `nostd_entrypoint!`
- `crates/pina_macros` — proc macros such as `#[account]`, `#[instruction]`, `#[event]`, `#[error]`, `#[discriminator]`, `#[derive(Accounts)]`
- `crates/pina_sdk_ids` — typed Solana program and sysvar IDs
- `crates/pina_cli` — CLI/library for IDL generation and Codama workflows
- `crates/pina_codama_renderer` — repository-local Codama Rust renderer
- `crates/pina_pod_primitives` — alignment-safe POD primitives
- `crates/pina_profile` — static CU profiler for compiled SBF programs

There are also multiple examples and security fixtures under `examples/` and `security/`.

## Core patterns

### Entrypoint pattern

```rust
nostd_entrypoint!(process_instruction);
fn process_instruction(
	program_id: &Address,
	accounts: &mut [AccountView],
	data: &[u8],
) -> ProgramResult {
	let instruction: MyInstruction = parse_instruction(program_id, &ID, data)?;
	match instruction {
		MyInstruction::Action => MyAccounts::try_from(accounts)?.process(data),
	}
}
```

### Discriminator system

- Every account, instruction, and event type has a discriminator as its first field.
- `#[discriminator]` generates conversions plus `Pod`/`Zeroable` implementations.
- `#[account]`, `#[instruction]`, and `#[event]` inject discriminator fields and generate validation-related implementations.

### Account validation

Prefer chained validation on `AccountView` references:

```rust
account.assert_signer()?.assert_writable()?.assert_owner(&program_id)?
```

These methods return the same reference type they receive, so mutable validation chains stay mutable.

### Pod types

Common alignment-safe wrapper types include:

- `PodBool`
- `PodU16`
- `PodU32`
- `PodU64`
- `PodU128`
- `PodI16`
- `PodI64`

Use them in `#[repr(C)]` account structs with bytemuck-compatible layouts.
