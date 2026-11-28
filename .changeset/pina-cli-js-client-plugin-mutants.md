---
pina_cli: fix
---

# Fix JS program plugin brace offset and mutant coverage

Fix an off-by-one slice offset in `patch_js_program_plugin` that swallowed the character immediately following `instructions: {`, and add focused unit tests covering single- and double-quoted generated imports, retained instructions, empty instruction maps, and repeat-call idempotency.
