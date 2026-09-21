---
pina: none
---

# Close the remaining audit findings in the example programs

`multisig_program` closes three findings. `MultisigImport` binds its provenance: the legacy account must sit at the PDA its parsed `create_key` derives under the caller-named legacy program, and the holder of that `create_key` must sign the import, so a fabricated account carrying a borrowed roster of keys that never consented no longer imports (the instruction gains the `legacy_create_key` signer account). `ConfigAuthorityExecute` rejects `AddSpendingLimit` actions with a new `SpendingLimitRequiresProposal` error — creating an allowance moves vault funds, which stays behind threshold and timelock even in controlled mode, so the config authority manages configuration rather than custody. `VaultExecute` additionally protects every program-owned writable non-signer account, not just the multisig and the executing proposal, so a vault message can no longer write another proposal or a spending limit through a nested CPI.

`escrow_program` gains a maker `Cancel` (discriminator 3) that refunds the full vault balance and closes both accounts, so a maker is no longer stranded until a taker appears. `prop_amm_program`'s `Update` now accepts the oracle's stored authority in addition to the benchmark's static key, making `RotateAuthority` mean what it says — previously the rotated key could never publish a price. The multisig readme documents the permissionless `ConfigInitialize` race (initialize the config in the deployment transaction or a sniper owns the fee treasury) and the spending-limit rules, and the staking readme documents the same first-initializer shape for pool creation.
