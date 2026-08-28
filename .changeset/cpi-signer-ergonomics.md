---
pina: feat
pina_cli: feat
pina_macros: feat
---

# Improve PDA signer and CPI builder ergonomics

Adds a stack-backed `PdaSigner` helper, generated `#[pda]` seed conversions for Pinocchio CPI signing, and a validated `Program<T>` wrapper for generated CPI builders. The Prop AMM generated-style CPI prototype now exposes Pinocchio-style builders with `.invoke()` and `.invoke_signed()` methods behind an opt-in `cpi` feature.

Generated `pina init` programs now declare `default = []`, `bpf-entrypoint = []`, and `cpi = []`, and include a starter `src/cpi.rs` module that is only compiled when consumers explicitly enable `features = ["cpi"]`.
