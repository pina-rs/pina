---
pina_lints: breaking
pina_cli: fix
---

# Require post-CPI reloads for every token CPI destination

`require_post_cpi_balance_reload` (deny by default) now checks destinations it used to ignore. It previously inspected only destinations whose name contained `vault`, `custody`, `reserve`, or `pool`. It now also rejects a balance snapshot of any `Transfer`, `TransferChecked`, `MintTo`, or `MintToChecked` destination — `user_stake_ata`, `treasury`, `fee_receiver`, whatever it is called — that is taken before the CPI and trusted after it. Programs that passed before can therefore fail to build. The diagnostic points at the stale use.

After the CPI, a _snapshot-derived_ value is a pre-CPI read of the destination, or any local, conversion (`as`, and integer `From`/`Into`/`TryFrom`/`TryInto`), or arithmetic result computed from one. It may appear only as:

1. one side of a comparison whose other side is a post-CPI reload (or a value derived from one) or a constant (`if after != before + 10`, `if prior == 0`);
2. the subtrahend of a subtraction-like operation (`-`, `checked_sub`, `saturating_sub`, `wrapping_sub`, `overflowing_sub`, in method or `u64::checked_sub(a, b)` form) whose minuend is a reload, or either side of `abs_diff`, which yields a delta; or
3. an operand of an addition-like operation whose other operand is such a delta (`before + (after - before)`, or `before.checked_add(delta)`).

The reload must follow the CPI and run on every path to the use. Every other appearance is a stale use:

- a call argument, a return value, a store, a tuple, a struct field, or an array element;
- an addition with a bare reload; and
- arithmetic that cancels the reload out (`before + after * 0`).

Uses the CPI cannot reach are accepted: the CPI is in a diverging block or a sibling branch.

Accounts are keyed by binding (never by name) plus full field path. Only these steps are looked through:

- `let` aliases, `&`, `*`, and `?`;
- Pina's token-view methods, and the token crates' `from_account_view`-style loaders;
- the `.base` field of a loaded Token-2022 view; and
- `Option`/`Result` pass-through adaptors, and Pina's `assert_*` checks.

Cursor methods (`Iterator::{next, nth}`, `DoubleEndedIterator::{next_back, nth_back}`, Pina's `AccountsCursor::next*`) get a key unique to their call site. Every other method with constant arguments, `&mut self` accessors included, is keyed by receiver, resolved method, and arguments. A `let` binding initialized from any `&mut self` method call, such as a hand-written cursor's `take()`, names its own account. So wrapper-typed fields, shadowed or pattern-bound locals, and successive iterator or cursor items never collapse together, while `ctx.vault_mut()` names one account on every call. Reads inside closures count for neither tier. Snapshots are followed through tuple destructuring, copies, and assignments.

Builders are recognised by their constructor's resolved return type and signature:

- The type name ends in the token instruction name (`SplTransfer` counts), and `Result<Builder, _>` is unwrapped.
- The constructor leads with reference parameters followed by an integer amount, and the decimals argument may be absent. A builder from a token crate needs three leading account parameters. One defined elsewhere needs four for a transfer (`from, mint, to, authority`), so a lamport transfer, which never names a mint, is not treated as a token transfer. Non-token types such as `AuthorityTransfer::new(config, new_authority, signer)` are ignored.
- The destination is located from the number of leading account parameters. This also brings the four-argument token-crate `Transfer::new` into the custody check.

The system program's builders are excluded by their defining crate. `invoke_with_unverified_program()` invocations are now associated with their builder. When the typed identity cannot name a custody destination or one of its reads, the custody tier falls back to the name-based check, so code it accepted is not newly rejected for that reason.

The static-`invoke()` exemption is wider in one respect. It previously matched only the non-generic spelling of the legacy builders, so the real generic `pinocchio_token` 0.7 builders invoked with `invoke()`/`invoke_signed()` into a custody account were flagged in practice. They are now exempt when the call's receiver has the full type of the constructed builder with the `pinocchio_token::TokenProgram` program parameter, because such a call can only target the legacy SPL Token program, which cannot charge a transfer fee. A wrapper's `invoke()`, an expression yielding a Token-2022 builder, and Pina's `token_2022` builder aliases stay covered.

`pina lint --explain require_post_cpi_balance_reload` and the lint reference describe the new contract.
