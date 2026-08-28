---
pina_cli_npm:
  bump: patch
  type: fix
---

# Synchronize npm platform dependency ranges during releases

Update `@pina-rs/cli` platform dependency ranges after Monochange prepares a release, then refresh the pnpm lockfile before committing the release pull request. This keeps every optional native package aligned with the launcher version and prevents release validation from accepting stale dependency ranges.
