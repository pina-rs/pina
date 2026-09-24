---
pina_lints: breaking
pina_cli: fix
---

# Require post-CPI reloads for every token CPI destination

`require_post_cpi_balance_reload` (deny by default) now checks destinations it used to ignore. It previously inspected only destinations whose name contained `vault`, `custody`, `reserve`, or `pool`. It now also rejects a balance snapshot of any `Transfer`, `TransferChecked`, `MintTo`, or `MintToChecked` destination — `user_stake_ata`, `treasury`, `fee_receiver`, whatever it is called — that is taken before the CPI and trusted after it. Programs that passed before can therefore fail to build. The diagnostic points at the stale use.

After the CPI, a snapshot may only be used in two ways:

- in the same statement as a reload of the destination that follows the CPI and runs on every path to that statement (`after.checked_sub(before)`, `before.checked_add(after - before)`); or
- in a comparison against a constant (`if prior == 0`, `before as u128 >= CAP`).

Reloading only to check the reload and then crediting `before + amount` is still flagged. Uses the CPI cannot reach are accepted: the CPI is in a diverging block or a sibling branch.

Accounts are keyed by root binding plus full field path, looking through only `let` aliases, `&`, `*`, `?`, Pina's token-view loaders, the `.base` field of a loaded Token-2022 view, and type-preserving methods with constant arguments. So wrapper-typed fields such as `ctx.user_ata` and `ctx.fee_ata` never collapse together, and `accounts.get(2)` differs from `accounts.get(3)`. Reads inside closures count for neither tier. Snapshots are followed through tuple destructuring, copies, and assignments.

Builders are recognised by their constructor's resolved return type and signature:

- The type name ends in the token instruction name (`SplTransfer` counts). `Result<Builder, _>` is unwrapped.
- The constructor has at least three leading parameters that are references to a struct or generic type, followed by an integer amount. The decimals argument may be absent. Non-token types such as `AuthorityTransfer::new(config, new_authority, signer)` are ignored.
- The destination is located from the number of leading reference parameters. This also brings four-argument `Transfer::new` into the custody check.

The system program's lamport `Transfer` is excluded by its defining crate. `invoke_with_unverified_program()` invocations are now associated with their builder.

The static-`invoke()` exemption is wider in one respect. It previously matched only the non-generic spelling of the legacy builders, so the real generic `pinocchio_token` 0.7 builders invoked with `invoke()`/`invoke_signed()` into a custody account were flagged in practice. They are now exempt when that call is made on the builder itself and the builder's program type parameter is `pinocchio_token::TokenProgram`, because such a call can only target the legacy SPL Token program, which cannot charge a transfer fee. A wrapper's `invoke()` and Pina's `token_2022` builder aliases, which bind the same structs to `Token2022Program`, stay covered.

`pina lint --explain require_post_cpi_balance_reload` and the lint reference describe the new contract.
