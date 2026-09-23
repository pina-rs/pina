---
pina_macros: fix
---

# Reject PDA declarations that cannot derive

Every generated `#[pda]` derivation appends a bump seed: `try_find_pda` and `find_pda` search for the canonical one, `with_bump` supplies the stored or explicit one, and the generated loaders derive from the stored field. The runtime caps a derivation at 16 seeds total, so a declaration of 16 seeds produced 17 and no instruction could ever load the account.

The macro now caps the declared list at 15 seeds and names both numbers in the error. The cap applies whether or not the declaration has a `bump =` field, because the bump-free variant derives through `try_find_pda` and appends a bump there. A declaration that previously compiled with 16 seeds was already unreachable at runtime; it now fails the build instead of shipping a PDA that always returns `InvalidSeeds`.
