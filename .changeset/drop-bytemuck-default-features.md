---
pina: fix
---

# Stop enabling `bytemuck` on `solana-address`/`solana-pubkey`

The workspace requested `solana-address/bytemuck` and `solana-pubkey/bytemuck`. Pina used `bytemuck` for its own pod primitives until the migration to `pinapod`, so nothing in the workspace reads the `Pod` or `Zeroable` derives those features add, and no first-party code calls a `bytemuck` API.

Both entries now request what Pina actually depends on. `solana-address` asks for `copy`, which derives `Copy` on `Address` — the bound `pinapod`'s `ZcElem` requires and the only part of `bytemuck`'s feature set that was load-bearing. Requesting it directly removes the dependence on `bytemuck`'s implication, and on `pinocchio` and `pinapod` enabling `copy` transitively. `solana-pubkey` no longer requests `bytemuck` at all.

Deployable programs are unchanged: `bytemuck` contributed no symbols to any built program, so all 24 `bpf-entrypoint` examples produce byte-identical sizes and an identical instruction mix with and without it. This is a dependency-graph cleanup, not a size or compute-unit change.
