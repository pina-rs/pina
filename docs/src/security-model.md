# Security Model

Pina's safety posture is built around explicit validation and predictable state transitions.

## Core invariants

- Type correctness: account bytes must match expected discriminator and layout.
- Authority correctness: signer/owner checks must precede mutation.
- PDA correctness: seed and bump checks must gate PDA-bound operations.
- Value correctness: arithmetic and balance mutations must be checked.

## High-priority guardrails

- Prefer checked arithmetic (`checked_add`, `checked_sub`) for all user-facing or balance-affecting values.
- Ensure all token account types used by helper traits implement `AccountValidation`.
- Keep close/transfer helpers conservation-safe (no temporary double-crediting).

## Testing strategy

- Unit tests for negative validation cases.
- Regression tests for every previously fixed bug class.
- Integration tests for cross-account invariants where mutation order matters.
