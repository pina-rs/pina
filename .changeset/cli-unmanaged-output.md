---
pina_cli: fix
---

# Preserve unmanaged client output

Refuse to replace nonempty generated output directories when their tracking manifest is missing or invalid. Auto and update generation no longer delete unmanaged TypeScript CLI output, and the shared publisher applies the same protection to TypeScript and Dart client sources.
