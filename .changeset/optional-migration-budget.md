---
pina: feat
pina_macros: feat
pina_cli: feat
pina_skill: docs
---

# Make the migration lamport budget optional

`migrations_max_lamports` no longer has to be declared to wire the reserved `Migrate` route. A program opts in with the entrypoint alone:

```rust
#[discriminator(entrypoint)]
pub enum ProgramInstruction {
	// …
}
```

The budget becomes a deliberate tightening rather than a required incantation. `MigrateAccount` and `MigrateContext` take `Option<u64>`; `None` declares no ceiling and the executor enforces none, which is safe because a transfer is never more than the rent deficit of a growth the runtime already caps at `MAX_PERMITTED_DATA_INCREASE`. Declaring a budget now means "refuse a migration costlier than this", not "permit rent at all".

The reserved route calls the `account-resize` executor, so a program that serves migrations needs that feature. `pina init` now scaffolds `pina = { …, features = ["account-resize", "logs", "derive"] }`, and the generated code references a constant named `ACCOUNT_RESIZE_FEATURE_REQUIRED_FOR_MIGRATE_ROUTE`, so enabling the route without the feature reports the feature to enable instead of an unresolved `MigrateContext`.

The `migrations(A, B)` list remains an optional override, still the only way to batch several accounts of one contract in a sweep.

`MigrateAccount::max_lamports` and `MigrateContext::new` change from `u64` to `Option<u64>`, so downstream callers wrap the value they pass.
