---
pina_cli: fix
---

# Read accounts from hand-written versioned dispatch

`pina idl`, `pina generate`, and `pina migrations create` described an instruction as taking no accounts when a hand-written dispatcher routed it through the generated `process_versioned`, the form the migration guide gives for an instruction that keeps its envelope:

```rust,ignore
WalkInstruction::Update => UpdateInstruction::process_versioned(
	UpdateAccounts::try_from((program_id, accounts))?,
	data,
),
```

The extractor read the accounts struct only from the receiver of `.process(data)`. Every generated client then sent the instruction with no accounts, which the program rejects with `NotEnoughAccountKeys`, and `pina migrations create` recorded the empty account list as the instruction's process, which a published history refuses to correct.

The extractor now reads the `Accounts::try_from((program_id, accounts))` conversion wherever an arm performs it, including when the parsed accounts are bound to a local first. An arm that converts into two different structs has no single account layout and is still emitted without accounts. Programs routed by `#[discriminator(entrypoint)]` were not affected.

A program that already recorded an instruction this way should run `pina migrations create` after upgrading: an unpublished history is re-recorded with its accounts.
