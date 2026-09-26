# `privacy_pool_program`

<br>

A tiered-disclosure privacy pool: a shielded-pool payments program whose distinguishing feature is its disclosure protocol.

<br>

## What this demonstrates

<br>

- **Runtime-crypto integration.** Poseidon commitments go through the `sol_poseidon` syscall and Groth16 proofs verify through the `sol_alt_bn128_group_op` pairing selector, both behind a contained, heavily documented syscall boundary. A full spend — tree read, nullifier scan, four-pairing Groth16 check, payout — fits in one transaction at a fraction of the compute budget.
- **Generated CLI surface.** The committed TypeScript, Dart, and Rust CLIs derive fixed-seed PDA addresses when `--address` is omitted, and payer-resolution accounts always resolve to the loaded payer keypair — the only key these CLIs can sign with.
- **Real end-to-end zero knowledge.** The `prover` feature carries the host-side toolkit: an in-circuit Poseidon gadget built from the same circom x5 parameter tables the syscall implements, a spend circuit, a seeded trusted setup, and the big-endian wire codec. The tests generate real Groth16 proofs and push them through the SBF artifact — no mocks.
- **The tiered disclosure state machine.** Three tiers — consent, verified (challenge window), and compelled — with a custodian quorum, an authority-resolvable challenge path, and an append-only public log whose entries are written by the same instruction that executes a disclosure.
- **Heap opt-in.** `nostd_entrypoint_alloc!` installs the bump allocator; the verifier's scratch buffers are heap-backed while all account state stays fixed-layout.
- **The full migration envelope.** Every account is enrolled, with machine-checked ABI layouts in `tests/abi_layout.rs`.

<br>

## The pool

<br>

| Instruction           | Description                                                                                    |
| --------------------- | ---------------------------------------------------------------------------------------------- |
| `Initialize`          | Creates the config, vault, tree, registries, and log; prefills the zero-hash chain.            |
| `SetVerificationKey`  | Installs a Groth16 verifying key for the withdraw or transfer circuit slot.                    |
| `SetCustodians`       | Rotates the three-member disclosure committee.                                                 |
| `RegisterRequester`   | Grants an entity a maximum disclosure tier.                                                    |
| `Deposit`             | Escrows one denomination of SOL and inserts the client-computed commitment.                    |
| `Withdraw`            | Verifies a Groth16 spend proof, records the nullifier, pays out.                               |
| `Transfer`            | Spends a note into a fresh commitment with a new consent key.                                  |
| `RequestDisclosure`   | Files a tiered request with an encrypted notice and legal-basis hash.                          |
| `GrantDisclosure`     | The note's view key consents (tier 0).                                                         |
| `ChallengeDisclosure` | The view key freezes a tier-1 request during its window.                                       |
| `ResolveChallenge`    | The authority resolves a challenge.                                                            |
| `ApproveDisclosure`   | A custodian approves; at quorum the request executes and the log entry is appended atomically. |
| `CancelDisclosure`    | The requester withdraws a pending request.                                                     |

A note is `(secret, nullifier_seed)`; the commitment is `Poseidon(Poseidon(secret, seed), amount)` and the nullifier is `Poseidon(seed, secret)`. The circuit proves knowledge of the note's preimage, membership against the tree root, and the nullifier derivation — without revealing which leaf. Deposits and withdrawals remain public edges by design (fixed denomination); the nullifier set is the only spent record — note accounts never carry a spent flag, because linking a nullifier to its commitment is exactly the leak the pool exists to prevent.

<br>

## The disclosure tiers

<br>

- **Tier 0 — consent.** Anyone may request. The note's per-deposit _view key_ must grant before the committee may execute.
- **Tier 1 — verified.** Registered requesters. The subject is notified through an encrypted notice blob and holds a challenge window; once it lapses without a challenge (or a challenge is resolved in the requester's favor), the committee may execute.
- **Tier 2 — compelled.** Registered requesters with a recorded legal basis. No window; the committee may execute immediately.

The invariant the program enforces — the reason this is a protocol rather than a policy document — is that reaching the custodian quorum _is_ the log entry: `ApproveDisclosure` flips the request to executed and appends to the append-only `DisclosureLog` in the same instruction. Disclosure without a permanent public record is impossible by construction. Key shares travel off-chain, so no key material ever becomes public; no single custodian can open an envelope alone.

<br>

## What the tests prove

<br>

- `tests/e2e.rs` runs the compiled SBF artifact through mollusk: the on-chain Poseidon root matches the host-predicted root byte for byte, real Groth16 withdrawals and transfers pay out and re-key notes, tampered proofs, unknown roots, and double spends are rejected, and the full tier lifecycle (consent gating, windows, challenges, resolutions, registry entitlement, duplicate-approval rejection, cancellation) holds.
- `tests/surfpool/src/lib.rs` runs the same journeys on a real runtime through `pina test`, with fixed seeds so the recorded instruction paths stay deterministic for the benchmark harness.
- The in-crate unit tests cover the tree, nullifier set, root ring, log encoding, and — through the `prover` feature — circuit satisfaction and a host mirror of the on-chain verifier that accepts real proofs and rejects flipped public inputs.

<br>

## Honest limits (this is a teaching scaffold, not a production system)

<br>

- **Custodians can collude off-chain.** The protocol makes silent disclosure impossible, not collusion itself.
- **No relayer.** The submitting wallet pays fees and funds successor notes, so transfers are anonymous with respect to the spent note, not to fees.
- **Deposit/withdrawal correlation.** Fixed denomination blunts but does not eliminate edge analysis; production pools add denomination ladders and delays.
- **Capacity.** 128 leaves and 128 nullifiers keep accounts under the 10,240-byte inner-CPI creation ceiling; production trees shard levels across child accounts and hash-index their nullifiers.
- **Demo-grade share encryption.** The test harness encrypts custodian shares with a fixed scheme; production uses X25519 + AEAD or threshold ElGamal.
- **Public log names its target.** The log records the commitment directly for clarity; a production design stores a salted scope root plus a private notice so the log itself cannot deanonymize.
- **Single authority.** Verifying keys, custodians, and the challenge resolution all hang off one governance key; production distributes these.
- **Fixed bootstrap initializer.** Every pool account is a singleton PDA, so `Initialize` is a one-time capture of the governance root: only the committed `BOOTSTRAP_AUTHORITY` may run it (a front-running first signer would otherwise become the permanent authority). The example's bootstrap key is a committed fixture; a real deployment generates its bootstrap authority off-circuit, commits only the public key, and initializes in the same ceremony that deploys the program. There is deliberately no authority-transfer path — if a future version adds one, give it the two-step accept/revoke discipline a custody rotation needs.
