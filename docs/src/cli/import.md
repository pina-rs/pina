# Import a Foreign Program

Call another on-chain program from your own without hand-writing its wire format. `pina import` fetches a foreign program's IDL, renders a standalone `no_std` CPI crate, and records what the crate was generated from.

## Synopsis

```text
pina import <NAME> --program-id <PUBKEY> [OPTIONS]
```

| Input             | Default        | Meaning                                              |
| ----------------- | -------------- | ---------------------------------------------------- |
| `<NAME>`          | required       | Crate name; written to `clients/cpi/<NAME>`.         |
| `--program-id`    | required       | Program ID the crate targets.                        |
| `--idl <FILE>`    | none           | Read the IDL from a local file.                      |
| `--url <URL>`     | none           | Fetch the IDL over HTTP(S).                          |
| `--cluster`       | `mainnet-beta` | Fetch the on-chain canonical IDL for this cluster.   |
| `--output <DIR>`  | `clients/cpi`  | Directory to write the crate into.                   |
| `--mode <MODE>`   | `auto`         | `auto`, `create`, `update`, or complete `overwrite`. |
| `--npx <COMMAND>` | `npx`          | Runner for Anchor IDL conversion.                    |

Give one of `--idl`, `--url`, or `--cluster`. Passing `--idl` with `--url` is rejected rather than silently preferring one.

```bash
# From a vendored IDL
pina import switchboard \
  --program-id SBondMDrcV3K4kxZR1HNVT7osZxAHVHgYXL5Ze1oMUv \
  --idl ./idls/on_demand.json

# From a URL
pina import metaplex \
  --program-id metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s \
  --url https://example.com/token_metadata.json

# From the program's published on-chain IDL
pina import squads \
  --program-id SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf \
  --cluster mainnet-beta
```

## What the generated crate contains

Each instruction becomes a call struct holding its accounts in the target program's own order plus a typed `ix` field for the arguments:

```rust,ignore
RandomnessReveal {
    randomness,
    oracle,
    queue,
    ix: RandomnessRevealIx { signature, recovery_id, value },
}
.invoke_signed(&[signer])?;
```

Each account in the IDL also gets a read-only parser:

```rust,ignore
use switchboard_cpi::accounts::randomness_account_data::RandomnessAccountData;

let state = RandomnessAccountData::parse(randomness.try_borrow()?.as_ref())
    .ok_or(ProgramError::InvalidAccountData)?;
if state.reveal_slot == clock.slot {
    let value = state.value;
}
```

The parser carries the account's discriminator as a public constant, its encoded size as `LEN` (or `MAX_LEN` when the layout varies), a `matches` guard, and a `parse` that returns `None` on a short buffer or a foreign discriminator.

An account whose layout has a variable-width field still gets its struct, discriminator, and size constant, but no `parse`: a variable-width field leaves every field after it with no fixed offset, so a partial parser would read the wrong bytes. The generated crate records why in a `PARSER_UNSUPPORTED` constant so the omission is visible at the call site.

## Provenance

`pina import` stamps the generated README with the IDL's SHA-256, where it came from, and the generator version:

| Field       | Value                                         |
| ----------- | --------------------------------------------- |
| Program ID  | `SBondMDrcV3K4kxZR1HNVT7osZxAHVHgYXL5Ze1oMUv` |
| IDL source  | file `./idls/on_demand.json`                  |
| IDL SHA-256 | `336e8714...`                                 |
| Generator   | `pina_cpi_renderer` 0.18.0                    |

The digest is the contract a reviewer checks. A CPI crate is a copy of another program's interface, so without a recorded digest a reviewed crate can be regenerated from a different IDL with nothing in the diff to show it.

Re-running the same import against an unchanged IDL reports `already up to
date` and rewrites nothing, so an import checked into CI produces no drift.

## Binding the program ID

The target program address is compiled into the crate as an `Address` constant, and `is_expected_program` compares an address against it. Use it when the address arrives from caller input:

```rust,ignore
if !switchboard_cpi::is_expected_program(oracle_program.address()) {
    return Err(ProgramError::IncorrectProgramId);
}
```

The generated crate also ships a unit test that binds the compiled-in constant to the address spelled in the IDL. A swapped dependency could otherwise retarget every CPI in the crate without the source changing, so the expected address is asserted in full rather than only through the constant.

## Supported argument shapes

Anchor routes most non-primitive arguments through `definedTypes`. The renderer resolves those links and declares the structs and enums it needs as Rust types with their own encoders, so nesting stays recursive instead of unrolling into the instruction body.

Supported:

- little-endian integers `u8`-`u128` and `i8`-`i128`, booleans, and public keys
- fixed-width byte arrays
- length-prefixed strings, byte slices, arrays, and maps
- `Option`, including Anchor's variable-length form
- structs, tuples, and enums whose variants carry equally sized payloads
- structs and enums referenced through `definedTypeLinkNode`

Rejected, with the reason in the error message:

| Shape           | Why                                                                                  |
| --------------- | ------------------------------------------------------------------------------------ |
| `shortU16`      | Anchor encodes it as a 1-3 byte variable-length prefix reserved for account lengths. |
| `f32` / `f64`   | A `no_std` crate has no float ABI conversion, so the writer would silently disagree. |
| Bare `string`   | Without a length prefix a reader cannot tell where the value ends.                   |
| Bare `bytes`    | Same; wrap it in `sizePrefixTypeNode` or `fixedSizeTypeNode`.                        |
| Big-endian ints | Solana instruction data is little-endian.                                            |
| Uneven enums    | Variants with different payload widths have no single instruction-data layout.       |

## Optional accounts

Anchor's `programId` strategy — where an absent optional account becomes the program ID — is supported directly.

The `omitted` strategy, where an absent account is dropped from the list, is supported only when every optional account is trailing. A fixed-size CPI account array cannot express a hole in the middle of a list, so a mid-list optional account fails with the instruction named rather than silently shifting the accounts after it.

## Verifying an import

The same gate that renders every checked-in fixture compiles the result for `bpfel-unknown-none`:

```bash
./scripts/verify-cpi-fixtures-sbf.sh
```

Checked-in fixtures cover Switchboard On-Demand randomness, Metaplex Token Metadata, Meteora DLMM, and Squads v4 multisig. The Switchboard fixture pins its instruction discriminators, account counts, and encoded instruction lengths against the hand-written reference crate in `pina-rs/lootbox`, and they match exactly.
