# Migrate to safe token loaders

Pina now exposes one clearly checked name for each token loader. The previous unsuffixed loaders were already owner-safe because the pinned `pinocchio-token` and `pinocchio-token-2022` account-view parsers validate ownership and layout together. Pina continues to delegate to those parsers without a duplicate owner comparison. The ATA loader additionally validates the derived address and the current authority and mint stored in token-account data. This change removes the redundant checked method names and the lints that required preceding assertions.

## Remove checked suffixes

Replace each removed method with its shorter equivalent:

| Before                                     | After                              |
| ------------------------------------------ | ---------------------------------- |
| `as_token_mint_checked()`                  | `as_token_mint()`                  |
| `as_token_account_checked()`               | `as_token_account()`               |
| `as_token_2022_mint_checked()`             | `as_token_2022_mint()`             |
| `as_token_2022_account_checked()`          | `as_token_2022_account()`          |
| `as_associated_token_account_checked(...)` | `as_associated_token_account(...)` |

The replacement methods enforce the same owner checks through the same checked upstream parsers. They preserve guard-backed lifetimes and return the same typed views as the removed methods.

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

Pina no longer needs lexical checks for these conditions because the loader boundary enforces them at runtime. `require_consistent_token_program` and `require_explicit_token_2022_extension_policy` remain active.

## Check ATA assumptions

`as_associated_token_account()` is a canonical ATA loader. It rejects the address before parsing when it is not the ATA derived from `wallet`, `mint`, and `token_program`. It then rejects a token account when its stored current authority or mint differs from those inputs. A token account whose authority was reassigned remains at its original associated address, but it is no longer canonical for the original wallet under this loader. If a program intentionally accepts that state, load it with `as_token_account_for_program()` and validate the program-specific relationship explicitly.

The loader does not require the account to be initialized or unfrozen, and it does not restrict delegates, close authority, or Token-2022 extensions. Apply those protocol-specific checks after loading. For Token-2022, use `assert_extensions_allowed()` or `assert_no_extensions()` where the protocol has an extension policy.

## Check error-code expectations

The checked upstream parsers preserve their own precise errors. In particular, the legacy and Token-2022 token-account parsers currently report a wrong owner as `InvalidAccountData`, while their mint parsers and Token-2022 extension-aware wrapper report `InvalidAccountOwner`. Code should normally propagate these errors rather than branching on them. Tests that asserted an exact error from a removed `*_checked` wrapper may need to be updated.
