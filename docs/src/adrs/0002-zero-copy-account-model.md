# ADR 0002: Keep zero-copy behind explicit validation

- Status: Accepted
- Date: 2026-04-18
- Deciders: Pina maintainers
- Related: [Security model](../security-model.md), `security/loaders-audit.md`

## Context

Low compute usage is a stated project goal, and zero-copy account access is one of the biggest reasons to use Pina instead of heavier Solana framework stacks.

But zero-copy is only defensible when the layout contract is tight. Unsafe or dynamically shaped reinterpretation can erase the very safety properties the framework is supposed to enforce.

## Decision

Pina keeps zero-copy account and instruction handling as a core design choice, but delegates representation and byte-to-view conversion to zeropod's native derive model.

In practice that means:

- application types are native schemas deriving `zeropod::ZeroPod`; loaders return the generated `TypeZc` storage view
- typed loads must validate discriminator, size, content (`ZcValidate`), and relevant account identity constraints before use
- dynamic, variable-length, or schema-driven reinterpretation is out of scope for the core loader model
- Pina does not manually implement zeropod's unsafe traits, duplicate its pointer casts, or expose a schema/storage-view object representation as bytes
- fixed-capacity inactive storage is unobservable through Pina APIs

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
