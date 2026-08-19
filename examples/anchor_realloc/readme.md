# anchor_realloc

<br>

Secure adaptation of Anchor's account reallocation safety checks.

## What this demonstrates

<br>

- An explicit initialize → grow → shrink lifecycle for an authority-bound sample PDA.
- Reallocation growth-limit and rent-exemption enforcement through `realloc_account`.
- Type, owner, stored-authority, and canonical-PDA validation before every resize.
- Duplicate realloc target detection, before any mutation.

## Differences From Anchor

<br>

Anchor's original `tests/realloc` fixture creates a single global `[b"sample"]` PDA. Its purpose is to exercise framework reallocation constraints, not to demonstrate an authorization policy. This Pina adaptation is deliberately safer and is **not ABI-compatible** with the prior Pina example:

- `Initialize` (discriminator `2`) creates a canonical sample PDA at `[b"sample", authority]`. Its `bump` argument must be the canonical bump.
- `Realloc` (discriminator `0`) now requires `authority` to be writable and a signer. It may resize only that authority's initialized sample, and `len` is the total account-data length, including the 34-byte `Sample` header.
- `Realloc2` (discriminator `1`) is restored to the original fixture's duplicate-account regression: both authenticated sample accounts resolve to the same PDA and it returns `AccountDuplicateReallocs` without mutation. It is no longer an arbitrary two-account resize API.

The new account header stores both the PDA bump and the authority. The PDA derivation provides the primary binding; storing the authority makes the authorization policy auditable and detects malformed state before reallocation. Trailing resized bytes are intentionally not exposed by this example as a serialization format.

## Security Invariants

`Realloc` validates the following before changing data length or lamports:

1. The authority is a writable signer and the system program is canonical.
2. The sample is writable, owned by this program, non-empty, and has the `Sample` discriminator.
3. The sample address is the canonical PDA for that exact authority and its stored bump.
4. The stored authority equals the signing authority.
5. The requested length preserves the header and does not exceed Solana's 10 KiB per-instruction growth limit.

The SBF regressions cover the normal lifecycle, a signer attempting to resize another authority's sample, an arbitrary program-owned but typed account, and the duplicate-target path. They exercise these invariants; they do not claim to prove the absence of every possible Solana attack.

## Run

<br>

```sh
cargo test -p anchor_realloc
pina idl --path examples/anchor_realloc --output codama/idls/anchor_realloc.json
```
