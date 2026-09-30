---
pina: fix
# The devenv task, guides, and expansion snapshots this change edits belong to
# `pina_root`, which is unpublished, so the coverage is recorded without a bump.
pina_root: none
---

# Convert entrypoint errors inline

`nostd_entrypoint!` and `nostd_entrypoint_alloc!` now declare pina's own `entrypoint` instead of expanding `pinocchio::program_entrypoint!`. Accounts are still deserialized by `pinocchio::entrypoint::deserialize`, with the same account array and the same safety contract; the only difference is how the result becomes the runtime's `u64` status. Pinocchio converts the `ProgramError` in an `#[inline(never)]` function, an 864-byte comparison tree that every program carries even when each error it returns is a constant. Pina converts it inline, so an error returned by value folds to its status code where it is returned, and the tree is only emitted for errors the compiler cannot see through, such as those from outlined helpers and CPIs.

Status codes are unchanged: a new test runs the entrypoint over loader-format input for every `ProgramError` variant and compares each status with `u64::from`, and further tests cover skipped accounts past the array and duplicate slots. Those tests also run under Miri in `test:miri`.

Measured with the framework-comparison profile on top of the entrypoint account capacity: the hello fixture fell from 2,944 to 2,088 bytes and the counter fixture from 9,416 to 9,304, with no compute-unit increase. Among the examples, `escrow_program` fell by 1,048 bytes, `staking_rewards_program` by 432, `counter_program` by 368, and `multisig_program` by 48.
