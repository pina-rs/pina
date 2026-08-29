---
pina_cli: fix
---

# Pin security lints to a durable main revision

Use the immutable squash commit containing the reviewed Dylint 6 lint suite so generated projects and `pina lint` fetch a revision that remains reachable from `main`.
