---
pina_cli: fix
---

# Discover workspace boundaries without executing Git

Use filesystem repository and worktree markers to discover workspace roots, so Anchor conversion cannot execute a project-local Git binary before isolating its package runner.
