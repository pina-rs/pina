---
pina_macros: feat
pina_cli: fix
---

Support `discriminator = Enum::Variant` in the `#[instruction]`, `#[account]`, and `#[event]` attribute macros, replacing the separate `variant = Variant` argument.

```rust
// Before
#[instruction(discriminator = VestingInstruction, variant = Initialize)]
pub struct InitializeInstruction {
	// ...
}

// After
#[instruction(discriminator = VestingInstruction::Initialize)]
pub struct InitializeInstruction {
	// ...
}
```

The shorthand form is unchanged: when the struct name matches the variant, `discriminator = Enum` alone still works.

```rust
#[instruction(discriminator = VestingInstruction)]
pub struct Initialize {
	// ...
}
```

## Migration guide

- Replace `#[instruction(discriminator = Enum, variant = Variant)]` with `#[instruction(discriminator = Enum::Variant)]`. The same applies to `#[account(...)]` and `#[event(...)]`.
- The old `variant = Variant` argument remains supported for backwards compatibility. When it is present, the complete `discriminator` value is treated as the enum path, which preserves qualified forms such as `crate::types::Enum, variant = Variant`.
- `pina_cli` IDL extraction understands all three forms: `Enum::Variant`, `Enum` + `variant = Variant`, and bare `Enum` (variant defaults to the struct name). The `pina init` template now emits the new syntax.
