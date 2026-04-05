# Core Concepts

## Discriminator layout (raw bytes)

Pina stores discriminator bytes directly in the struct itself as the first field of every `#[account]`, `#[instruction]`, and `#[event]` type. This is a **discriminator-first** layout, not an external header.

At runtime this means the parser does a fixed-byte read + `size_of::<T>()` validation, then a zero-copy cast.

```text
offset | size | meaning
------ | ---- | -------
0      | N    | discriminator (N = BYTES of enum primitive: 1/2/4/8)
N      | ...  | payload fields
```

This contract is what enables:

- deterministic `size_of::<T>()` checks,
- zero-copy validation with `as_account()` / `try_from_bytes()`,
- alignment-safe offsets for fixed-size Pod fields.

### Why this is safer than implicit external headers

External fixed-size headers require manual casting logic in each parse path and make compiler-assist checks harder. With auto-injected first-field discriminators, the compiler can guarantee the exact struct layout and validate it in type-checked assertions.

## Discriminator width and compatibility

The enum primitive width controls both on-chain layout and migration surface.

- Width is set on the discriminator enum using `#[discriminator(primitive = u8)]` (default `u8`).
- Allowed widths are `u8`, `u16`, `u32`, and `u64`.
- The maximum practical width is capped at 8 bytes for zero-copy safety.

<!-- {=pinaDiscriminatorVersionCompatibility} -->

## Discriminator and payload versioning

| Change | Compatibility impact |
| --- | --- |
| Add a new enum variant | Usually backward-compatible if old clients ignore unknown variants |
| Change an existing variant value | **Breaking** for every historical byte slice |
| Reorder or remove struct fields | **Breaking** (offsets change) |
| Append fields to a struct | Mostly non-breaking, but consumers must accept the larger size |
| Switch primitive width (`u8` → `u16`, etc.) | **Breaking** for serialized payloads at that boundary |

For on-chain accounts, treat layout as part of protocol ABI:

- Keep field order stable.
- Introduce optional `version` fields at the tail for in-place migration strategies.
- Never change existing discriminator values in place.
- When incompatible layout changes are required, perform explicit migration with a new account version and an operator upgrade flow.

For instruction payloads:

- Prefer additive migration: add a new variant and keep legacy handlers for a release cycle.
- Reject stale payload shapes with explicit errors rather than silently reinterpreting bytes.

<!-- {/pinaDiscriminatorVersionCompatibility} -->

<!-- {=pinaDiscriminatorLayoutDecisionMatrix} -->

## Discriminator layout decision matrix

The discriminator strategy determines byte layout, parser guarantees, and cross-protocol compatibility.

| Goal | Recommended layout |
| --- | --- |
| Keep layout **minimal and zero-copy** while staying explicit | **Current Pina model**: discriminator bytes are the first field inside `#[account]`, `#[instruction]`, and `#[event]` structs. |
| Preserve compatibility with existing Anchor-account payloads (SHA-256 hash prefixes) | **Legacy adapter model**: custom raw wrapper types parse/write the existing 8-byte external prefix before converting to typed structs. |
| Minimize account size growth when you have many types | **Use `u8`** (default) discriminator width. |
| You need more than 256 route variants | **Use `u16` / `u32` / `u64`** by setting `#[discriminator(primitive = ...)]`. |
| Avoid schema migrations across existing serialized data | Keep existing field order and discriminator values; only append fields. |

### Raw discriminator width by use-case

| Width | Max variants | Storage cost (bytes) | Recommended when |
| --- | --- | --- | --- |
| `u8` | 256 | 1 | Most programs and instructions |
| `u16` | 65,536 | 2 | Medium-large routing tables and explicit version partitioning |
| `u32` | 4,294,967,296 | 4 | Very large enums, rarely needed |
| `u64` | 18,446,744,073,709,551,616 | 8 | Legacy interoperability shims or reserved growth |

- Discriminator width only affects the first field bytes.
- Widths above 8 are rejected at macro expansion time.
- Wider discriminators improve variant space, but increase CPI payload and account rent by the exact number of bytes.

<!-- {/pinaDiscriminatorLayoutDecisionMatrix} -->

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
