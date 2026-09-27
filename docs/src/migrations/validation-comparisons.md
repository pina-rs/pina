# Migrate value rules to comparisons

Value rules in `#[pina(validate(...))]` are now comparisons over the field's `value` or its `len`, so the annotation reads as the check it generates. The `min`, `max`, `min_len`, `max_len`, and `exact_len` parameter spellings are deprecated: they still parse and generate the identical checks, but each one warns at the parameter and names its replacement.

Nothing on the wire changes. Value rules are not recorded in the ABI document or migration manifests, and the deprecated spellings generate the same checks their replacements generate, so migrating an annotation is a rename, not a release.

## Rewrite each named bound as a comparison

| Before          | After         | Check it generates                      |
| --------------- | ------------- | --------------------------------------- |
| `min = 1`       | `value >= 1`  | inclusive lower bound                   |
| `max = 10`      | `value <= 10` | inclusive upper bound                   |
| `min_len = 2`   | `len >= 2`    | inclusive minimum byte or element count |
| `max_len = 64`  | `len <= 64`   | inclusive maximum byte or element count |
| `exact_len = 4` | `len == 4`    | exact byte or element count             |

```rust
// before
#[pina(validate(min = 1, max = 1_000_000, error = TransferError::InvalidAmount))]
pub amount: u64,

// after
#[pina(validate(value >= 1 && value <= 1_000_000, error = TransferError::InvalidAmount))]
pub amount: u64,
```

Chain bounds on one receiver with `&&` and separate rules with `,`. A chain may also put the receiver in the middle, mirroring the range it checks:

```rust
#[pina(validate(100 < value <= u64::MAX))]
pub amount: u64,

#[pina(validate(4 < len <= 100))]
pub memo: String<100>,
```

Comparisons also express what the named bounds could not. Use `value != 0` for a declarative non-zero check, or `len != EXPR` to reject one specific length.

## What keeps its single `=`

Named parameters that are not comparisons keep their spelling:

- `error = ERROR` names the failure to raise, not a comparison. It stays in the validation group it overrides.
- `validate(with = function)` in the outer macro is a hook parameter and keeps `=`.
- Account rules in `#[derive(Accounts)]` — `signer`, `address = EXPR`, `owner = EXPR`, `owners = EXPR`, `program = EXPR`, `sysvar = EXPR`, `data_len = EXPR`, `distinct_from = FIELD`, `empty`, `not_empty`, `writable`, `executable` — are unchanged. Only value rules over `value` and `len` moved to comparisons.

## Read the deprecation warning

Each deprecated parameter warns at the token the author wrote, which is the ordinary `deprecated` lint: `#[allow(deprecated)]` silences it and `-D warnings` fails on it.

```text
warning: use of deprecated constant `_::PINA_DEPRECATED_VALIDATION_BOUND`: `min` is deprecated;
write the bound as a comparison, as in `value >= 1`
 --> src/lib.rs:8:22
  |
8 |     #[pina(validate(min = 1, max = 10))]
  |                      ^^^
```

The warning lowers to an empty const block, so it costs no compute units and adds no bytes to the program.

## Type gates and duplicates carry over

A comparison is subject to the same gates its named bound was: `value` rules require an integer field (`u8` through `u128`, `i8` through `i128`, or the Pina `Pod*` integer types), and `len` rules require `String<N>`, `PodString<N, PFX>`, `Vec<T, N>`, `PodVec<T, N, PFX>`, or `[u8; N]`. Duplicates and the `exact_len` conflict are diagnosed the same way, spelled for the new grammar where the rule that triggered them is new.

See [Declarative Validation](../validation.md) for the complete rule table.
