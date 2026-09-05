---
pina: none
pina_lints: fix
---

# Prefix seed constants with SEED_

Every example seed constant now leads with SEED_: PROFILE_SEED becomes SEED_PROFILE, STATE_SEED_PREFIX becomes SEED_STATE_PREFIX, and the same rule covers all remaining program, security-example, and surfpool-test seed constants. The illustrative COUNTER_SEED, CONFIG_SEED, STATE_SEED, MY_SEED, and VAULT_SEED constants used across crates documentation, doc comments, templates, test fixtures, and the pina_root macro contract tests rename to the SEED_ prefix form for consistency. The require_explicit_discriminators_and_seed_namespaces lint now also recognizes SEED_-prefixed seed constants so assertions using the preferred naming stay clean; names only reorder and seed bytes, PDA addresses, and program behavior are unchanged.
