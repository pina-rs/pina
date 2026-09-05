---
pina: none
---

# Prefix seed constants with SEED_

Every example seed constant now leads with SEED_: PROFILE_SEED becomes SEED_PROFILE, STATE_SEED_PREFIX becomes SEED_STATE_PREFIX, and the same rule covers all remaining program and surfpool-test seed constants. The illustrative COUNTER_SEED constant used across crates documentation, doc comments, test fixtures, and the pina_root macro contract tests renames to SEED_COUNTER for consistency. Names only reorder; seed bytes, PDA addresses, and program behavior are unchanged.
