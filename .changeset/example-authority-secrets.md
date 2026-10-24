---
pina: fix
pina_test: feat
---

# Remove published authority keys from deployable examples

Bind privacy-pool initialization to the deployment's upgrade authority through its canonical loader-owned ProgramData account. Initializers must pass that read-only account after the system program and initialize before revoking the program's upgrade authority. Restrict prop-AMM price updates to the stored oracle authority so rotation removes every previous publisher's privilege.

Add an isolated Surfnet helper for configuring deployment authority in tests.

Existing pool accounts retain their stored authority after a code upgrade. Pools initialized with a disclosed fixture key need separate recovery or redeployment.
