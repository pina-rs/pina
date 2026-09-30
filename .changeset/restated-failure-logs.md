---
pina: fix
# The guides and compute-unit policy this change edits belong to `pina_root`,
# which is unpublished, so the coverage is recorded without a bump.
pina_root: none
---

# Stop logging failure messages that restate the error

Five of pina's account validations logged a fixed message that only restated the error they return: a missing signature (`MissingRequiredSignature`), a wrong owner (`InvalidAccountOwner`), an account that is not empty (`AccountAlreadyInitialized`) or is empty (`UninitializedAccount`), and a fixed account of the wrong size (`InvalidAccountSize`). The runtime already reports each of those codes, and pina returns each of them for that one reason, so the message told a client nothing new. It still cost deployed size at every inlined validation site, because a logging branch cannot be merged with the other failures that return the same code.

Default builds no longer log these messages. With `verbose-logs`, they are logged as before, together with the address detail and caller location. Messages that tell apart the causes of a shared code are unchanged and still logged by default: the several reasons for `InvalidAccountData`, the causes of `InvalidSeeds`, and `account has an invalid discriminator`, which distinguishes an account discriminator from an instruction discriminator returning the same `InvalidDiscriminator`. Error values and error order are unchanged.

Measured with the framework-comparison profile on top of the inline entrypoint error conversion: the hello fixture fell from 2,088 to 1,984 bytes and the counter fixture from 9,304 to 8,712. Among the examples, `multisig_program` fell by 2,968 bytes, `staking_rewards_program` by 1,632, `escrow_program` by 1,000, `counter_program` by 680, and `hello_solana_program` by 104.
