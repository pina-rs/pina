---
pina_cli: fix
---

# Keep the unchecked PDA builder in the IDL account analysis

The typed creation-builder allowlist that classifies instruction accounts as PDAs did not include `CreateProgramAccountWithUncheckedBump`, so handlers using it lost their PDA classification: the generated IDL dropped the account's `pdaValueNode` default and the JavaScript client stopped emitting its `…InstructionAsync` builder. The builder is now recognized, and the affected IDLs and clients are regenerated.
