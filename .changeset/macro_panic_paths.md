---
pina_macros: patch
---

Replace latent panic paths in the proc macros with spanned compile errors and explicit internal-error panics. `#[derive(Accounts)]` now reports a clear error instead of panicking if the input shape is ever accepted by `darling` without named fields, and the remaining `unwrap()` calls on provably-safe values carry explanatory messages. Add trybuild negative tests for `#[derive(Accounts)]` on enums and tuple structs.
