# Pina

Pina is a high-performance Solana smart-contract framework built on top of [`pinocchio`](https://github.com/anza-xyz/pinocchio). The project focuses on low compute-unit usage, small dependency surface area, and strong account validation ergonomics for on-chain Rust programs.

This book is the single place for project documentation. It complements API reference docs by describing architecture, patterns, workflows, and quality standards used across the repository.

## What you get in this book

- The project's goals and trade-offs.
- Setup and day-to-day development workflow.
- Core framework concepts (`#[account]`, `#[instruction]`, `#[derive(Accounts)]`, discriminator model, and validation chains).
- Codama IDL/client-generation workflow (including external-project invocation).
- Guidance for examples and security-focused development.
- CI/release pipeline expectations.
- A practical recommendations roadmap for improving goal alignment.
