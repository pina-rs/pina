---
pina: patch
---

Fix three critical bugs in the `pina` core crate:

- **`assert_seeds()` inverted logic** — previously returned `Ok` when the account key did _not_ match the derived PDA. Now correctly returns `Ok` when the key matches, consistent with `assert_seeds_with_bump` and `assert_canonical_bump`.
- **`send()` never wrote back lamports** — the `checked_sub` / `checked_add` results were computed but discarded, so lamport balances never actually changed. Now assigns the results back via `*self_lamports = ...` and `*recipient_lamports = ...`.
- **Typo fix** — corrected "recipent" to "recipient" in the `send()` error log message.
