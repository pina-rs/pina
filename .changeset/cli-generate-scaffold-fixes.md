---
pina_cli: fix
---

# Repair cli-rust paths, TS event dirs, and the CPI pin

The `cli-rust` scaffold referenced the generated rust client three directory levels up instead of two, so the rendered CLI crate could not resolve its client dependency. The TypeScript event log module now creates the `events` directory before writing `logs.ts` and treats a missing barrel file as empty instead of failing, which lets programs without events generate cleanly. The CPI crate scaffold pins `pina = "0.17"` instead of the stale `0.12` floor.
