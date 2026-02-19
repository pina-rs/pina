---
pina_cli: minor
---

Add codama SDK integration tests for IDL client generation.

Generate both Rust and JavaScript clients from pina-generated IDLs using the
codama SDK renderers, and verify that the generated code compiles correctly.
The test pipeline covers all four example programs (counter_program, escrow_program,
hello_solana, transfer_sol) and validates:

- IDL parsing by the codama SDK
- Rust client code generation and compilation (`cargo check`)
- JavaScript client code generation and TypeScript type-checking (`tsc --noEmit`)
