---
pina: feat
pina_cli: feat
pina_macros: feat
---

# Generate instruction dispatch from the discriminator enum

`#[instruction_dispatch]` turns the instruction enum into the program entrypoint, so a program stops hand-writing the discriminator → accounts → `process` match, the account-count constant that goes with it, and the reserved `Migrate` ladder. Apply it above `#[discriminator]`; variant `Foo` routes to `FooAccounts` unless a `#[dispatch(accounts = BarAccounts)]` override says otherwise. The generated `process_instruction` is `#[inline(always)]` like the dispatchers it replaces; pass `inline = "hint"` for the plain `#[inline]` spelling when that is what the program was measured with, because the two are not equivalent at the codegen level.

`MAX_INSTRUCTION_ACCOUNTS` is derived rather than set by hand: it is the largest `ACCOUNT_BOUND` across the routed accounts structs, saturated at `pinocchio::MAX_TX_ACCOUNTS`. A `#[cfg(test)]` assertion block makes the constant verify itself, replacing the contract test each consumer used to write. Keep the entrypoint's default account array — sizing it to the constant would make the loader skip extra accounts instead of letting `finish_exact` reject them.

`#[derive(Accounts)]` now reports `ParseAccounts::ACCOUNT_BOUND`: one slot per positional field, one for a `#[pina(remaining)]` slice, and the nested struct's bound folded in. Hand-written parsers keep the `UNBOUNDED` sentinel so they can never understate what a program reads.

`migrations(Account, ...)` plus `migrations_max_lamports = EXPR` emits the reserved `Migrate` prelude, so the slot order is a typed declaration instead of a comment beside a run of `run_optional` calls. A contract named in the ladder must appear in the checked-in manifest, and a program with no manifest fails with the same `pina migrations make` remedy as a schema that opts into migrations.

`pina idl` reads the annotation directly, because the generated match is invisible to a source parser. For the four converted examples the generated IDL is byte-identical to the checked-in fixtures, and all 24 tracked example programs build to byte-identical SBF binaries.
