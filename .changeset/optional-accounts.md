---
pina: major
pina_cli: major
pina_macros: major
---

# Add optional accounts with a fixed program-address filler

`#[derive(Accounts)]` now supports `Option<&'a AccountView>` and `Option<&'a mut AccountView>` fields. Account counts stay fixed: generated Codama clients fill an omitted optional slot with a readonly meta pointing at the executing program's address, and on-chain parsing maps any slot holding the program address back to `None`.

Breaking: `TryFromAccountInfos::try_from_account_infos` and the derived `TryFrom<(&Address, &mut [AccountView])>` now take the executing program id so optional slots can detect the filler sentinel. Entrypoint dispatch changes from `Accounts::try_from(accounts)?` to `Accounts::try_from((program_id, accounts))?`. The IDL pipeline emits `isOptional: true` plus the `programId` optional-account strategy for optional slots, and validation-chain analysis now attributes assertions written against `if let Some(alias)` bindings back to their originating fields.
