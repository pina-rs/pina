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

## Build and test

<!-- {=buildAndTestCommands} -->

```bash
cargo build --all-features
cargo test
```

<!-- {/buildAndTestCommands} -->

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

## Build this documentation

<!-- {=docsBuildCommand} -->

```bash
docs:build
```

<!-- {/docsBuildCommand} -->

The generated site is written to `docs/book/`.
