---
pina: none
---

# Bind privacy-pool withdrawals to their payout recipient

Include a Poseidon hash of the complete recipient address in the withdrawal proof's public inputs, preventing copied proofs from redirecting payouts. Update the circuit, key installation checks, and proof fixtures; test recipient substitution against real proofs and the SBF program. Preserve transfer disclosure binding and Boolean Merkle path constraints. Withdrawal proving and verifying keys must be regenerated for a new deployment; existing funded pools require a separately audited migration. Instruction bytes and account layouts stay unchanged.
