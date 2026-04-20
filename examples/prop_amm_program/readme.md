# `prop_amm_program`

<br>

A Pina-native port of Anchor `anchor-next` benchmark example `bench/programs/prop-amm/anchor-v2`.

## What it covers

<br>

- A minimal oracle account with explicit discriminator-first Pod layout.
- Non-PDA account initialization via `create_account(...)`.
- Global update-authority checks modeled after Anchor `prop-amm` v2.
- Authority rotation validated against stored account state.
- Native unit tests plus Mollusk e2e coverage.
- A generated-style `src/cpi.rs` module showing how typed `CpiHandle` account structs can drive allocator-free on-chain CPI helpers.

## Important adaptation notes

<br>

This example is a semantic port, not a byte-for-byte benchmark clone.

Notably, it does **not** port Anchor v2's handwritten assembly fast path from:

- `bench/programs/prop-amm/anchor-v2/src/asm/entrypoint.s`

Pina keeps the implementation fully Rust-based and aligned with the workspace's `unsafe_code = "deny"` and `unstable_features = "deny"` constraints.

## Run

<br>

```bash
cargo test -p prop_amm_program -- --nocapture
pina idl --path examples/prop_amm_program --output codama/idls/prop_amm_program.json
```

## Optional SBF build

<br>

```bash
cargo build --release --target bpfel-unknown-none -p prop_amm_program -Z build-std -F bpf-entrypoint
```
