---
pina: breaking
pina_cli: fix
pina_lints: fix
pina_skill: fix
---

# Consolidate canonical PDA creation

Make typed explicit-bump creation builders enforce canonical bumps and reuse a single derivation for validation and signing. Add bump-aware fixed initializers and compact patch factories, preserve generated-client PDA resolution, and rename the low-level noncanonical allocator so its relaxed guarantee is visible at the call site.

The canonical-bump lint now directs creation code to the checked builders while retaining its validation-only protection for `assert_seeds_with_bump`.
