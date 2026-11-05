---
pina_codama_renderer_cli: fix
---

# Validate CLI IDL identifiers and output paths

Reject unsafe or reserved identifiers before deleting or writing generated TypeScript and Dart CLIs. Refuse escaped output paths and existing symbolic links, and keep program-address replacement patterns inside string literals.
