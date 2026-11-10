---
pina: feat
pina_macros: feat
pina_cli: fix
---

# Add compact creation and a program ID check opt-out

The typed creation builders gain `invoke_fresh_with` and `invoke_signed_fresh_with`; the canonical builder also gains versions that pass the derived bump to the initializer. These opt-in methods require a zero-lamport target and use `CreateAccount`. A third party funding a deterministic PDA first can block this path, so the permissive methods remain the default.

The permissive allocation path now uses SIMD-0312's `CreateAccountAllowPrefund` for both fresh and pre-funded targets. **The runtime must have SIMD-0312 activated.** It transfers only the rent deficit, allocates, and assigns in one CPI. The benchmark verifier checks partially funded, fully funded, and overfunded accounts against a real SBF runtime, including exact balances and initialized state.

`#[discriminator(entrypoint, skip_program_id_check)]` explicitly disables the declared program ID comparison in the instruction routers. Instruction and account validation remain enabled, and the reserved migration handler keeps its independent ID check. The CLI scanner accepts the new option. Programs should keep the default check unless execution under another program ID is intentional.
