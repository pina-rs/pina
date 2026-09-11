---
pina: feat
pina_cli: patch
pina_macros: feat
---

# Reserve the framework Migrate instruction

Pina now reserves the all-ones discriminator of every instruction width for a framework migration instruction, and `#[discriminator]` rejects user variants that claim the reserved value at compile time. Programs with migratable accounts route the reserved instruction through the new `MigrateContext` runtime helper, which validates the `[payer, systemProgram, …accounts]` layout, rejects duplicated slots and foreign-owned accounts, draws every rent transfer from one instruction-wide lamport cap, and runs the same on-demand migration executor as the inline path. Machines whose instruction space is one byte wide can identify the instruction with `is_migrate_instruction`, and the migrations example exercises the whole route through its Surfpool suite, including the shared budget. The CLI migrations module is also split by concern, and `pina migrations make` now settles draft refreshes with the answers already recorded on the draft, never proposes one added field as the rename target of two removals, and fails closed when a `--rename` targets a field the change did not add.
