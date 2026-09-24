---
pina_lints: breaking
pina_cli: fix
---

# Require post-CPI reloads for every token CPI destination

`require_post_cpi_balance_reload` (deny by default) now checks destinations it used to ignore. It previously inspected only destinations whose name contained `vault`, `custody`, `reserve`, or `pool`. It now also rejects a balance snapshot of any `Transfer`, `TransferChecked`, `MintTo`, or `MintToChecked` destination — `user_stake_ata`, `treasury`, `fee_receiver`, whatever it is called — that is taken before the CPI and trusted after it. Programs that passed before can therefore fail to build. The diagnostic points at the stale use.

After the CPI, a snapshot may only be used in two ways:

- as a direct operand of arithmetic, a comparison, or a `checked_*`/`saturating_*`/`wrapping_*`/`overflowing_*` method whose other operand is a reload of the destination that follows the CPI and runs on every path to the use (`after.checked_sub(before)`, `before.checked_add(after - before)`, or `before.checked_add(delta)` with `delta = after.checked_sub(before)?`); or
- in a comparison against a constant (`if prior == 0`, `before as u128 >= CAP`).

Tuples, struct fields, call arguments, and arithmetic against anything else are stale uses, as is reloading only to check the reload and then crediting `before + amount`. Uses the CPI cannot reach are accepted: the CPI is in a diverging block or a sibling branch.

Accounts are keyed by binding (never by name) plus full field path. Only these steps are looked through:

- `let` aliases, `&`, `*`, and `?`;
- Pina's token-view methods, and the token crates' `from_account_view`-style loaders;
- the `.base` field of a loaded Token-2022 view; and
- non-`&mut self` methods with constant arguments.

A `&mut self` call such as `it.next()` gets a key unique to its call site. So wrapper-typed fields, shadowed or pattern-bound locals, and successive iterator items never collapse together. Reads inside closures count for neither tier. Snapshots are followed through tuple destructuring, copies, and assignments.

Builders are recognised by their constructor's resolved return type and signature:

- The type name ends in the token instruction name (`SplTransfer` counts), and `Result<Builder, _>` is unwrapped.
- The constructor leads with reference parameters followed by an integer amount, and the decimals argument may be absent. A builder from a token crate needs three leading account parameters. One defined elsewhere needs four for a transfer (`from, mint, to, authority`), so a lamport transfer, which never names a mint, is not treated as a token transfer. Non-token types such as `AuthorityTransfer::new(config, new_authority, signer)` are ignored.
- The destination is located from the number of leading account parameters. This also brings the four-argument token-crate `Transfer::new` into the custody check.

The system program's builders are excluded by their defining crate. `invoke_with_unverified_program()` invocations are now associated with their builder. When the typed identity cannot name a custody destination or one of its reads, the custody tier falls back to the name-based check, so code it accepted is not newly rejected for that reason.

The static-`invoke()` exemption is wider in one respect. It previously matched only the non-generic spelling of the legacy builders, so the real generic `pinocchio_token` 0.7 builders invoked with `invoke()`/`invoke_signed()` into a custody account were flagged in practice. They are now exempt when the call's receiver has the full type of the constructed builder with the `pinocchio_token::TokenProgram` program parameter, because such a call can only target the legacy SPL Token program, which cannot charge a transfer fee. A wrapper's `invoke()`, an expression yielding a Token-2022 builder, and Pina's `token_2022` builder aliases stay covered.

`pina lint --explain require_post_cpi_balance_reload` and the lint reference describe the new contract.
