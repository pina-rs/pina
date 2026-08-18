---
pina: fix
pina_cli: docs
---

# Correct the CPI introspection guarantee

Expose `assert_current_instruction_program_id` for the guarantee the Instructions sysvar can actually provide. Deprecate the misleading `assert_no_cpi` name because transaction-level instruction metadata cannot detect self-CPI.
