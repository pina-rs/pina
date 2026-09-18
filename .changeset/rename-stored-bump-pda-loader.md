---
pina_macros: breaking
pina: breaking
pina_cli: feat
---

# Name the stored-bump compact PDA loader for what it verifies

The release that split the compact PDA loader into `with_pda` and `with_checked_pda` gave the two methods names that do not state their difference: `with_pda` kept its name while its verification was weakened from a canonical bump search to a single derivation from the account's stored bump. A program upgrading across that release lost canonical-bump rejection at every existing `with_pda` call site with no compiler signal.

The weak loader is now `with_stored_bump_pda`, which says what it checks. The old `with_pda` name remains as a deprecated forwarding alias for one release, so existing calls compile but emit a `deprecated` warning naming the difference and pointing at `with_checked_pda` for the trustless case. `with_checked_pda` is unchanged.

`pina_cli`'s IDL parser recognizes `with_stored_bump_pda` as a PDA loader alongside the other names, so account properties are unchanged.
