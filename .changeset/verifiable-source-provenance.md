---
pina_cli: fix
---

Build verifiable source snapshots from the recorded Git revision using a private index and worktree, so concurrent repository changes cannot substitute different build inputs.

Clear inherited Git repository overrides before inspecting or staging the requested project.
