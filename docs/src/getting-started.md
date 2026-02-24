# Getting Started

## Prerequisites

- Rust nightly toolchain from `rust-toolchain.toml`
- `devenv` (Nix-based environment)
- `gh` (for GitHub workflows)

## Setup

```bash
devenv shell
install:all
```

## Build and test

```bash
cargo build --all-features
cargo test
```

## Common quality checks

```bash
lint:clippy
lint:format
verify:docs
```

## Generate a Codama IDL

```bash
pina idl --path ./examples/counter_program --output ./codama/idls/counter_program.json
```

See [Codama Workflow](./codama-workflow.md) for end-to-end generation and external-project usage.

## Build this documentation

```bash
docs:build
```

The generated site is written to `docs/book/`.
