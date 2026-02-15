---
pina_macros: major
---

Update generated code for pinocchio 0.10.x compatibility:

- **`AccountInfo` → `AccountView`** — the `#[derive(Accounts)]` macro now generates `&'a AccountView` references and `TryFromAccountInfos` implementations using `AccountView` instead of `AccountInfo`.
- **`TryFrom` impl updated** — the blanket `TryFrom<&[AccountView]>` implementation delegates to `TryFromAccountInfos` with the new type.
- **Doc examples updated** — all documentation examples now reference `Address` instead of `Pubkey` for struct fields.
