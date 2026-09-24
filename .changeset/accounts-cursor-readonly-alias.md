---
pina: fix
---

# Compare cursor aliases by account identity

`AccountsCursor` alias checks now compare `AccountView`s instead of addresses. The entrypoint deserializer makes every duplicate slot a copy of the original view, so a single pointer comparison replaces the writable-flag load and 32-byte address comparison that `next_mut`, `next_mut_opt`, and `remaining_mut_distinct` performed for each slot they scanned. Across the example suite, 59 of 100 measured instructions got cheaper and none got more expensive, saving 969 compute units in total. Examples: privacy pool `initialize` went from 49,373 to 49,245, staking `deposit` from 24,927 to 24,874, and escrow `make` from 37,073 to 37,035.

The cursor documentation now states the alias rules, and tests pin them. Checks look forward: a mutable account must not reappear in a later slot. A readonly slot followed by a mutable slot for the same account is accepted, because an authority that signs readonly and also pays is one account that the runtime marks writable in both slots. When two fields must be distinct accounts, compare their addresses explicitly.
