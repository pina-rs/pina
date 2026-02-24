# anchor_errors

Pina parity port of Anchor's custom error handling patterns.

## What this demonstrates

- Stable custom error numbering.
- Guard helpers (`require_eq`, `require_neq`, etc.).
- Deterministic mapping from instruction variants to errors.

## Differences From Anchor

- Error checks are plain Rust helper functions rather than Anchor macros.
- Instruction behavior is centralized in `process_instruction_variant`.
- Numeric error-code expectations are asserted in unit tests.

## Run

```sh
cargo test -p anchor_errors
pina idl --path examples/anchor_errors --output codama/idls/anchor_errors.json
```
