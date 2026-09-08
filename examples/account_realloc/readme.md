# account_realloc

<br>

Secure adaptation of Anchor's account reallocation safety checks.

## What this demonstrates

<br>

- An explicit initialize → grow → shrink lifecycle for an authority-bound compact sample PDA.
- A dynamic `Vec<u64, 64>` tail that stores only active values and appears as a prefixed array in the Codama IDL.
- Automatic grow-before-update and shrink-after-update ordering through `UpdateResizableAccount`, including exact rent funding and refunds.
- Lower-level growth-limit, type, capacity, and element-boundary validation shared with `ReallocCompactAccount`.
- One complete compact load plus stored-authority and canonical-PDA validation before every resize, without repeating the same compact parse.
- Duplicate realloc target detection, before any mutation.

## Differences from Anchor

<br>

Anchor's original `tests/realloc` fixture creates a single global `[b"sample"]` PDA. Its purpose is to exercise framework reallocation constraints, not to demonstrate an authorization policy. This Pina adaptation is deliberately safer and is **not ABI-compatible** with the prior Pina example:

- `Initialize` (discriminator `2`) creates a canonical sample PDA at `[b"sample", authority]`. Its `bump` argument must be the canonical bump.
- `Realloc` (discriminator `0`) now requires `authority` to be writable and a signer. It can update only that authority's initialized sample.
- `Realloc2` (discriminator `1`) is restored to the original fixture's duplicate-account regression: both authenticated sample accounts resolve to the same PDA and it returns `AccountDuplicateReallocs` without mutation. It is no longer an arbitrary two-account resize API.

The account header stores the PDA bump, authority, and active value count. The PDA derivation provides the primary binding; storing the authority makes the authorization policy auditable and detects malformed state before reallocation. The trailing values use Pina's checked compact loader and generated Codama codecs.

## Size management

Use a generated patch instead of duplicating the account layout formula:

```rust
let patch = SamplePatch::new().replace_values(&values);
let target_len = Sample::updated_len(current_data, &patch)?;
```

`Sample::MIN_SIZE` is the empty-tail allocation. `updated_len` validates the replacement against the declared capacity and calculates its encoded size.

`UpdateResizableAccount` validates the patch before changing bytes or lamports. It grows before applying a longer value and applies a shorter value before shrinking. It skips reallocation when `target_len` equals the current allocation. Use `invoke_signed` when the rent account is a PDA. Use `ReallocCompactAccount` only when the caller needs lower-level allocation control or spare bytes.

## Security invariants

`Realloc` validates the following before changing data length or lamports:

1. The authority is a writable signer and the system program is canonical.
2. The sample is writable, owned by this program, non-empty, and has the `Sample` discriminator.
3. The sample address is the canonical PDA for that exact authority and its stored bump.
4. The stored authority equals the signing authority.
5. The generated patch validates the requested values and does not exceed Solana's 10 KiB per-instruction growth limit.

The SBF regressions cover the normal lifecycle, a signer attempting to resize another authority's sample, an arbitrary program-owned but typed account, and the duplicate-target path. They exercise these invariants; they do not claim to prove the absence of every possible Solana attack.

## Run

<br>

```sh
cd examples/account_realloc
pina test --unit
pina test
pina generate
```
