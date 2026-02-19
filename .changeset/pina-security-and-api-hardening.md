---
pina: patch
---

Hardened account and discriminator handling to avoid panic paths and unsafe deserialization assumptions:

- `IntoDiscriminator` primitive implementations now handle short input slices without panicking.
- `AsAccount`/`AsAccountMut` now require exact account data length before reinterpretation.
- PDA helper wrappers now work consistently on native targets and include roundtrip tests.
- Lamport send/close helpers now reject same-account recipients and enforce writable preconditions before balance mutation.

Also improved security examples by replacing saturating transfer/withdraw arithmetic with checked math that returns explicit `ProgramError` values on insufficient funds or overflow.
