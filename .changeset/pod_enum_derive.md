---
pina_macros: major
---

# Use zeropod's native enum schema support

Remove Pina's custom `PodEnum` derive. Unit enums used in account, instruction, or event schemas now derive `zeropod::ZeroPod` together with their containing schema. Application fields use the native enum type; zeropod generates and validates the alignment-one `EnumZc` storage companion.

```rust
use pina::ZeroPod;

#[derive(ZeroPod)]
#[repr(u8)]
enum Color {
	Red = 0,
	Green = 1,
	Blue = 2,
}

#[account(discriminator = MyAccount)]
struct Palette {
	color: Color,
	brightness: u64,
}
```

This keeps native domain types in application schemas while ensuring untrusted bytes are validated before the generated zero-copy view is returned.
