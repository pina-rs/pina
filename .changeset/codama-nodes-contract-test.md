---
pina_codama_nodes: none
---

# Pin the client contract test to the released surfaces

The vesting and staking clients gained accounts in this release (the claim clock sysvar and refund path, the staking reward vault), so the package's contract test pins the new slot counts and roles. Test-only; no release note.
