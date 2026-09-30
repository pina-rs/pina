---
pina: fix
---

# Check PDA creation targets with sha256

The PDA creation builders checked the target account's address with the `sol_create_program_address` syscall before the create-account CPI, about 1,500 compute units per creation. They now hash the seeds, bump, owner, and PDA marker with `sha256` and compare, which took the counter comparison fixture's `initialize` from 3,073 to 1,713 compute units.

The hash skips only the ed25519 curve check, and the runtime repeats it: the allocation signs for the target through `invoke_signed` with the same seeds and bump, and the runtime refuses to sign for an on-curve address. `counter_program`'s Surfpool suite now proves it with a bump whose derived address is on the curve, which fails with "Could not create program address with signer seeds" and creates nothing. A mismatched address still fails the builder's own check with `InvalidSeeds`.

`pina` now depends on `solana-sha256-hasher` directly for the variable-length hash, with the same target-specific features `solana-address` already enables.
