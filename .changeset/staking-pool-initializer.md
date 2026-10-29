---
pina: none
---

# Authenticate staking pool initialization

Require the reward mint authority to sign as the administrator when initializing the staking rewards example's canonical pool. Reject unrelated signers and revoked authorities before creating state or vaults, and enable the front-running regression in the regular Surfpool suite. Existing pool addresses, account layouts, and instruction bytes are unchanged.
