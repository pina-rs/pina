# Examples

The `examples/` workspace members demonstrate focused usage patterns. They are not audited applications or deployment templates:

- `hello_solana`: minimal program structure and instruction dispatch.
- `counter_program`: PDA creation, mutation, and account validation.
- `todo_program`: PDA-backed state with boolean + digest updates.
- `transfer_sol`: lamport transfers and account checks.
- `escrow_program`: richer multi-account flow and token-oriented logic.
- `vesting_program`: schedule-state and vault-ATA scaffold; it does not enforce time-based vesting or transfer tokens.
- `role_registry_program`: role-based configuration and registry PDAs with admin rotation.
- `staking_rewards_program`: staking account and bookkeeping scaffold; deposit, withdraw, and claim do not transfer tokens.
- `profile_program`: user profile registry with fully initialized bounded UTF-8 and tag fields plus checked semantic accessors.
- `pina_bpf`: minimal pina-native BPF hello world with nightly `build-std=core,alloc`.
- `prop_amm_program`: Pina-native semantic port of Anchor `anchor-next` benchmark `prop-amm`, focused on authority-controlled oracle updates without the upstream asm fast path.
- `anchor_declare_id`: first Anchor test parity port, focused on program-id mismatch checks.
- `anchor_declare_program`: Anchor `declare-program` parity for external-program ID checks.
- `anchor_duplicate_mutable_accounts`: explicit duplicate mutable account validation pattern.
- `anchor_errors`: Anchor-style custom error code and guard helper parity.
- `anchor_events`: event schema parity through deterministic serialization checks.
- `anchor_floats`: float data account create/update flow with authority validation.
- `anchor_system_accounts`: system-program owner validation parity.
- `anchor_sysvars`: clock/rent/stake-history sysvar validation parity.
- `anchor_realloc`: authority-bound PDA realloc lifecycle, growth limits, and duplicate-target safety checks.

Use examples as references for the specific framework behavior each one demonstrates. Do not infer unimplemented economic behavior from an instruction name. Read [Production Readiness](./production-readiness.md) before adapting an example for an asset-bearing program.

Anchor test-suite parity progress is tracked in [Anchor Test Porting](./anchor-test-porting.md).

Every example directory includes a local `readme.md` with purpose, coverage, limitations, and run commands. Some E2E suites skip when their SBF binary is missing; production CI should build the artifact first and treat a missing artifact as a failure.

When adding new examples:

- Keep instruction/account discriminator handling explicit.
- Use checked arithmetic in state transitions.
- Include unit tests and clear doc comments for every instruction path.
