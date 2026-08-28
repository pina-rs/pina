---
pina: feat
---

# Add the security lint suite

Add seven program-security lints covering canonical PDA bumps, account-data borrows across CPIs, token-program consistency, explicit Token-2022 extension policies, post-CPI custody balance reloads, checked asset arithmetic, and bounded remaining-account iteration.

Token mint and account views now also expose no-allocation helpers for rejecting all Token-2022 extensions or allowing an explicit extension set. Every security lint is documented with its threat model, compliant usage, scope, and known limitations, and the workspace examples pass the complete lint suite.
