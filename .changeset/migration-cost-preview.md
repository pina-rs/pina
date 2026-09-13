---
pina_cli: feat
---

# Preview Migration Costs in `pina migrations status`

`pina migrations status` now prints a pre-deploy cost preview derived from the checked-in history and `pina profile`'s static SBF estimates. Per account contract it reports the current size, the bytes a version-0 (day-one) account grows, and the approximate rent deficit at the shared 6,960-lamports-per-grown-byte convention. Per instruction process it names the worst-case adjacent-step ladder a stale account can trigger, with the step count and a static CU estimate; the ladder model and CU model are printed so the numbers stay interpretable, and anything that cannot be estimated reports an explicit reason instead of a zero. A program-wide "most expensive touching transaction" summary identifies the instruction whose worst-case ladders cost the most rent so `max_lamports` and `MAX_INLINE_STEPS` can be sized deliberately, and the summary quotes the same `max_lamports` remedy text as the `make` growth warning.

`pina migrations status --json` gained a `costPreview` object and now emits `{ "statuses": [...], "costPreview": {...} }`; each `MigrationStatus` field keeps its previous name and shape. `pina migrations check --json` still emits the status array unchanged.
