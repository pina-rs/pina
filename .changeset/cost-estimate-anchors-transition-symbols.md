---
pina_cli: fix
---

# Anchor transition symbols and report zero-cost migrations

`pina migrations status` estimates a ladder's compute units by summing the transition functions recorded in an SBF profile. The symbol match accepted a transition name appearing anywhere in a mangled symbol, so an unrelated function whose name merely contained it was counted too, doubling the estimate. A transition that genuinely cost zero units was also reported as missing, replacing a real measurement with an "unavailable" reason.

Matches now anchor to whole path components in both mangled and demangled spellings, and a present transition is tracked separately from its cost, so a zero-unit transition reports `0` instead of a missing-symbol error.
