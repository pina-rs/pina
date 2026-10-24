---
pina: none
---

# Constrain privacy-pool Merkle path directions

Require Boolean direction witnesses in both privacy-pool spend circuits and use conditional selection for child placement. Newly initialized pools require new withdraw and transfer proving/verifying keys; fixed key slots prevent replacing keys in existing funded pools, which need a separately audited migration. Resolve test-harness lint failures and cover the account-data validation contract.
