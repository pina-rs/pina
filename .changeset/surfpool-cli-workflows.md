---
pina_cli:
  bump: minor
  type: feat
pina_test:
  bump: minor
  type: feat
pina_cli_npm:
  bump: none
  type: none
  caused_by: [pina_cli]
pina_skill:
  bump: minor
  type: feat
---

# Add layered Surfpool testing and development workflows

Add `pina test` with two explicit testing layers: `--unit` preserves the fast native Rust and Mollusk loop, while the default builds the real SBF artifact and runs a generated integration test against an isolated, offline Surfpool SDK instance. Missing test targets and SBF artifacts fail instead of being skipped.

Add `pina dev` as a thin foreground delegation to Surfpool's artifact watcher and redeployment workflow. Development runs offline unless a cluster or RPC URL is selected explicitly. New projects use the focused `pina_test` support crate in a dedicated host-only test package, include a real SBF test that deploys at a non-system address and executes the starter instruction with panic-safe teardown, and document the separate roles of native, Mollusk, and Surfpool testing. `pina_test` owns the pinned Surfpool 1.5 compatibility graph, while the test package's workspace boundary keeps it separate from the program's Mollusk and SBF dependency graphs.

Keep the isolated host dependency accountable with locked check, test, Clippy, documentation, coverage, license, and RustSec gates. The repository's Surfpool workflow continuously creates a fresh project and proves its generated starter instruction against a real SBF artifact. Surfpool prereleases do not satisfy the CLI version floor, and version diagnostics retain at most 4 KiB while escaping terminal controls. `pina dev` requires explicit `--yes` consent before Surfpool creates its first `txtx.yml` deployment runbook.
