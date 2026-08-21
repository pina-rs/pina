---
pina_cli: fix
---

# Make IDL extraction fail closed

Reject incomplete or ambiguous program source graphs instead of silently emitting partial Codama metadata. IDL generation now resolves explicit module paths, distinguishes missing conditional modules from missing required files, validates package names and entrypoint ownership, and requires PDA attributes and inferred PDA account links to resolve exactly.
