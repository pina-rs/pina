---
pina: none
---

# Drop duplicated ATA derivations from hot paths

The canonical associated-token-address search costs roughly 1,500 CU per call on Solana because every attempt re-hashes the seeds and re-runs the point-decompression check, and the total scales with how far the address's canonical bump sits below 255. Three handlers paid that search twice for the same account in one instruction:

- `vesting_program`'s `Claim` asserted the vault was the schedule's associated token account in the validation chain and then derived the identical `(wallet, mint, token program)` tuple again when it read the vault balance. The chain assert is gone; the balance load (`as_associated_token_account`) derives the address exactly once and still rejects a caller-supplied account, which is the property that stops the schedule's signature from draining a foreign vault.
- `vesting_program`'s `Cancel` had the same assert-then-load pair for the refund vault. The chain assert is gone and the refund-balance load is the vault's only derivation.
- `escrow_program`'s `Take` derived `taker_ata_b` explicitly and then invoked `CreateIdempotent`, whose own seed derivation rejects a mismatched account before its idempotent branch. The explicit derivation is gone; the `assert_writable` check and the CPI's address check remain, matching the convention the same handler already documented for `maker_ata_b`.

Measured on the Surfpool simulation maximum (the same statistic the performance policy enforces): `vesting_program/claim` 22,202 to 20,642 (-1,560, -7.0%), `vesting_program/cancel` 29,281 to 27,722 (-1,559, -5.3%), and `escrow_program/take` 32,351 to 30,695 (-1,656, -5.1%). `escrow_program/make` and `vesting_program/initialize` are byte-for-byte unchanged. Failure attribution is preserved: every rejection these paths previously reported at the removed asserts is now reported by the equivalent derivation at the load or CPI, with the same `InvalidSeeds` error.
