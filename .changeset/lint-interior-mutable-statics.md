---
pina_lints: fix
---

# reject interior-mutable statics as trusted provenance

The `require_program_check_before_cpi` lint no longer treats an immutable static whose type contains interior mutability as a compile-time-trusted expected ID. The static's type must be `Freeze` — no `UnsafeCell` anywhere in the type — so `static TARGET: Mutex<Pubkey>`, a raw `UnsafeCell` wrapper, or a struct with an interior-mutable field falls through to the narrow allowance path instead of authenticating a dynamic CPI target. Consts, assoc consts, and immutable `Freeze` statics (for example plain `Address` or `[u8; 32]` values) remain accepted provenance. Code that deliberately passes a runtime-replaceable expected ID must use a narrowly scoped lint allowance with documented authentication.
