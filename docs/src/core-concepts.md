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

Traits in `crates/pina/src/loaders.rs` provide typed conversion paths from raw `AccountView` values into strongly typed account states.

## Entrypoint model

`nostd_entrypoint!` wires BPF entrypoint plumbing while preserving `no_std` constraints for on-chain builds.
