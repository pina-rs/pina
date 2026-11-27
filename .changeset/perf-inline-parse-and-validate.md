---
pina: fix
pina_macros: fix
---

# Inline route parsing, validation, and opt-mut aliasing

Fixes binary-size and compute-unit regressions introduced in `0.25.0` for programs that share `#[derive(Accounts)]` structs across multiple `dispatch_entrypoint!` variants, use `Option<&mut AccountView>` fields, validate program accounts with `#[pina(validate(program = ...))]`, or use `dispatch_entrypoint!` with shared account walks:

- Always inline derived account parsing along `dispatch_entrypoint!` routes (`__parse_accounts_inlined`) so `AccountsCursor::from_verified` (`check_duplicates = false`) and exact route lengths constant-fold even when one `#[derive(Accounts)]` struct is shared by multiple variants, while leaving `TryFromAccountInfos::try_from_account_infos` (`nostd_entrypoint!`) codegen unchanged.
- Always inline generated `PinaValidate::validate` implementations for `#[derive(Accounts)]` structs and factor program-account checks through `program_matches` behind `validate_program` so repeated program checks can be outlined cleanly without pushing `validate` out of line.
- Add `validate_opt_mut_aliasing_64` for mutable-only layouts (indices `< 64`, no `#[pina(unique)]`, no trailing mutable `#[pina(remaining)]`) with `Option<&mut AccountView>` slots, skip inline 3/4-way and compact unique-<=4 alias unrolling inside already-cold `dispatch_entrypoint!` duplicate branches (`COLD_ROUTE`), and use `wrapping_add(1)` for slice-bounded loop indices across aliasing validators to eliminate `overflow-checks = true` panic metadata.
- Parameterize `entry::walk_accounts` over its return encoding so `walk_accounts_shared` returns a bare `u8` status register without packing/unpacking `Result<u8, ProgramError>` while inlined (`SHARED = false`) walks still return `Result` directly.

## User impact

Deployed SBF binaries are smaller and instructions consume fewer compute units across `dispatch_entrypoint!`, optional-mutable, and validated-accounts programs. All account, signer, owner, writable, PDA, program-ID, and duplicate-account checks — including writability-before-duplicate error precedence and `program_id` optional-sentinel semantics — are unchanged.
