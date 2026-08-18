# ADR 0002: Keep zero-copy behind explicit validation

- Status: Accepted
- Date: 2026-04-18
- Deciders: Pina maintainers
- Related: [Security model](../security-model.md), `security/loaders-audit.md`

## Context

Low compute usage is a stated project goal, and zero-copy account access is one of the biggest reasons to use Pina instead of heavier Solana framework stacks.

But zero-copy is only defensible when the layout contract is tight. Unsafe or dynamically shaped reinterpretation can erase the very safety properties the framework is supposed to enforce.

## Decision

Pina keeps zero-copy account and instruction handling as a core design choice, but only for fixed-size layouts that are validated before reinterpretation.

In practice that means:

- zero-copy types must be fixed-size and Pod-compatible
- typed loads must validate discriminator, size, and relevant account identity constraints before use
- dynamic, variable-length, or schema-driven reinterpretation is out of scope for the core loader model
- performance-motivated `unsafe` is only acceptable when the soundness boundary is narrow and documented

## Consequences

Benefits:

- no heap copies are required for common account access paths
- account parsing stays predictable in both runtime cost and memory behavior
- the framework can keep `no_std` and low-dependency goals without abandoning typed APIs

Costs:

- some data models must use explicit versioning or companion accounts instead of variable-length in-place layouts
- loader APIs need stronger lifetime coupling than a simple `&T` return type can provide
- future extensions must prove they preserve layout and aliasing safety, not just correctness in happy-path tests

## Alternatives considered

### Copy-based deserialization into owned structs

Rejected because it adds compute overhead, increases stack or heap pressure, and gives up one of Pina's primary performance advantages.

### Unsafe dynamic zero-copy for arbitrary layouts

Rejected because it makes soundness depend on ad-hoc caller discipline and scattered invariants instead of framework-level rules.
