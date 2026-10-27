---
pina: fix
---

# Authenticate Multisig Config Initialization

Require the deployment's current upgrade authority to initialize the multisig example's global configuration, authenticated through its canonical loader-owned ProgramData account. Reject unrelated signers, stale authorities, immutable deployments, and forged or malformed metadata before creating accounts or moving rent. Generated clients now require the extra read-only account after the system program. Existing configuration accounts retain their stored authority.
