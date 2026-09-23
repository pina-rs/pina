---
pina_macros: fix
---

# Qualify every `Address` in generated PDA code

`#[pda]` emitted its generated `Address` types unqualified: the `program_id` parameter of `try_find_pda`, `find_pda`, and `assert_seeds`, and the seed parameter and stored field types of the generated seeds struct. Those spellings resolve against the user's own scope, so a program with its own type named `Address` could silently change which address type a generated signature takes. Every generated `Address` is now the crate-qualified form, matching what the surrounding generated code and the `#[account]` schema path already emitted.

The expansion also emits the same identity proof the schema path uses — `const _: fn(Address) -> pina::Address = |value| value;` — but only when a declaration actually has an `Address` seed, so a numeric-only `#[pda]` never requires `Address` in scope. The proof binds the caller's bare `Address` spelling to the crate's type, so a future unqualified spelling is a compile error in the user's crate rather than a silent shadow.
