---
pina_cli: feat
---

# Deploy from verified input snapshots

Copy deployment artifacts and signer keypairs into a private snapshot, verify the copied bytes against the approved plan, and keep that snapshot alive while the deployment command runs. Concurrent replacement of the original files can no longer change what the child process receives after pre-execution validation.
