# Anchor Test Porting

This page tracks sequential parity ports from `solana-foundation/anchor/tests` into `examples/`, using Rust-first tests (mollusk/native unit tests) instead of TypeScript.

## Port Status

- [ ] `anchor-cli-account` (no direct parity yet; Anchor CLI account decoding over dynamic `Vec`/`String` data is not a direct pina/no-std match)
- [ ] `anchor-cli-idl` (no direct parity yet; Anchor CLI IDL account lifecycle is Anchor-CLI-specific)
- [ ] `auction-house`
- [ ] `bench`
- [ ] `bpf-upgradeable-state`
- [ ] `cashiers-check`
- [ ] `cfo`
- [ ] `chat`
- [ ] `composite`
- [ ] `cpi-returns`
- [ ] `custom-coder`
- [ ] `custom-discriminator`
- [ ] `custom-program`
- [x] `declare-id` -> `examples/anchor_declare_id`
- [x] `declare-program` -> `examples/anchor_declare_program` (adapted)
- [x] `duplicate-mutable-accounts` -> `examples/anchor_duplicate_mutable_accounts` (adapted)
- [x] `errors` -> `examples/anchor_errors` (adapted)
- [x] `escrow` -> `examples/escrow_program` (adapted with parity-focused tests)
- [x] `events` -> `examples/anchor_events` (adapted event schema parity)
- [x] `floats` -> `examples/anchor_floats`
- [ ] `idl`
- [ ] `ido-pool`
- [ ] `interface-account`
- [ ] `lazy-account`
- [ ] `lockup`
- [ ] `misc`
- [ ] `multiple-suites`
- [ ] `multiple-suites-run-single`
- [ ] `multisig`
- [ ] `optional`
- [ ] `pda-derivation`
- [ ] `pyth`
- [x] `realloc` -> `examples/anchor_realloc` (adapted)
- [ ] `relations-derivation`
- [ ] `safety-checks`
- [ ] `spl`
- [ ] `swap`
- [x] `system-accounts` -> `examples/anchor_system_accounts` (adapted)
- [x] `sysvars` -> `examples/anchor_sysvars` (adapted)
- [ ] `test-instruction-validation`
- [ ] `tictactoe`
- [ ] `typescript`
- [ ] `validator-clone`
- [ ] `zero-copy`
