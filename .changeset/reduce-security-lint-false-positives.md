---
pina: fix
---

# Reduce false positives in security lints

Make the security lint suite recognize semantically equivalent safe patterns: resolved account-borrow and CPI methods, dominating constant bounds, identifier-based asset arithmetic, immutable token-program aliases, direct token CPI builders, and canonical legacy Token Program exemptions. Opaque wrappers remain conservative, while the expanded UI fixtures document each accepted and rejected pattern.
