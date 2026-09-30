---
pina: fix
---

# Keep address-check errors visible to the entrypoint

`assert_address` reached the entrypoint through `validate_address`, which LLVM kept out of line because the program and sysvar checks share it. An out-of-line function returns its `ProgramResult` through memory, so neither the caller nor the entrypoint's inline error conversion could see which error it carried. The address comparison and its failure log now live in one out-of-line helper that returns `bool`, and `validate_address` and `assert_address` are always inlined, so every call site returns `InvalidAccountData` as a constant while sharing one copy of the comparison.

Measured across the examples: 14 programs smaller (−8 to −664 bytes, `role_registry_program` −664, `vesting_program` −528, `multisig_program` −448), 12 unchanged, and `privacy_pool_program` +184. The counter comparison fixture fell from 8,712 to 8,632 bytes, 64 under Anchor v2. Behavior is unchanged: the same checks run in the same order, fail with the same error, and log the same message.
