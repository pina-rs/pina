# anchor_errors

<br>

Pina parity port of Anchor's custom error handling patterns.

## What this demonstrates

<br>

- Stable custom error numbering.
- Guard helpers (`require_eq`, `require_neq`, etc.).
- Deterministic mapping from instruction variants to errors.

## Differences From Anchor

<br>

- Error checks are plain Rust helper functions rather than Anchor macros.
- Instruction behavior is centralized in `process_instruction_variant`.
- Numeric error-code expectations are asserted in unit tests.

## Run

<br>

```sh
cargo test -p anchor_errors
pina idl --path examples/anchor_errors --output codama/idls/anchor_errors.json
```
