# Review Follow-ups

This project now tracks and resolves relevant previously ignored pull-request feedback where it still applies to the current codebase.

## Addressed items

- Enabled `solana-address` `curve25519` feature to ensure PDA helper APIs are available in host builds.
- Replaced unchecked `current + 1` increment in the counter example with checked arithmetic and `ProgramError::ArithmeticOverflow` on failure.
- Fixed stale hello example docs that described behavior not present in code.
- Added missing `AccountValidation` implementations for all token account/mint types used by token conversion helpers.

## Explicitly ignored as not relevant

Some unresolved comments pointed to paths that no longer exist in the current repository (for example removed historical `security/` and `lints/` paths). These were not applied because there is no active code location to patch.
