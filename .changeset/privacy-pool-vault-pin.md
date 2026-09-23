---
pina: none
---

# Pin the privacy pool vault

`privacy_pool_program` transferred deposits to, and paid withdrawals from, whatever account the caller passed as `pool_vault` without verifying it. The vault is data-less by design, so no handler parsed it and no check existed: a depositor could name an account they control, keep their lamports, and still have a spendable commitment recorded in the merkle tree — minting notes backed by nothing and draining the honest pool as those notes were withdrawn.

Both `Deposit` and `Withdraw` now assert the vault is the pool's derived PDA before any value moves. An adversarial Surfpool case proves it: a substituted vault fails against the unpatched program (the deposit succeeds and the note exists) and is rejected with the check. The recorded ABI and the generated clients follow: the vault account now carries a `pda` binding in the manifest and IDL, and the generated `Deposit`/`Withdraw` constructors derive it instead of accepting it, so the client API cannot express the substitution.
