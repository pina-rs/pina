---
pina: fix
pina_cli: fix
---

# Close live 2026-09-22 audit example findings

The examples below ship with the workspace; their IDLs and generated clients are regenerated in the same change, so the release line records the ABI movement alongside the framework fixes.

Multisig: creation and config changes reject a nonzero proposal TTL at or below the timelock (it expires every proposal before its execution window can open); expired and stale proposals are permissionlessly closable with their rent refunded to the configured collector; and spending-limit closures and shrinks during config execution refund the configured rent collector instead of the executor-supplied rent payer.

Staking: `SetRewardIndex` receives the reward mint, token program, and canonical reward vault, and refuses an index whose aggregate liability (`total_staked * index / SCALE`, in `u128`) exceeds `u64` capacity or the vault's balance; `InitializePool` rejects extended Token-2022 mints so no pool can be born without working exits.

Vesting: `Initialize` moves the full allocation from the admin's ATA into the vault in the same instruction (a schedule cannot exist unfunded) and rejects extended Token-2022 mints; `Cancel` reads the clock and settles the vested-but-unclaimed entitlement to the beneficiary's ATA before any remainder returns to the administrator.
