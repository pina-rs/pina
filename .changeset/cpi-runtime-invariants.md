---
pina: fix
pina_cli: fix
---

# Enforce CPI signer and PDA target invariants

Make CPI signer metadata describe the called instruction instead of inheriting outer-instruction privileges. Generated-style builders can now request signer accounts explicitly, so both transaction signatures and `invoke_signed` PDA seeds satisfy the same typed CPI API without forwarding unrelated signatures.

Require `CpiContext` to receive a validated `Program<T>` and reject PDA account creation when the supplied account address does not match the requested seeds, bump, and owner. This prevents independently signing accounts from bypassing PDA derivation checks on both zero-balance and pre-funded allocation paths.
