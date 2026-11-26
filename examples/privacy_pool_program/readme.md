# `privacy_pool_program`

<br>

A programmable, tiered-disclosure privacy pool: a multi-asset UTXO shielded-pool payments program combining Solana Privacy Rings' production architecture with a governed, publicly-logged disclosure state machine.

<br>

## What this demonstrates

<br>

- **Per-nullifier PDAs (`NullifierRecord`).** Each spent nullifier is stored in its own 3-byte PDA rather than a fixed-capacity array — giving $O(1)$ double-spend checks, unbounded spend capacity, and zero shared nullifier write-lock contention. Dummy inputs (`nullifier == [0; 32]`) skip PDA creation.
- **Depth-16 frontier Merkle tree (`MerkleTree`).** Holds 65,536 leaves in a 512-byte frontier plus a 32-root ring using compile-time `ZERO_HASHES`, eliminating runtime zero-hash recomputation on `Initialize` and cutting account size by more than 5× over a full-node store.
- **Unified variable-amount 2-in-2-out join-split circuit.** One Groth16 circuit (`VK_SLOT_JOIN_SPLIT`) handles arbitrary-amount withdrawals, change outputs, UTXO splits, and UTXO merges with 64-bit range checks and single-input `statement_hash` compression (one `g1_mul` + `g1_add` on-chain).
- **SPL Token & Token-2022 escrows (`AssetVault`).** Permissionless per-mint `RegisterAsset`, variable-amount `DepositToken`, and join-split `WithdrawToken` via `TransferChecked` CPI.
- **192-byte compressed Groth16 proofs.** Compressed proofs (`A: 32B, B: 128B, C: 32B`) decompress G₁ on-chain via `sol_alt_bn128_compression` before the four-pairing `sol_alt_bn128_group_op` check.
- **Relayer & fee-payer decoupling.** Optional `relayer` and `relayer_fee` are bound into the Groth16 statement so users can pay transaction fees from shielded balances without leaking a public fee payer.
- **Real shielded keypair & X25519 + AEAD (`shielded` module).** Host-side RFC 7748 X25519 viewing keys, authenticated note envelopes, per-transaction viewing keys for selective disclosure, and 2-of-3 GF(256) Shamir custodian shares.
- **Rent-free note emission & privacy-preserving `DisclosureLog`.** Encrypted envelopes and custodian shares emit through `NoteEncryptedEvent` (zero per-note PDA rent); `RequestDisclosure` authenticates `view_pubkey` via Merkle membership; `DisclosureLog` records a salted `scope_hash` rather than raw commitments; `SetVerificationKey` supports authority key rotation.
- **Custom Ring policy & composable CPI.** Configurable pause and co-signer / CPI authority gates (`SetPolicy`) plus composable ZK policy proof verification (`VerifyPolicy` on `VK_SLOT_POLICY`).

<br>

## The pool

<br>

| Instruction           | Description                                                                                             |
| --------------------- | ------------------------------------------------------------------------------------------------------- |
| `Initialize`          | Creates config, SOL vault, frontier tree, registries, and log using compile-time `ZERO_HASHES`.         |
| `SetVerificationKey`  | Installs or rotates a single-input Groth16 verifying key for `VK_SLOT_JOIN_SPLIT` or `VK_SLOT_POLICY`.  |
| `SetCustodians`       | Rotates the three-member disclosure committee.                                                          |
| `RegisterRequester`   | Grants an entity a maximum disclosure tier.                                                             |
| `Deposit`             | Escrows a variable amount of SOL, updates the frontier tree, and emits `NoteEncryptedEvent`.            |
| `Withdraw`            | Verifies a 2-in-2-out join-split proof, nullifies inputs, pays recipient + relayer, emits change note.  |
| `Transfer`            | Spends 1–2 UTXOs into 1–2 UTXOs with optional relayer fee and emits `NoteEncryptedEvent` per output.    |
| `RequestDisclosure`   | Authenticates `(commitment, view_pubkey)` via Merkle proof and files a tiered disclosure request.       |
| `GrantDisclosure`     | The note's authenticated view key consents (tier 0).                                                    |
| `ChallengeDisclosure` | The note's authenticated view key freezes a tier-1 request during its challenge window.                 |
| `ResolveChallenge`    | The authority resolves a challenged request.                                                            |
| `ApproveDisclosure`   | A custodian approves; at quorum the request executes and appends a salted `scope_hash` log entry.       |
| `CancelDisclosure`    | The requester withdraws a pending request.                                                              |
| `RegisterAsset`       | Permissionlessly registers an SPL Token or Token-2022 mint and its program-controlled escrow account.   |
| `DepositToken`        | Escrows a variable amount of SPL/Token-2022 tokens, updates the frontier tree, emits encrypted note.    |
| `WithdrawToken`       | Verifies a join-split proof for `asset_id(mint)`, pays recipient + relayer in tokens, emits change.     |
| `SetPolicy`           | Configures Custom Ring policy flags (`POLICY_PAUSED`, `POLICY_REQUIRE_COSIGNER`) and co-signer address. |
| `VerifyPolicy`        | Verifies a 128-byte compressed Custom Ring ZK policy proof on `VK_SLOT_POLICY` (composable via CPI).    |

