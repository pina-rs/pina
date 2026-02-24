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

## Build this documentation

```bash
docs:build
```

The generated site is written to `docs/book/`.
