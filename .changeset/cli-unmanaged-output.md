---
pina_cli: fix
---

# Preserve unmanaged client output

Refuse to replace nonempty generated output directories when their tracking manifest is missing or invalid. Auto and update generation no longer delete unmanaged TypeScript CLI output, and the shared publisher applies the same protection to TypeScript and Dart client sources. Dart CLI generation also checks ownership of its entrypoints and shared helper files before publishing.

The registry audit also records an approved, documented test stack exception for RUSTSEC-2026-0387 in the existing Solana/Surfpool dependency chain. Upstream replacement and removal of the exception are tracked in <https://github.com/pina-rs/pina/issues/654>.
