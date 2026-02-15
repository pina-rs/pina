---
pina: docs
---

Rewrite `readme.md` with comprehensive documentation for the pinocchio 0.10.x API:

- Updated quick start example with `AccountView` and `Address` types
- Added crate features table (`derive`, `logs`, `token`)
- Added core concepts sections: entrypoint, discriminators, accounts, instructions, events, errors, validation chains, Pod types, CPI helpers, logging
- Added full account validation assertions reference
- Updated crate table (removed `pina_token_2022_extensions`)
- Updated building for SBF and testing sections

Updated `CLAUDE.md` to reflect pinocchio 0.10.x architecture:

- Updated workspace crates list (removed `pina_token_2022_extensions`, updated dependency names)
- Updated entrypoint pattern with `Address`/`AccountView`
- Updated account validation description with `Result<&AccountView>`
- Updated package names list for changesets
