# Anchor Test Porting

This page tracks sequential parity ports from `solana-foundation/anchor/tests` into `examples/`, using Rust-first tests (mollusk/native unit tests) instead of TypeScript.

## Port Status

- [ ] `anchor-cli-account` (no direct parity yet; Anchor CLI account decoding over dynamic `Vec`/`String` data is not a direct pina/no-std match)
- [ ] `anchor-cli-idl` (no direct parity yet; Anchor CLI IDL account lifecycle is Anchor-CLI-specific)
- [ ] `auction-house`
- [x] `bench` -> `examples/prop_amm_program` (adapted from `bench/programs/prop-amm/anchor-v2`; asm fast path intentionally not ported)
- [ ] `bpf-upgradeable-state`
- [ ] `cashiers-check`
- [ ] `cfo`
- [ ] `chat`
- [ ] `composite`
- [ ] `cpi-returns`
- [ ] `custom-coder`
- [ ] `custom-discriminator`
- [ ] `custom-program`
- [x] `declare-id` -> `examples/declare_id`
- [x] `declare-program` -> `examples/declare_program` (adapted)
- [x] `duplicate-mutable-accounts` -> `examples/duplicate_mutable_accounts` (adapted)
- [x] `errors` -> `examples/custom_errors` (adapted)
- [x] `escrow` -> `examples/escrow_program` (adapted with parity-focused tests)
- [x] `events` -> `examples/events` (adapted event schema parity)
- [x] `floats` -> `examples/float_accounts`
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
- [x] `realloc` -> `examples/account_realloc` (security-adapted: per-authority PDA instead of Anchor's global test fixture)
- [ ] `relations-derivation`
- [ ] `safety-checks`
- [ ] `spl`
- [ ] `swap`
- [x] `system-accounts` -> `examples/system_accounts` (adapted)
- [x] `sysvars` -> `examples/sysvar_checks` (adapted)
- [ ] `test-instruction-validation`
- [ ] `tictactoe`
- [ ] `typescript`
- [ ] `validator-clone`
- [ ] `zero-copy`
