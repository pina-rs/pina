---
pina_macros: major
---

# Use zeropod's native enum schema support

Remove Pina's custom `PodEnum` derive in favor of zeropod's standalone native enum schema support. Pina's audited `#[account]`, `#[instruction]`, and `#[event]` schemas reject enum and custom `ZcField` fields because the macro cannot establish their full mapping and validation-order invariants. Standalone enums can still derive `zeropod::ZeroPod` for advanced direct zeropod integrations outside that closed contract.

```rust
use pina::ZeroPod;

#[derive(ZeroPod)]
#[repr(u8)]
enum Color {
	Red = 0,
	Green = 1,
	Blue = 2,
}

let color = Color::from_bytes(&[1]);
```

For macro-generated Pina schemas, store an audited scalar discriminant and convert it to a domain enum only after explicit semantic validation.
