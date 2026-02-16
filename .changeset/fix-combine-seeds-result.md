---
pina: major
---

**BREAKING**: `combine_seeds_with_bump` now returns `Result<[Seed; MAX_SEEDS], ProgramError>` instead of `[Seed; MAX_SEEDS]`. The function previously used `assert!` which would abort the transaction on-chain with no recovery. It now returns `Err(ProgramError::InvalidSeeds)` when `seeds.len() >= MAX_SEEDS`, giving callers a graceful error path. Update call sites to handle the `Result` with `?` or pattern matching.
