---
pina_lints: fix
pina_cli: fix
---

# Require post-CPI reloads for every token CPI destination

`require_post_cpi_balance_reload` no longer exempts destinations outside the `vault`/`custody`/`reserve`/`pool` name set. A balance snapshot of any `Transfer`, `TransferChecked`, `MintTo`, or `MintToChecked` destination — `user_stake_ata`, `treasury`, `fee_receiver`, whatever it is called — that is taken before the CPI and used after it now requires a reload of the destination after the CPI. The diagnostic points at the stale use. Custody-named transfer destinations keep their stricter rule: they must be read before and after every transfer even when no snapshot exists yet.

Builders are now recognised by the type their constructor returns instead of by the spelling of the call, so type aliases, re-exports, `use ... as` imports, `with_multisig_signers`, and four-argument `Transfer::new` are covered, and `invoke_with_unverified_program()` invocations are associated with their builder. The static-`invoke()` exemption now applies only when the builder's program type parameter is the legacy `pinocchio_token::TokenProgram`, so Pina's `token_2022` builder aliases invoked statically stay covered. `pina lint --explain require_post_cpi_balance_reload` and the lint reference describe the new contract.
