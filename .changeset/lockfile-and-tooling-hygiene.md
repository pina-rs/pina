---
pina_cli: fix
---

# Fix the IDL size message and sanitize doc topics

`pina idl fetch` rejected raw client output above 8 MiB but reported the limit as "4 MiB". The check and the message now share one constant, `MAX_RAW_HEX_BYTES`, so the two cannot drift again, and the message states the true 8 MiB limit.

`pina docs <topic>` joined the caller-supplied topic directly into `<PINA_TEMPLATES_DIR>/<topic>.t.md`, so a topic such as `../../etc/passwd` escaped the template directory. Only the file-name component of the topic is now used, and a topic with no file-name component fails with a clear error.

The unused `tar` dependency is gone from `pina_cli` and the workspace dependency table; nothing in the crate referenced it, and the only other lockfile consumer is `agave-snapshots`, which declares it directly.
