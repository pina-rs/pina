# Architecture decision records

This section captures the durable architectural decisions behind Pina's public model, safety posture, and verification strategy.

## ADR format and naming

- Files live under `docs/src/adrs/`.
- ADRs use the naming pattern `NNNN-short-slug.md`.
- The starter template lives at `docs/src/adrs/0000-template.md`.
- Architecture-impacting pull requests should link the ADR they follow or update.

## ADR index

| ADR                                                      | Status   | Decision                                                                        |
| -------------------------------------------------------- | -------- | ------------------------------------------------------------------------------- |
| [ADR 0001](./0001-discriminator-first-layout.md)         | Accepted | Keep discriminator bytes as the first field inside typed layouts.               |
| [ADR 0002](./0002-zero-copy-account-model.md)            | Accepted | Keep zero-copy for fixed-size Pod layouts, but only behind explicit validation. |
| [ADR 0003](./0003-guard-backed-typed-account-loaders.md) | Accepted | Keep runtime borrow guards alive for the full typed loader lifetime.            |
| [ADR 0004](./0004-no-std-and-no-allocator-boundary.md)   | Accepted | Preserve `no_std` / no-allocator constraints for on-chain code paths.           |
| [ADR 0005](./0005-token-feature-boundaries.md)           | Accepted | Keep SPL token support optional and feature-gated.                              |
| [ADR 0006](./0006-verification-layers-in-ci.md)          | Accepted | Treat CI as layered verification, not a single all-purpose test lane.           |

## How to use this section

Use these ADRs when you need to answer questions like:

- why Pina uses discriminator-first layouts instead of external headers
- when zero-copy is allowed, and where the safety boundaries are
- why typed account loaders must be guard-backed instead of returning bare references
- why `no_std` and allocator constraints are treated as architecture, not implementation detail
- why token helpers are optional instead of always-on
- why Miri, compile-fail tests, feature matrices, and compute-unit checks all exist at once
