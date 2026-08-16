---
pina_profile: fix
---

Replace `.expect()`/`.unwrap()` calls in pina_profile tests with `unwrap_or_else(|e| panic!("...", e))` for explicit, descriptive panic messages per repo conventions.
