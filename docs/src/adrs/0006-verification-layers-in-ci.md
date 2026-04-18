# ADR 0006: Use layered verification in CI

- Status: Accepted
- Date: 2026-04-18
- Deciders: Pina maintainers
- Related: [CI and releases](../ci-and-releases.md), #122, #124, #125, #126

## Context

Pina makes claims about safety, compatibility, and performance. Those claims are not covered by a single kind of test.

Ordinary unit and integration tests catch many behavioral regressions, but they do not cover undefined behavior, macro diagnostics, feature-flag drift, generated-client drift, or compute-unit regressions on their own.

## Decision

Pina treats CI as layered verification.

The expected layers are:

- standard tests for behavior and regression coverage
- feature-matrix checks for compatibility across supported configurations
- compile-fail tests for proc-macro diagnostics
- Miri for borrow and undefined-behavior regressions in sensitive loader paths
- IDL and generated-client verification for schema stability
- security verification and repository-specific linting
- binary-size and compute-unit reporting for performance drift

Static `pina profile` comparisons are the default CI mechanism for compute-unit regression reporting because they are deterministic and stable for PR-vs-base comparison.

## Consequences

Benefits:

- each high-risk bug class has a matching verification layer
- safety and performance claims stay enforceable in pull requests instead of only in release notes
- contributors can tell which lane failed and what class of invariant it protects

Costs:

- CI takes longer and is more operationally complex
- some lanes need careful threshold tuning to avoid noisy failures
- performance verification remains approximate unless paired with deeper runtime benchmarking

## Alternatives considered

### Rely on `cargo test` alone

Rejected because it leaves entire bug classes untested, especially macro diagnostics, UB regressions, and feature-flag drift.

### Use only runtime performance measurements

Rejected because runtime measurements are noisier and harder to compare deterministically in PR CI than static profiler output.
