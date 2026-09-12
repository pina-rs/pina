---
pina: feat
pina_cli: fix
pina_lints: breaking
pina_skill: docs
---

# Enforce emptiness in typed creation builders

The four typed creation builders now reject targets whose storage holds any nonzero byte with `AccountAlreadyInitialized` before allocating. The deny-by-default `require_empty_before_init` lint is retired: remove it from `pina.toml` `[lints]` sections and drop manual pre-creation `assert_empty()` calls. Keep `assert_empty()` calls that guard raw `CreateAccount` allocations or token-account creation.
