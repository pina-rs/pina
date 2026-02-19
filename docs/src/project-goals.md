# Project Goals

Pina's codebase currently optimizes for the following goals.

## 1. Performance and low compute units

- Prefer `pinocchio` primitives over heavier Solana SDK surfaces.
- Minimize instruction overhead by using zero-copy layouts and typed discriminators.
- Keep runtime checks explicit but lightweight.

## 2. `no_std`-first smart contract ergonomics

- Keep crates deployable to Solana SBF targets.
- Avoid patterns that introduce allocator/runtime assumptions.
- Gate entrypoint-specific behavior behind features.

## 3. Safety for account handling and state transitions

- Strong discriminator and owner checks.
- Explicit validation chains for signer, writable, PDA seeds, and type.
- Defensive arithmetic and transfer operations.

## 4. Macro-powered developer experience

- Reduce boilerplate with `#[account]`, `#[instruction]`, `#[event]`, `#[error]`, and `#[derive(Accounts)]`.
- Keep generated behavior predictable, documented, and tested.

## 5. Maintainability and release quality

- Reproducible dev environments (`devenv` + pinned tooling).
- CI coverage for linting, tests, and builds.
- Changelog-driven release discipline via changesets.
