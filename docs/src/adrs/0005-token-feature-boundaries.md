# ADR 0005: Keep token support optional and feature-gated

- Status: Accepted
- Date: 2026-04-18
- Deciders: Pina maintainers
- Related: [Crates and features](../crates-and-features.md), #121, #126

## Context

Many Solana programs need SPL token, Token-2022, or ATA helpers, but many do not. Making token support unconditional would increase the dependency surface and blur the boundary between core framework validation and token-specific conveniences.

At the same time, token helpers need to be first-class when the feature is enabled, including correct owner validation and Token-2022 compatibility.

## Decision

Pina keeps token support behind the optional `token` feature.

From that decision follow a few rules:

- core account validation and zero-copy APIs must compile and remain useful without `token`
- SPL token, Token-2022, and ATA helpers live behind the feature gate
- checked token loaders must validate owner and account-identity constraints before typed access
- feature-matrix CI must continue to cover at least default, no-default, token-only, and all-features configurations

## Consequences

Benefits:

- non-token programs do not pay dependency or API complexity they do not need
- token-heavy programs still get ergonomic helpers once they opt in
- feature-matrix testing becomes an explicit compatibility contract instead of a best effort

Costs:

- docs and tests must describe feature boundaries clearly
- token-related APIs must be careful not to leak assumptions into core no-feature paths
- compatibility work for Token-2022 needs dedicated coverage instead of being assumed by SPL token support

## Alternatives considered

### Make token support part of the default feature set

Rejected because it increases the default dependency surface and weakens the project's minimal-core story.

### Move token support entirely out of `pina`

Rejected for now because the helpers are part of the framework's core ergonomics, but the dependency cost still needs to stay opt-in.