A note is `(secret, nullifier_seed, amount, asset_id, view_pubkey)`. Its commitment is `Poseidon(Poseidon(secret, nullifier_seed), Poseidon(asset_id, amount))`, its tree leaf is `Poseidon(commitment, address_hash(view_pubkey))`, and its nullifier is `Poseidon(nullifier_seed, secret)` (or `0` for dummy inputs).

All join-split public inputs are compressed into a single BN254 scalar `statement_hash = Poseidon(Poseidon(root, Poseidon(n0, n1)), Poseidon(Poseidon(c0, c1), Poseidon(Poseidon(ext_data_hash, asset_id), amounts_packed)))`, where `amounts_packed = public_in + public_out * 2^64 + relayer_fee * 2^128` and `ext_data_hash` is a 248-bit SHA-256 digest over `(recipient, relayer, mint, out0_meta, out1_meta)`. On-chain Groth16 verification therefore requires only a single `g1_mul` and `g1_add` before the four-pairing check.

<br>

## The disclosure tiers

<br>

- **Tier 0 — consent.** Anyone may request. The note's authenticated _view key_ must grant before the committee may execute.
- **Tier 1 — verified.** Registered requesters. The subject is notified through an encrypted notice blob and holds a challenge window; once it lapses without a challenge (or a challenge is resolved in the requester's favor), the committee may execute.
- **Tier 2 — compelled.** Registered requesters with a recorded legal basis. No window; the committee may execute immediately.

Reaching the custodian quorum _is_ the log entry: `ApproveDisclosure` flips the request to `STATUS_EXECUTED` and appends a salted `scope_hash = SHA256("pina:privacy-pool:disclosure-scope:v2" || commitment || requester || nonce)` to `DisclosureLog` in the same instruction. Silent disclosure is impossible by construction, and the public log does not expose raw note commitments.

<br>

## What the tests prove

<br>

- `tests/e2e.rs` runs the compiled SBF artifact through Mollusk: frontier tree roots match host predictions, real 192-byte compressed Groth16 proofs verify on-chain for SOL and SPL withdrawals, 2-in-2-out transfers with relayer fees, Custom Ring policy enforcement and ZK policy proofs, and the full tiered disclosure lifecycle.
- `tests/surfpool/src/lib.rs` runs every instruction discriminator on a real runtime through `pina test` with deterministic seeds for the benchmark harness.
- In-crate unit tests verify the compile-time `ZERO_HASHES` table, frontier tree insertion, root ring eviction, salted disclosure scope hashes, RFC 7748 X25519 test vectors, AEAD note envelopes, per-transaction viewing keys, 2-of-3 Shamir share recovery, and compressed Groth16 proving/verification.

<br>

## Honest limits (reference scaffold)

<br>

- **Seeded test setup.** Tests use deterministic `seeded_setup_*`; production deployments require a multi-party trusted setup ceremony.
- **Custodian collusion.** The protocol makes silent disclosure impossible, not off-chain collusion among a threshold of custodians.
- **Single governance authority.** Verifying-key rotation, custodians, policy flags, and challenge resolution hang off one `PoolConfig::authority`; production deployments should assign a multisig PDA.
