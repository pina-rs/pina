---
pina: feat
pina_macros: feat
---

# Add a dispatch-first entrypoint

`dispatch_entrypoint!(Instruction)` declares the entrypoint for a program routed by `#[discriminator(entrypoint)]`. It reads the instruction data through the pointer the loader passes since SIMD-0321, validates the program ID and discriminator, then walks only the accounts the routed accounts struct reads, instead of deserializing every account first. On the comparison fixtures, the counter measured 8,424 → 7,592 bytes, `initialize` 1,713 → 1,694 compute units, and `increment` 378 → 360; the hello world measured 1,984 → 1,616 bytes and 146 → 136 compute units.

`#[derive(Accounts)]` now declares `ParseAccounts::ACCOUNT_LIMIT`, the most accounts a struct reads, and `ACCOUNT_MINIMUM`, the fewest it accepts, which the router uses to pick each route's walk. Every check the struct and handler make still runs. Account counts now take precedence over per-account checks: an instruction with more accounts than its struct reads fails with `TooManyAccountKeys`, and one with fewer than a fixed-length struct reads fails with `NotEnoughAccountKeys`.

`AccountsCursor::next` and `next_mut` are now always inlined, which the new entrypoint depends on and which also makes most programs on `nostd_entrypoint!` smaller.
