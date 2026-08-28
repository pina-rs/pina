---
pina_cli:
  bump: none
  type: none
---

# Stabilize generated shell fixtures on Linux CI

Execute dynamic test bodies through a stable checked-in shell driver instead of asking the kernel to execute files immediately after writing them. This removes overlay-filesystem `ETXTBSY` races from coverage runs without changing production behavior.
