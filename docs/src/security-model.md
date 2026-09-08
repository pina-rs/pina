# Security Model

Pina's safety posture is built around explicit validation and predictable state transitions.

The [Security Lints](./security-lints.md) page maps these invariants to the compile-time checks applied to every repository example and secure security fixture.

## Core invariants

- Type correctness: account bytes must match expected discriminator and layout.
- Authority correctness: signer/owner checks must precede mutation.
- PDA correctness: seed and bump checks must gate PDA-bound operations.
- Value correctness: arithmetic and balance mutations must be checked.

See [ADR 0001](./adrs/0001-discriminator-first-layout.md), [ADR 0002](./adrs/0002-zero-copy-account-model.md), and [ADR 0003](./adrs/0003-guard-backed-typed-account-loaders.md) for the durable rationale behind these invariants.

## Version-safe binary layout and compatibility

The discriminator-first model makes byte layout part of protocol compatibility. Treat every `#[account]` struct as ABI:

- Do not reorder fields.
- Do not change existing discriminator values.
- Do not alter field types in-place without migration.
- If a struct grows, treat it as a new versioned shape and migrate state explicitly.

<!-- {=pinaDiscriminatorVersionCompatibility} -->

## Discriminator and payload versioning

| Change                                      | Compatibility impact                                               |
| ------------------------------------------- | ------------------------------------------------------------------ |
| Add a new enum variant                      | Usually backward-compatible if old clients ignore unknown variants |
| Change an existing variant value            | **Breaking** for every historical byte slice                       |
| Reorder or remove struct fields             | **Breaking** (offsets change)                                      |
| Append fields to a struct                   | Mostly non-breaking, but consumers must accept the larger size     |
| Switch primitive width (`u8` → `u16`, etc.) | **Breaking** for serialized payloads at that boundary              |

For on-chain accounts, treat layout as part of protocol ABI:

- Keep field order stable.
- Introduce optional `version` fields at the tail for in-place migration strategies.
- Never change existing discriminator values in place.
- When incompatible layout changes are required, perform explicit migration with a new account version and an operator upgrade flow.

For instruction payloads:

- Prefer additive migration: add a new variant and keep legacy handlers for a release cycle.
- Reject stale payload shapes with explicit errors rather than silently reinterpreting bytes.

<!-- {/pinaDiscriminatorVersionCompatibility} -->

## High-priority guardrails

- Prefer checked arithmetic (`checked_add`, `checked_sub`) for all user-facing or balance-affecting values.
- Ensure all token account types used by helper traits implement `AccountValidation`.
- Keep close/transfer helpers conservation-safe (no temporary double-crediting).

## Closing accounts safely

<!-- {=pinaCloseAccountGuidance} -->

Closing guidance under Pinocchio 0.11:

- `close_with_recipient(&ID, recipient)` verifies that `ID` owns the account, transfers its lamports, and closes the account handle. It does not zero or resize account data.
- When stale bytes must be invalidated, use `CloseAccountZeroed { account, recipient, program_id: &ID }.invoke()` or manually call `zeroed()` before `close_with_recipient(&ID, recipient)`.
- The `account-resize` feature only affects realloc helpers; it does not change close semantics.

<!-- {/pinaCloseAccountGuidance} -->

## Best practices

<!-- {=pinaSecurityBestPractices} -->

- **Always call `assert_signer()`** before trusting authority accounts
- **Use Pina's token loaders directly** because they delegate canonical owner and layout validation to the corresponding checked upstream parser before returning typed state
- **Use `as_associated_token_account()`** when reading a canonical ATA because it validates the runtime owner, derived address, stored current authority, and stored mint together; enforce state, delegate, close-authority, and extension policy separately
- **Always call `assert_empty()`** before account initialization to prevent reinitialization attacks
- **Use `invoke_with` or `invoke_signed_with`** when fixed-account creation must establish nonzero values before final PinaPod validation
- **Use generated `load_pda` or `load_pda_mut`** when a fixed stored-bump PDA handler needs a typed guard, so recursive content and the PDA address are validated once
- **Use generated `with_pda`** when a compact stored-bump PDA handler needs a compact view, so the layout, canonical bump, and PDA address are validated during the same borrow
- **Always verify program accounts** with `assert_address()` / `assert_program()` before CPI invocations
- **Use `assert_type::<T>()`** to prevent type cosplay: it checks discriminator, owner, and data size
- **Use `send_owned(&ID, amount, recipient)`** for direct lamport debits; it verifies that the program owns the sender before mutation
- **Use `CloseAccountZeroed { account, recipient, program_id: &ID }.invoke()` or `zeroed()` + `close_with_recipient(&ID, recipient)`** when stale account bytes must be invalidated before close
- **Prefer `assert_seeds()` / `assert_canonical_bump()`** over `assert_seeds_with_bump()` to enforce canonical PDA bumps
- **Give each account type its own seed namespace** so PDAs cannot collide across account types

<!-- {/pinaSecurityBestPractices} -->

## Content validation with PinaPod

Pina's zero-copy account model is built on PinaPod. Its generated storage view makes validation load-bearing: `PinaAccount::try_from_bytes` and `as_account` reject noncanonical booleans, invalid UTF-8, overlength vector prefixes, invalid option tags, invalid active nested values, and invalid enum discriminants before returning a reference.

The `#[account]` macro uses the native struct only as a schema and derives `PinaPod`. For `Account`, PinaPod generates `AccountZc`; loaders return that companion, not a reference to the native schema. `PinaAccount::validate_account_data` checks the discriminator, exact size, and every fixed field. Compact loaders also check every tail offset and active length.

### Unit enums

Unit enums with explicit discriminants can derive `PinaPod`. PinaPod generates an `EnumZc` companion that stores raw bytes and validates the discriminant before converting it to the native enum. Pina's audited `#[account]` grammar does not accept arbitrary custom enums, so this form applies to direct PinaPod schemas and advanced manual `PinaAccount` implementations.

### Inactive capacity

PinaPod initializes the full capacity of fixed strings, vectors, and options. Shortening or clearing a value also zeroes the removed payload, so stale application data does not remain in inactive capacity. Pina still exposes validated field accessors rather than a byte slice over an in-memory schema or storage view.

Compact patches clear bytes removed by a tail replacement. `UpdateResizableAccount` validates the complete patch before moving rent or changing account bytes. A failed preflight leaves both data and lamports unchanged.

## Testing strategy

- Unit tests for negative validation cases.
- Regression tests for every previously fixed bug class.
- Integration tests for cross-account invariants where mutation order matters.

These framework guarantees do not validate an application's economic design. Use the [Production Readiness](./production-readiness.md) gate before deploying an asset-bearing program.
