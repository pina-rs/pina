# Multisig Program

An ambitious, production-shaped multisig wallet built with pina: permissioned members, threshold approvals, timelocked execution of arbitrary compiled messages, governed configuration, spending limits, and first-class migration support.

## What it covers

- **Consensus core**: sorted member rosters with permission masks (initiate / vote / execute), draft → active → approved / rejected / cancelled → executed proposal lifecycle, and stale-proposal invalidation whenever the consensus parameters move.
- **Bitmask voting**: approvals and rejections are `u32` bitmasks over the sorted roster — constant-time word arithmetic instead of sorted `Vec<Pubkey>` churn, so voting never resizes a proposal account.
- **Unified proposals**: one account carries the vote state and the payload — either a compiled vault message (multi-instruction, classic account ordering, per-proposal ephemeral signers) or a typed config action stream.
- **Timelock and expiry**: execution waits out a configurable timelock anchored at approval, and proposals carry a configurable lifetime so stale consent cannot linger.
- **Vote revocation**: any approver may revoke; dropping below the threshold returns the proposal to active, and a fresh timelock applies after re-approval.
- **Config action streams**: one governed instruction executes add/remove member, threshold, timelock, TTL, rent collector, config authority, and spending-limit actions; controlled multisigs run the same stream directly through the config authority.
- **Spending limits**: per-member allowances with period resets, destination allow-lists, and SOL or SPL transfer paths signed by the vault PDA.
- **Legacy import**: adopt an existing on-chain multisig account written in the classic Anchor layout (8-byte discriminator, inline member roster), parsed zero-copy and re-seeded into pina's compact state.
- **Migration envelope**: every account is versioned through `pina migrations`, with a checked-in manifest and generated ABI layout tests.

## Deliberately out of scope

- Address table lookups and transaction batching.
- Transaction buffers for messages larger than one transaction packet (vault messages are capped at 640 bytes).
- Token-2022 transfer paths and spl-associated-token account creation.
- Funds held by a legacy vault stay under the legacy program's control after an import.

See [Production Readiness](../../docs/src/production-readiness.md) before adapting this example for an asset-bearing program.

## Run

```sh
pina test --unit   # unit tests
pina test          # unit + Surfpool end-to-end journeys (builds the SBF artifact)
pina generate      # refresh the IDL and clients
```

Optional SBF build:

```sh
cargo build --release --target bpfel-unknown-none -p multisig_program -Z build-std -F bpf-entrypoint
```
