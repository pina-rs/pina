# Recommendations

This section contains concrete suggestions to better align the codebase with Pina's goals.

## 1. Add performance regression baselines

Goal alignment: low compute units.

- Add benchmark harnesses for high-volume instruction paths (counter increment, escrow state transitions, token flows).
- Track baseline CU budgets in CI and fail when regressions exceed threshold.
- Keep benchmark inputs deterministic and versioned.

## 2. Strengthen feature-matrix testing

Goal alignment: `no_std` reliability + maintainability.

- Test a matrix of feature combinations (`default`, `--no-default-features`, `--features token`, `--all-features`).
- Include `bpfel-unknown-none` build checks for all example programs.
- Add one CI lane for docs/tests under minimal features to catch accidental default-feature coupling.

## 3. Expand security regression coverage

Goal alignment: safety.

- Add explicit regression tests for arithmetic overflow/underflow paths.
- Add tests for token transfer edge cases (insufficient funds, overflow on destination).
- Add tests for each account close/transfer helper to verify lamport conservation invariants.

## 4. Improve macro diagnostics quality

Goal alignment: developer experience.

- Add compile-fail tests for malformed macro attributes and unsupported discriminator configurations.
- Improve error messages to include expected/actual forms and actionable fix text.
- Maintain a docs page mapping macro attributes to generated behaviors.

## 5. Keep architecture decision records current

Goal alignment: maintainability.

- Link architecture-impacting pull requests to the ADR they follow or update.
- Add a new ADR when a public invariant, compatibility contract, or verification policy changes materially.
- Periodically review older ADRs and mark them superseded when the project direction changes.

## 6. Publish a migration guide from Anchor-style patterns

Goal alignment: adoption.

- Document direct mapping from common Anchor concepts to Pina equivalents.
- Provide before/after examples for account validation and instruction routing.
- Include expected CU/dependency differences for realistic workloads.
