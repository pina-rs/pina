# anchor_realloc

<br>

Secure adaptation of Anchor's account reallocation safety checks.

## What this demonstrates

<br>

- An explicit initialize → grow → shrink lifecycle for an authority-bound compact sample PDA.
- A dynamic `Vec<u64, 64>` tail that stores only active values and appears as a prefixed array in the Codama IDL.
- Automatic grow-before-edit and shrink-after-commit ordering through `ResizeCompactAccount`, including exact rent funding and refunds.
- Lower-level growth-limit, type, capacity, and element-boundary validation shared with `ReallocCompactAccount`.
- Type, owner, stored-authority, and canonical-PDA validation before every resize.
- Duplicate realloc target detection, before any mutation.

## Differences From Anchor

<br>

Anchor's original `tests/realloc` fixture creates a single global `[b"sample"]` PDA. Its purpose is to exercise framework reallocation constraints, not to demonstrate an authorization policy. This Pina adaptation is deliberately safer and is **not ABI-compatible** with the prior Pina example:

- `Initialize` (discriminator `2`) creates a canonical sample PDA at `[b"sample", authority]`. Its `bump` argument must be the canonical bump.
- `Realloc` (discriminator `0`) now requires `authority` to be writable and a signer. It may resize only that authority's initialized sample, and `len` is the total account-data length returned by `Sample::projected_bytes(values_count)`.
- `Realloc2` (discriminator `1`) is restored to the original fixture's duplicate-account regression: both authenticated sample accounts resolve to the same PDA and it returns `AccountDuplicateReallocs` without mutation. It is no longer an arbitrary two-account resize API.

The account header stores the PDA bump, authority, and active value count. The PDA derivation provides the primary binding; storing the authority makes the authorization policy auditable and detects malformed state before reallocation. The trailing values use Pina's checked compact loader and generated Codama codecs.

## Size management

Use the generated compact-size API instead of duplicating the account layout formula:

```rust
let target_len = Sample::projected_bytes(values_count)?;
let allocated_len = sample_account.data_len();
let committed_len = sample.encoded_size();
let staged_len = sample.projected_size();
```

`Sample::MIN_SIZE` is the empty-tail allocation. `projected_bytes` validates the requested value count against `VALUES_CAPACITY`. An immutable or mutable view reports its committed logical size through `encoded_size()`. After `set_values`, a mutable view reports the pending size through `projected_size()` until `commit()` writes the new tail length.

`ResizeCompactAccount::invoke` receives a callback that stages and commits the compact view. It grows the physical allocation before that callback, verifies the committed size against the exact target, and shrinks only after the mutable borrow has been dropped. Use `invoke_signed` when the rent account is a PDA. Use `ReallocCompactAccount` only when you intentionally need lower-level allocation control or spare bytes.

## Security Invariants

`Realloc` validates the following before changing data length or lamports:

1. The authority is a writable signer and the system program is canonical.
2. The sample is writable, owned by this program, non-empty, and has the `Sample` discriminator.
3. The sample address is the canonical PDA for that exact authority and its stored bump.
4. The stored authority equals the signing authority.
5. The requested length round-trips through `Sample::projected_bytes(values_count)` and does not exceed Solana's 10 KiB per-instruction growth limit.

The SBF regressions cover the normal lifecycle, a signer attempting to resize another authority's sample, an arbitrary program-owned but typed account, and the duplicate-target path. They exercise these invariants; they do not claim to prove the absence of every possible Solana attack.

## Run

<br>

```sh
cd examples/anchor_realloc
pina test --unit
pina test
pina generate
```
