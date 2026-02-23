# Examples

The `examples/` workspace members demonstrate practical usage patterns:

- `hello_solana`: minimal program structure and instruction dispatch.
- `counter_program`: PDA creation, mutation, and account validation.
- `todo_program`: PDA-backed state with boolean + digest updates.
- `transfer_sol`: lamport transfers and account checks.
- `escrow_program`: richer multi-account flow and token-oriented logic.

Use examples as reference implementations for account layout, instruction parsing, and validation ordering.

When adding new examples:

- Keep instruction/account discriminator handling explicit.
- Use checked arithmetic in state transitions.
- Include unit tests and clear doc comments for every instruction path.
