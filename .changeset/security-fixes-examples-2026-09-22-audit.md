---
pina: fix
pina_cli: fix
---

# Close live 2026-09-22 audit example findings

The examples below ship with the workspace; their IDLs and generated clients are regenerated in the same change, so the release line records the ABI movement alongside the framework fixes.

Multisig: creation and config changes reject a nonzero proposal TTL at or below the timelock (it expires every proposal before its execution window can open); expired and stale proposals are permissionlessly closable with their rent refunded to the configured collector; spending-limit closures during config execution refund the configured rent collector instead of the executor-supplied rent payer; and member-tail shrinkage refunds the collector too — `commit_multisig` selects the resize rent account by direction, growth still charging the executing rent payer, so an executor cannot pocket a governed refund by roster reduction. The authority path (`ConfigAuthorityExecute`) gains the same validated `rent_collector` account the governed path has.

Staking: the pool tracks an `outstanding_rewards` liability counter — advanced by the index increment on `SetRewardIndex`, reduced by each payout on `Claim`, and holding banked pending rewards through withdrawals — and refuses an index update whose outstanding liability exceeds `u64` capacity or the canonical reward vault's balance. The vault now backs what is actually owed, so paid-out rewards stop reserving capacity and banked rewards keep reserving it; equal entitlements no longer depend on claim order. `InitializePool` rejects extended Token-2022 mints so no pool can be born without working exits.

Vesting: `Initialize` moves the full allocation from the admin's ATA into the vault in the same instruction (a schedule cannot exist unfunded) and rejects extended Token-2022 mints; `Cancel` settles nothing before the cliff (the beneficiary's pre-cliff entitlement is zero) and after it returns the vested-but-unclaimed entitlement to the beneficiary's ATA before any remainder returns to the administrator, bound to the stored owner and mint.
