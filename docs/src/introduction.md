# Introduction

<p align="center">
	<img
		src="./assets/logo.png"
		alt="Pina: a pineapple made from interlocking ribbons"
		width="140"
	>
</p>

Pina is a Solana program framework built on [`pinocchio`](https://github.com/anza-xyz/pinocchio). Programs written with it deploy with the smallest binaries of any Solana framework, spend compute like hand-written code, and evolve through managed ABI migrations. Every on-chain crate is `no_std` and allocation-free.

This book is the documentation home for the project. It explains the ideas behind the framework, walks through building and shipping a program, and records the trade-offs the codebase is built on. API reference lives on [docs.rs](https://docs.rs/pina/).

> Pina is unaudited and still hardening. Read the [security model](./security-model.md) and the [production-readiness gate](./production-readiness.md) before you deploy a program that holds value.

## Where to start

- **New to Pina?** Follow [Getting Started](./getting-started.md), then build [your first program](./tutorials/first-program.md) end to end.
- **Coming from Anchor?** [Migrating from Anchor](./tutorials/migrating-from-anchor.md) maps each Anchor concept to its Pina equivalent.
- **Evaluating frameworks?** The [framework comparison](./framework-comparison.md) measures size and compute units for the same programs written in five frameworks.
- **Shipping an upgrade?** [How ABI migrations flow](./migrations/flow.md) covers version envelopes, transitions, and the publication ledger.

## What this book covers

- **Start here:** the [project goals](./project-goals.md), toolchain setup, tutorials, and the annotated [examples](./examples.md).
- **Framework:** [core concepts](./core-concepts.md) (`#[account]`, `#[instruction]`, `#[derive(Accounts)]`, discriminators, and validation chains), [declarative validation](./validation.md), [crate features](./crates-and-features.md), and the [Codama workflow](./codama-workflow.md) for generated clients.
- **Tooling:** the complete [CLI reference](./cli/index.md), including automation-safe output contracts, and the packaged [agent skill](./agent-skill.md).
- **ABI migrations:** how account and instruction schemas evolve without breaking deployed programs, and a migration guide for every breaking release.
- **Performance:** [compute-unit](./compute-unit-performance.md) and [program-size](./program-size.md) guidance, backed by the benchmarks in CI.
- **Security:** the [security model](./security-model.md), the [PinaPod safety boundary](./pinapod-safety-boundary.md), and the [security lints](./security-lints.md) that check program source.
- **Project:** [architecture decision records](./adrs/index.md), the [development workflow](./development-workflow.md), and [CI and releases](./ci-and-releases.md).
