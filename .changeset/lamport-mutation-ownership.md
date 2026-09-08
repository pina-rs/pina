---
pina: breaking
pina_cli: breaking
pina_lints: breaking
---

# Bind lamport ownership checks to mutation

Replace `LamportTransfer::send` with `send_owned`, require a program ID in account-close methods and builders, and reject wrong-owner accounts before any balance or data mutation. Remove the lexical owner-before-send lint because the safe operation now performs the check.
