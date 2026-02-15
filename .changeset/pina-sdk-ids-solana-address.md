---
pina_sdk_ids: major
---

Migrate from `pinocchio-pubkey` to `solana-address` for program ID declarations:

- **`pinocchio_pubkey::declare_id!` → `solana_address::declare_id!`** — all 27 program and sysvar ID declarations now use the `solana-address` crate's `declare_id!` macro.
- **Dependency change** — replaced `pinocchio-pubkey` with `solana-address ^2.0` (features: `decode`).
