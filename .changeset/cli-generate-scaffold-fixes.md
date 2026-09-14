---
pina_cli: fix
---

# Repair cli-rust paths, TS event dirs, and the CPI pin

The `cli-rust` scaffold referenced the generated rust client three directory levels up instead of two, so the rendered CLI crate could not resolve its client dependency. The TypeScript event log module now creates the `events` directory before writing `logs.ts` and treats a missing barrel file as empty instead of failing, which lets programs without events generate cleanly. The CPI crate scaffold pins `pina = "0.17"` instead of the stale `0.12` floor.

A new `pina snapshot` command emits the normalized CLI-surface snapshot that release automation diffs against the committed baseline at `.monochange/cli-snapshots/pina.json`, and `pina_cli` is now registered as a CLI package so changes to a flag or value set are classified as compatibility findings instead of unknown package changes. Run `pina snapshot --save` after an intentional surface change.
