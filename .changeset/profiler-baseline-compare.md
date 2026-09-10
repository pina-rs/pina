---
pina: none
pina_cli: feat
pina_profile: feat
---

# Add local profiler baseline comparison

`pina profile compare <BASELINE>` profiles the current SBF artifact, diffs it against a saved report, and prints a delta-oriented summary that answers "what changed between these two builds?".

The baseline may be any report written by `pina profile --json --output`, or a new versioned baseline document carrying an explicit `schema_version` marker; unknown versions and non-profile documents fail with clear errors instead of being silently misread. Functions are matched by symbol name, and added, removed, changed, and unchanged functions are reported separately.

The text output prints total deltas followed by per-function deltas sorted by absolute CU change, colored red for increases and green for decreases. `--json` emits a stable, ordered comparison document with a `schema_version`, total snapshots and deltas, the applied threshold, an overall status, and the sorted function array, so identical inputs always produce identical bytes.

`--fail-cu` (default 500) and `--fail-percent` (default 10) mirror the CI compute-unit failure policy: the exit status is 2 only when the total CU regression reaches both limits, 0 otherwise, and 1 for operational errors. Local runs therefore reproduce the CI gate by default.

The performance workflow now uses this to diff every tracked example at function level. A new `scripts/profile-example-diffs.ts` step in the `program-profiles` job pairs the base and head static profiles the job already produces, runs `pina profile compare` per program, and publishes a diagnostic "Program function-level profile diffs" section in the consolidated performance report: per-program tables of added, removed, and changed functions sorted by absolute CU delta (increases flagged), plus new baselines, skipped programs, and unchanged programs on a single line. New baselines, skipped programs, and individual comparison failures are reported without failing the run; the instruction compute-unit policy gate remains the sole enforcement point.
