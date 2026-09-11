---
pina: feat
pina_macros: feat
---

# Reserve the framework Migrate instruction

Pina now reserves the all-ones discriminator of every instruction width for a framework migration instruction, and `#[discriminator]` rejects user variants that claim the reserved value at compile time. Programs with migratable accounts route the reserved instruction through the new `MigrateContext` runtime helper, which validates the `[payer, systemProgram, …accounts]` layout, rejects duplicated slots and foreign-owned accounts, and runs the same on-demand migration executor as the inline path with the program's lamport cap. Machines whose instruction space is one byte wide can identify the instruction with `is_migrate_instruction`, and the migrations example exercises the whole route through its Surfpool suite.
