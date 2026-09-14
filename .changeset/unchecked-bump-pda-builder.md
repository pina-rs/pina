---
pina: feat
pina_lints: docs
---

# Add a cheap PDA builder that skips the bump search

`CreateProgramAccountWithUncheckedBump` creates a PDA-backed account by checking one derivation: that the supplied bump derives exactly the account's address. `CreateProgramAccountWithBump` instead proves the bump is canonical by searching for the highest valid bump, which costs roughly 9,200 additional compute units per creation — the measured counter `initialize` falls from 10,719 to 3,263 CU on the new builder.

The trade-off is deliberate and visible in the name. A non-canonical bump derives a second valid address for the same seed namespace, so canonical derivation elsewhere will not find the account. That is harmless for a per-authority counter and wrong for a vault another program derives by seed alone, which is why the canonical builder keeps its contract and the security examples keep using it. The `require_canonical_bump_before_pda_write` lint documents which builder proves what, and the examples now use the unchecked builder.
