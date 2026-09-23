---
pina: feat
pina_abi: feat
pina_macros: feat
pina_cli: feat
---

# Accept const capacities in schemas

Compact account and instruction schemas now accept a `const` item wherever they previously required an integer literal:

```rust
const MAX_MEMBERS: usize = 24;

#[account(discriminator = MultisigAccountType, compact)]
pub struct Multisig {
	pub bump: u8,
	pub member_keys: Vec<Address, MAX_MEMBERS>,
}
```

`Vec<T, N>`, `PodVec<T, N, PFX>`, `String<N>`, `PodString<N, PFX>`, an `Option<...>` around them, and fixed `[T; N]` arrays and instruction arguments all resolve a capacity through a constant. Constants may be arithmetic over other constants and may be declared in any module of the crate, so one bound is declared once and reused in the account, the instruction that writes it, and the helper that sizes it.

Pina resolves each capacity during expansion and records the number it evaluates to. The ABI layer still receives concrete values, so replacing a literal with a constant of the same value leaves `migrations/manifest.json`, the generated `tests/abi_layout.rs` assertions, `MAX_SIZE`/`MIN_SIZE`/`HEADER_SIZE`, and `projected_bytes(...)` byte-identical, and `pina migrations check` and the Codama IDL unchanged.

A capacity that cannot be evaluated at expansion time now fails the build with a diagnostic naming the expression and pointing at the workaround, instead of the previous `unsupported compact field` or `arrays require an integer literal length` message. An associated constant such as `Bounds::MAX_MEMBERS` is not resolved; declare the bound as a `const` item.

`pina_abi` gains `SchemaConsts`, the shared evaluator both the macros and the CLI use so the two layers agree on what a constant means. `pina_cli` resolves capacities while it parses a program, so migration and IDL tooling reads the same numbers the compiler does.
