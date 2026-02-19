---
pina: patch
---

Addressed outstanding review follow-ups and documentation quality:

- Enabled the `solana-address` `curve25519` feature in workspace dependencies so PDA helpers are available in host builds.
- Added missing `AccountValidation` implementations for token mint/account types used by token helpers.
- Replaced unchecked counter increment arithmetic in the example with checked overflow handling.
- Added an mdBook documentation site under `docs/` and wired docs verification into CI.
