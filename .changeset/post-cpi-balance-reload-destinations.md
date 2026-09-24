---
pina_lints: breaking
pina_cli: fix
---

# Require post-CPI reloads for every token CPI destination

`require_post_cpi_balance_reload` (deny by default) now checks destinations it used to ignore. It previously inspected only destinations whose name contained `vault`, `custody`, `reserve`, or `pool`. It now also rejects a balance snapshot of any `Transfer`, `TransferChecked`, `MintTo`, or `MintToChecked` destination — `user_stake_ata`, `treasury`, `fee_receiver`, whatever it is called — that is taken before the CPI and used after it without a reload. Programs that passed before can therefore fail to build. The diagnostic points at the stale use.

The new check accepts a reload only when it:

- comes after the CPI and before the use;
- has its value used; and
- runs on every path to the use.

It recognises Token-2022 reads through `.base.amount()` and `Type::amount(account)`, and follows snapshots through tuple destructuring, copies, and assignments. Comparing a snapshot against a constant, a snapshot used only before the CPI, and a use the CPI cannot reach (a diverging block or a sibling branch) are accepted.

Builders are recognised by the type their constructor returns. The name only needs to end in the token instruction name (`SplTransfer`). The constructor may return `Result<Builder, _>`, and the decimals argument may be absent. The destination is located by counting leading account arguments, which also brings four-argument `Transfer::new` into the custody check. The system program's lamport `Transfer` is excluded by its defining crate. `invoke_with_unverified_program()` invocations are now associated with their builder. The custody tier's reads now resolve through `let` aliases and Token-2022 `.base` projections too.

The static-`invoke()` exemption is wider in one respect. It previously matched only the non-generic spelling of the legacy builders, so the real generic `pinocchio_token` 0.7 builders invoked with `invoke()`/`invoke_signed()` into a custody account were flagged in practice. They are now exempt when the builder's program type parameter is `pinocchio_token::TokenProgram`, because that call can only target the legacy SPL Token program, which cannot charge a transfer fee. Pina's `token_2022` builder aliases, which bind the same structs to `Token2022Program`, stay covered.

`pina lint --explain require_post_cpi_balance_reload` and the lint reference describe the new contract.
