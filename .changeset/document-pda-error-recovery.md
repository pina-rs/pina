---
pina: docs
---

# Document recovery from invalid PDA creation bumps

Explain that noncanonical and unchecked PDA creation aborts execution when the runtime rejects an on-curve signer address. Callers that need to catch an invalid bump and continue can preflight the seeds, including the bump, with `create_program_address`. The fast default creation path and its compute-unit savings remain unchanged.
