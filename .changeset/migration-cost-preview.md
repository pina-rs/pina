---
pina_cli: feat
---

# Preview Migration Costs in `pina migrations status`

`pina migrations status` now prints a pre-deploy cost preview derived from the checked-in history and `pina profile`'s static SBF estimates. Per account contract it reports the current size, the bytes a version-0 (day-one) account grows, and the approximate rent deficit at the shared 6,960-lamports-per-grown-byte convention. Per instruction process it names the worst-case adjacent-step ladder a stale account can trigger, with the step count and a static CU estimate; the ladder model and CU model are printed so the numbers stay interpretable, and anything that cannot be estimated reports an explicit reason instead of a zero. The program-wide summary sizes both budgets deliberately and independently: it names the touching transaction funding the most rent (for `max_lamports`) and the one running the longest worst-case ladder (for `MAX_INLINE_STEPS`), because they need not be the same instruction, and quotes the same `max_lamports` remedy text as the `make` growth warning. A `writable`, non-signer process slot that names no checked-in account contract produces an explicit note instead of silently costing nothing.

`pina migrations status --json` gained a `costPreview` object and now emits `{ "statuses": [...], "costPreview": {...} }`; each `MigrationStatus` field keeps its previous name and shape. `pina migrations check --json` still emits the status array unchanged.
