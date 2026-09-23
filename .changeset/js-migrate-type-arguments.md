---
pina_cli: fix
---

# Emit valid TypeScript for reserved Migrate type arguments

The reserved `Migrate` composer closed its `MigrateInput<...>` type argument list with a trailing comma. That is a `TS1009` syntax error — trailing commas are legal in type parameter _declarations_ but not in type argument _uses_ — so raw `pina generate` output could not be parsed by `tsc`, `esbuild`, or any tool stricter than dprint's formatter. Repository pipelines only survived because `verify-codama-idls.sh` runs `dprint fmt` over the generated tree, silently deleting the comma before anything parsed the file. A consumer running `pina generate` without that reformatting step got uncompilable clients.

The generator now emits the comma-free shape dprint normalizes to, so committed clients are unchanged and raw output is valid TypeScript. Caught by the migrations walkthrough, which loads the freshly generated client through esbuild without a formatting pass.
