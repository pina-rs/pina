---
# `.github/workflows/performance.yml` belongs to `pina_root`, which is
# unpublished, so the CI-only change is recorded without a bump.
pina_root: none
---

# Skip the performance comment for cancelled runs

The `PR performance comment` job ran under `always()`, so when concurrency cancelled an older run for a newer push, the job still started, found no `performance-*` artifacts, and failed writing its comment into a directory the download never created. That left a spurious failing check on the pull request until the newer run finished. The job now runs under `!cancelled()`, which still posts a partial report when a benchmark job fails, and the assemble step creates its output directory so a run with no reports posts every section as unavailable instead of failing.
