---
pina: fix
---

# Shrink PDA-creation seed and signer arrays

The PDA-creation CPI spine built three fixed-capacity stack arrays for every account it created: 16 derivation seeds, 16 signer seeds, and 16 signers. Only the first `seeds.len() + 1` seeds and `signers.len() + 1` signers are ever read, but safe Rust must initialize every slot, and because each array escapes to a syscall LLVM cannot remove the stores to the unread slots. `combine_seeds_with_bump` also returned its 16-seed array by value, adding a 256-byte copy.

The spine now picks the smallest seed capacity — 4, 8, or 16 — that holds the seeds plus the bump. Every `#[pda]`-generated seed array has a compile-time length, so only one capacity survives inlining. When the builder receives no extra signers, the target's signer is passed on its own and the 16-signer array is never built. `CompactCreationTarget::allocate_zeroed`, which is deliberately outlined and shared by compact accounts with different seed counts, keeps one full-capacity copy rather than three.

Validation, error values, and error order are unchanged: the signer-count and seed-count checks run first on every entry, the derived address is compared before any CPI, and the same system-program instructions are issued with the same signer seeds. `combine_seeds_with_bump` keeps its public signature.

Measured with the framework-comparison profile and verifier, the counter fixture fell from 11,400 to 10,456 bytes and its `initialize` from 3,203 to 3,079 compute units. Among the examples, `multisig_program` fell by 1,608 bytes, `staking_rewards_program` by 976, `counter_program` by 864, `vesting_program` by 808, and `escrow_program` by 776.
