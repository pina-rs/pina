---
pina: feat
pina_macros: feat
---

# Improve PDA signer and CPI builder ergonomics

Adds a stack-backed `PdaSigner` helper, generated `#[pda]` seed conversions for Pinocchio CPI signing, and a validated `Program<T>` wrapper for generated CPI builders. The Prop AMM generated-style CPI prototype now exposes Pinocchio-style builders with `.invoke()` and `.invoke_signed()` methods.
