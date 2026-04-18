# ADR 0004: Preserve the `no_std` and no-allocator boundary

- Status: Accepted
- Date: 2026-04-18
- Deciders: Pina maintainers
- Related: [Crates and features](../crates-and-features.md), [Core concepts](../core-concepts.md)

## Context

Pina targets on-chain Solana programs first. Those programs run in a restricted execution environment where dependency surface area, allocator behavior, and binary size all matter.

Treating `no_std` as optional or allowing heap-heavy code paths in core runtime APIs would weaken both the performance story and the predictability of on-chain behavior.

## Decision

Pina treats `no_std` compatibility and allocator avoidance as architecture, not convenience.

That means:

- on-chain crates must compile for SBF without requiring `std`
- host-only conveniences stay behind `cfg(test)` or explicit host build guards
- fixed-size Pod layouts, stack data, and borrow-based APIs are preferred over heap allocation in instruction paths
- workspace rules continue to deny `unsafe_code` and `unstable_features` by default

## Consequences

Benefits:

- on-chain programs keep a small and predictable runtime surface
- developers can reason about cost and failure modes without hidden allocator behavior
- examples and generated clients stay aligned with the actual on-chain target model

Costs:

- some otherwise ergonomic Rust libraries are unsuitable for core runtime paths
- host/test helper code often needs explicit cfg boundaries
- API design has to favor fixed-size, deterministic shapes over flexible heap-backed ones

## Alternatives considered

### Allow a general allocator in core on-chain paths

Rejected because it increases binary and behavioral complexity without helping the framework's main goals.

### Treat `no_std` as a best-effort property only

Rejected because CI, examples, and public APIs would drift toward host assumptions over time.
