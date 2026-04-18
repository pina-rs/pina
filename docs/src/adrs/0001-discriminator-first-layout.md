# ADR 0001: Keep discriminator-first typed layouts

- Status: Accepted
- Date: 2026-04-18
- Deciders: Pina maintainers
- Related: [Core concepts](../core-concepts.md), [Security model](../security-model.md)

## Context

Pina's account, instruction, and event types are designed around fixed-size, zero-copy layouts.

That only works if the byte contract is explicit and stable. The project needs a layout model that is easy to validate at runtime, easy to generate through Codama, and hard to reinterpret accidentally.

## Decision

Pina keeps discriminator bytes as the first field inside every typed `#[account]`, `#[instruction]`, and `#[event]` layout.

The discriminator width is part of the ABI and is limited to `u8`, `u16`, `u32`, or `u64`.

From that decision follow a few rules:

- discriminator values are part of the protocol contract
- field order is part of the protocol contract
- widening or changing discriminator values is a breaking change
- incompatible layout changes require explicit migration instead of in-place reinterpretation

## Consequences

Benefits:

- runtime validation can do a fixed discriminator read plus `size_of::<T>()` checks
- generated Rust clients can match on-chain layouts exactly
- zero-copy parsing stays simple and predictable
- the compatibility surface is easy to explain in docs and reviews

Costs:

- account and instruction layouts must be treated like ABI, not ordinary Rust structs
- field reordering and in-place discriminator changes become explicit migrations
- compatibility with systems that expect external discriminator headers needs adapters, not silent reuse

## Alternatives considered

### External discriminator headers

Rejected because they split the byte contract across a manual header parser plus a typed payload parser. That makes validation paths less uniform and weakens compiler-assisted layout guarantees.

### Dynamic serialization formats

Rejected because they add copy/parse overhead, complicate `no_std` use, and weaken the fixed-layout guarantees that Pina uses for predictable compute costs.
