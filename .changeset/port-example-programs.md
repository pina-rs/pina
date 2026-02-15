---
pina: note
---

Add three example programs ported from `solana-developers/program-examples`, demonstrating core pina features with comprehensive documentation and unit tests:

- **`hello_solana`** — Minimal program showing basic pina structure: `declare_id!`, `#[discriminator]`, `#[instruction]`, `#[derive(Accounts)]`, `ProcessAccountInfos`, and `nostd_entrypoint!`.
- **`counter_program`** — PDA-based account state management with `#[account]`, `create_program_account`, `as_account_mut`, validation chains, and seed macros.
- **`transfer_sol`** — Two SOL transfer methods: CPI via `system::instructions::Transfer` and direct lamport manipulation via `LamportTransfer::send()`, plus custom error types with `#[error]`.

Each example includes a learning progression from basic → intermediate → advanced, with detailed module-level docs explaining every pina feature used.
