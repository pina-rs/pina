---
pina: none
---

# Bind privacy pool initialization to a bootstrap authority

The pool's accounts are all singleton PDAs over fixed seeds, so `Initialize` was a one-time, winner-takes-all instruction: the first funded signer to call it became the permanent `PoolConfig.authority`, with no transfer or renounce path. That key installs the Groth16 verifying keys, rotates the disclosure committee, and registers compelled-disclosure requesters, so a front-running attacker could prove spends against keys they generated (or brick the pool for honest users) and turn the "governed, logged disclosure" guarantee into attacker-controlled surveillance over every honest deposit.

`Initialize` now requires the signer to be the committed `BOOTSTRAP_AUTHORITY`. The check is one 32-byte comparison against a constant before any CPI. A new adversarial Surfpool case proves it: against the unpatched program an unapproved first initializer succeeds and captures the config, and with the check it is refused with no state created. The example readme records the production guidance — generate the bootstrap authority off-circuit, commit only its public key, and initialize in the deployment ceremony.

The measured cost is ratcheted into `scripts/compute-unit-policy.json`: privacy pool `initialize` 49,265 (+20) and `requestDisclosure` 11,605 (+7). Every other instruction is byte-identical, including `deposit`, `withdraw`, and `transfer`.

# Check the tier-1 disclosure deadline arithmetic

`RequestDisclosure` computed `now + window` unchecked. A wrapped deadline lands in the past and silently collapses the challenge window tier 1 exists to provide. The addition now goes through a checked `challenge_deadline` helper with boundary unit tests covering the exact limit and the refusal one second past it.
