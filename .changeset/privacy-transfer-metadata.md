---
pina: none
pina_cli: none
---

# Bind privacy-pool transfer disclosure metadata

The privacy-pool example now authenticates the successor view key, stored envelope length and content, and encrypted custodian shares in the transfer proof. Transfer proving and verifying keys must be regenerated; account and instruction layouts remain unchanged.

Run the compiled-program metadata-substitution regression in the existing runtime CI task so proof binding remains continuously checked.

Remove unnecessary tail returns in the core account validation paths so feature-specific Clippy checks pass without changing their behavior.

Prevent the deployment stdin-isolation test from blocking on unused stdout before the Solana child starts, preserving its EOF and process-success assertions.
