# Examples

The `examples/` workspace members demonstrate practical usage patterns:

- `hello_solana`: minimal program structure and instruction dispatch.
- `counter_program`: PDA creation, mutation, and account validation.
- `todo_program`: PDA-backed state with boolean + digest updates.
- `transfer_sol`: lamport transfers and account checks.
- `escrow_program`: richer multi-account flow and token-oriented logic.
- `pinocchio_bpf_starter`: upstream BPF starter-style hello world with `sbpf-linker`.
- `anchor_declare_id`: first Anchor test parity port, focused on program-id mismatch checks.
- `anchor_declare_program`: Anchor `declare-program` parity for external-program ID checks.
- `anchor_duplicate_mutable_accounts`: explicit duplicate mutable account validation pattern.
- `anchor_errors`: Anchor-style custom error code and guard helper parity.
- `anchor_events`: event schema parity through deterministic serialization checks.
- `anchor_floats`: float data account create/update flow with authority validation.
- `anchor_system_accounts`: system-program owner validation parity.
- `anchor_sysvars`: clock/rent/stake-history sysvar validation parity.
- `anchor_realloc`: realloc growth and duplicate-target safety checks.

Use examples as reference implementations for account layout, instruction parsing, and validation ordering.

Anchor test-suite parity progress is tracked in [Anchor Test Porting](./anchor-test-porting.md).

When adding new examples:

- Keep instruction/account discriminator handling explicit.
- Use checked arithmetic in state transitions.
- Include unit tests and clear doc comments for every instruction path.
