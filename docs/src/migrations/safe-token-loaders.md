# Migrate to safe token loaders

Pina token loaders now validate canonical program ownership before they parse account data. The ATA loader also validates the derived address and the wallet and mint stored in token-account data. This change removes the separate checked method names and the lints that required preceding assertions.

## Remove checked suffixes

Replace each removed method with its shorter equivalent:

| Before                                     | After                              |
| ------------------------------------------ | ---------------------------------- |
| `as_token_mint_checked()`                  | `as_token_mint()`                  |
| `as_token_account_checked()`               | `as_token_account()`               |
| `as_token_2022_mint_checked()`             | `as_token_2022_mint()`             |
| `as_token_2022_account_checked()`          | `as_token_2022_account()`          |
| `as_associated_token_account_checked(...)` | `as_associated_token_account(...)` |

The replacement methods enforce the same owner checks. They preserve guard-backed lifetimes and return the same typed views as the removed methods.

## Remove duplicate assertions

Delete owner and ATA assertions that exist only to guard an immediate loader call:

```rust
// Before
account.assert_owner(&token::ID)?;
let account = account.as_token_account()?;

vault.assert_associated_token_address(wallet, mint, token_program)?;
let vault = vault.as_associated_token_account(wallet, mint, token_program)?;

// After
let account = account.as_token_account()?;
let vault = vault.as_associated_token_account(wallet, mint, token_program)?;
```

Keep `assert_owner()`, `assert_owners()`, and `assert_associated_token_address()` in validation-only paths that do not read token data.

## Pick the loader by program policy

Use the unqualified legacy loaders when the instruction requires the original SPL Token program:

```rust
let mint = mint_account.as_token_mint()?;
let token_account = token_account.as_token_account()?;
```

Use the explicit Token-2022 loaders when the instruction requires Token-2022:

```rust
let mint = mint_account.as_token_2022_mint()?;
let token_account = token_account.as_token_2022_account()?;
```

Use `*_for_program()` when the instruction accepts either canonical token program:

```rust
token_program.assert_addresses(&[token::ID, token_2022::ID])?;
let program_id = *token_program.address();
let mint = mint_account
	.as_token_mint_for_program(&program_id)?
	.assert_no_extensions()?;
let account = token_account.as_token_account_for_program(&program_id)?;
```

The dynamic loaders reject unsupported program IDs and require each account's runtime owner to equal `program_id`.

## Update lint configuration

Remove these retired lint names from `[lints]` in `pina.toml`:

```toml
require_owner_before_token_cast = "deny"
require_associated_token_address_before_ata_cast = "deny"
```

Pina no longer needs lexical checks for these conditions because the loaders enforce them at runtime. `require_consistent_token_program` and `require_explicit_token_2022_extension_policy` remain active.

## Check ATA assumptions

`as_associated_token_account()` now rejects a token account when its stored wallet or mint differs from the values used to derive the ATA address. This catches an ATA whose token authority was reassigned, as well as malformed or spoofed account data. If a program intentionally accepts a token account with a different current authority, load it with `as_token_account_for_program()` and validate the program-specific relationship explicitly.
