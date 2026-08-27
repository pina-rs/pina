# Getting Started

## Prerequisites

- Rust nightly toolchain from `rust-toolchain.toml`
- `devenv` (Nix-based environment)
- `gh` (for GitHub workflows)

## Setup

<!-- {=devEnvironmentSetupCommands} -->

```bash
devenv shell
install:all
```

<!-- {/devEnvironmentSetupCommands} -->

You can also scaffold a new project directly with the CLI:

```bash
npm install --global @pina-rs/cli
pina init my_program
```

See `pina init --help` for options like `--path` and `--force`. The [CLI reference](./cli/index.md) documents every command, output contract, and automation workflow.

For agent-assisted project work, install the [Pina skill](./agent-skill.md).

If `pnpm-workspace.yaml` sets `useNodeVersion`, `devenv shell` activates the matching pnpm-managed `node`/`npm`/`npx`/`corepack` toolchain automatically.

## Build and test

<!-- {=buildAndTestCommands} -->

```bash
cargo build --all-features
cargo test
```

<!-- {/buildAndTestCommands} -->

For a deterministic Docker build suitable for Solana's verified-build workflow, install `solana-verify` 0.5.1, start Docker, commit the complete source tree, and run:

```bash
pina build --verify
```

This produces the canonical deploy artifact plus a hash-bound Pina build record. It does not perform on-chain verification. See [`pina build`](./cli/build.md#deterministic-verified-build-artifacts) for the trust model, prerequisites, and limitations.

## Common quality checks

<!-- {=commonQualityChecksCommands} -->

```bash
lint:clippy
lint:format
verify:docs
```

<!-- {/commonQualityChecksCommands} -->

## Generate a Codama IDL

```bash
pina idl --path ./examples/counter_program --output ./codama/idls/counter_program.json
```

See [Codama Workflow](./codama-workflow.md) for end-to-end generation and external-project usage.

Before adapting an example for a program that controls assets, work through the [Production Readiness](./production-readiness.md) gate. Examples demonstrate scoped framework behavior; they are not audited deployment templates.

## Build this documentation

<!-- {=docsBuildCommand} -->

```bash
docs:build
```

<!-- {/docsBuildCommand} -->

The generated site is written to `docs/book/`.
