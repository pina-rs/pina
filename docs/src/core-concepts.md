# Core Concepts

## Discriminators

Pina uses discriminator enums as first-field tags for instruction/account/event types. This gives stable type identification at runtime and enables explicit parsing/dispatch.

## Zero-copy account models

`#[account]` and `#[instruction]` generate `Pod`/`Zeroable`-compatible layouts for in-place reinterpretation of account/instruction bytes.

## Account validation chains

Validation methods on `AccountView` are composable:

```rust
account.assert_signer()?.assert_writable()?.assert_owner(&program_id)?;
```

This pattern improves readability while keeping checks explicit and audit-able.

## Typed account conversions

Traits in `crates/pina/src/impls.rs` provide typed conversion paths from raw `AccountView` values into strongly typed account states.

## Entrypoint model

`nostd_entrypoint!` wires BPF entrypoint plumbing while preserving `no_std` constraints for on-chain builds.

## Pod types

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

All types are `#[repr(transparent)]` over byte arrays (or `u8` for `PodBool`) and implement `bytemuck::Pod` + `bytemuck::Zeroable`.

<!-- {/podTypesTable} -->

<!-- {=podArithmeticDescription} -->

Arithmetic operators (`+`, `-`, `*`) use **wrapping** semantics in release builds for CU efficiency and **panic on overflow** in debug builds. Use `checked_add`, `checked_sub`, `checked_mul`, `checked_div` where overflow must be detected in all build profiles.

Each Pod integer type provides `ZERO`, `MIN`, and `MAX` constants.

<!-- {/podArithmeticDescription} -->

This means you can write ergonomic code like:

```rust
my_account.count += 1u64;
let fee = balance.checked_mul(3u64).unwrap_or(PodU64::MAX);
```

## Instruction introspection

<!-- {=pinaIntrospectionDescription} -->

The `pina::introspection` module provides helpers for reading the Instructions sysvar at runtime. This enables:

- **Flash loan guards** — verify the current instruction is not being invoked via CPI (`assert_no_cpi`)
- **Transaction inspection** — count instructions (`get_instruction_count`) or find the current index (`get_current_instruction_index`)
- **Sandwich detection** — check whether a specific program appears before or after the current instruction (`has_instruction_before`, `has_instruction_after`)

<!-- {/pinaIntrospectionDescription} -->
