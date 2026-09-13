---
pina: fix
pina_cli: fix
---

# Name migration budget failures by their remedy

On-chain migration budget failures are now separately diagnosable. `MigrationWorkspaceExceeded` (`0xFFFF_FFF4`), `MigrationAccountGrowthExceeded` (`0xFFFF_FFF3`), and `MigrationLamportBudgetExceeded` (`0xFFFF_FFF2`) replace the single `MigrationBudgetExceeded` for the executor's workspace, realloc-growth, and rent-budget checks, and each variant's rustdoc names the constant that fixes the failure. `MigrationUnavailable` documents that its remedy is rebalancing a ladder that exceeds the account's generated `MAX_INLINE_STEPS`, and the legacy aggregate code stays at `0xFFFF_FFF5` so program binaries compiled before the split remain decodable. `pina migrations make` now quotes the shared `max_lamports` remedy in its growth warning, names the on-chain error it prevents, and warns separately when the cumulative worst-case growth a supported stale account walks across its inline ladder — not only one adjacent transition — exceeds the runtime's 10,240-byte per-instruction realloc cap.
