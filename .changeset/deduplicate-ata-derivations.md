---
pina: none
---

# Drop duplicated ATA derivations from hot paths

The canonical associated-token-address search costs roughly 1,500 CU per call on Solana because every attempt re-hashes the seeds and re-runs the point-decompression check, and the total scales with how far the address's canonical bump sits below 255. Two handlers paid that search twice for the same account in one instruction:

- `vesting_program`'s `Claim` asserted the vault was the schedule's associated token account in the validation chain and then derived the identical `(wallet, mint, token program)` tuple again when it read the vault balance. The chain assert is gone; the balance load (`as_associated_token_account`) calls the exact same address validation and still rejects a caller-supplied account, which is the property that stops the schedule's signature from draining a foreign vault. The check runs before any state mutation or CPI.
- `vesting_program`'s `Cancel` had the same assert-then-load pair for the refund vault. The chain assert is gone and the refund-balance load is the vault's only derivation, again ahead of the `cancelled` write and every CPI.

Measured on the Surfpool simulation maximum (the same statistic the performance policy enforces): `vesting_program/claim` 22,202 to 20,642 (-1,560, -7.0%) and `vesting_program/cancel` 29,281 to 27,722 (-1,559, -5.3%). Failure attribution is preserved: the surviving load calls the identical `assert_associated_token_address` the removed chain step called, so a substituted vault still fails with `InvalidSeeds`.

`escrow_program`'s `Take` was audited for the same shape and deliberately left alone: its `taker_ata_b` derivation is the account's only canonical pin (the handler's `CreateIdempotent` CPI derives the maker's ATA, and the transfer CPI only proves signer authority), so removing it would let a taker pay from any mint-B account they control. Two adversarial Surfpool cases added here prove both sides: a foreign vault is rejected by both vesting handlers with no state change, and `Take` rejects both a foreign and a funded non-canonical taker-controlled token-B account.
