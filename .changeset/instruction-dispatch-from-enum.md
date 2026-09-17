---
pina: feat
pina_cli: feat
pina_macros: feat
---

# Generate the instruction entrypoint from the discriminator enum

`#[discriminator(entrypoint)]` turns the instruction enum into the program entrypoint, so a program stops hand-writing the discriminator → accounts → `process` match, the account-count constant that goes with it, and the reserved `Migrate` ladder. Variant `Foo` routes to `FooAccounts` unless a `#[dispatch(accounts = BarAccounts)]` override says otherwise.

Everything is generated as associated items on the enum, so nothing is added to the surrounding module and no free function can collide with the program's own items:

```rust
#[discriminator(entrypoint)]
pub enum CounterInstruction {
	Initialize = 0,
	Increment = 1,
}

nostd_entrypoint!(CounterInstruction::process_instruction);
```

`CounterInstruction::MAX_INSTRUCTION_ACCOUNTS` is derived rather than set by hand: it is the largest `ACCOUNT_BOUND` across the routed accounts structs, saturated at `pinocchio::MAX_TX_ACCOUNTS`. A `#[cfg(test)]` assertion block makes the constant verify itself, replacing the contract test each consumer used to write. Keep the entrypoint's default account array — sizing it to the constant would make the loader skip extra accounts instead of letting `finish_exact` reject them. The generated entrypoint is `#[inline(always)]` like the dispatchers it replaces; pass `inline = "hint"` for the plain `#[inline]` spelling when that is what the program was measured with, because the two are not equivalent at the codegen level.

At most one enum per program may opt in; a module-level marker turns a second one into a compile error. Custom entrypoint behavior stays available: a program that needs its own wiring can call `CounterInstruction::process_instruction` from wherever it prefers, or skip the flag entirely.

`#[derive(Accounts)]` now reports `ParseAccounts::ACCOUNT_BOUND`: one slot per positional field, one for a `#[pina(remaining)]` slice, and the nested struct's bound folded in. Hand-written parsers keep the `UNBOUNDED` sentinel so they can never understate what a program reads.

`migrations(Account, ...)` plus `migrations_max_lamports = EXPR` emits `Self::process_migrate`, which routes the reserved all-ones `Migrate` instruction over the declared slot order. The order becomes a typed declaration instead of a comment beside a run of `run_optional` calls, and a contract named in it must appear in the checked-in manifest — a program with no manifest fails with the same `pina migrations make` remedy as a schema that opts into migrations. The reserved path validates the configured program id before migrating anything, so a mismatched id still returns `IncorrectProgramId`.

`pina idl` reads the annotation directly, because the generated match is invisible to a source parser. For the four converted examples the generated IDL is byte-identical to the checked-in fixtures, and all 24 tracked example programs build to byte-identical SBF binaries.
