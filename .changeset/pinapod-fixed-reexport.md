---
pina: fix
---

# take `fixed` from pinapod's re-export

The `fixed` feature carried a second exact pin of the `fixed` crate — `=1.30.0`, duplicated from `pinapod`'s workspace table and held in lockstep by hand, because the root manifest's comment warned that mixed `fixed` versions can never be allowed to coexist. Pinapod 0.4.4 made that duplication unnecessary: its `fixed` feature re-exports the pinned crate, and Pina's own `fixed` feature already forwarded to `pinapod/fixed`.

`pina::fixed` now forwards `pinapod::fixed`, and the `fixed`, workspace, and optional-dependency declarations are gone. The resolved crate is identical — the same 1.30.0 instance, so `pina::fixed` names the same types and every existing schema compiles unchanged — but the pin exists in exactly one place instead of two, and a future `fixed` bump is pinapod's change to make rather than a coordinated edit across both repositories.

The pinapod minimum moves to 0.4.4, the first release carrying the re-export.
