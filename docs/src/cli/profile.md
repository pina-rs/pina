# `pina profile`

Estimate compute-unit costs in a compiled Solana SBF shared object without starting a validator.

## Synopsis

```text
pina profile [OPTIONS] [PROGRAM.SO] [COMMAND]
```

| Input                 | Default  | Meaning                                       |
| --------------------- | -------- | --------------------------------------------- |
| `PROGRAM.SO`          | detected | Compiled SBF ELF shared object.               |
| `--project <DIR>`     | `.`      | Start directory for artifact discovery.       |
| `--json`              | off      | Emit structured JSON instead of a text table. |
| `-o, --output <FILE>` | stdout   | Write the selected format to a file.          |

## Examples

```bash
pina profile
pina profile --project ./programs/counter_program
pina profile ./target/deploy/counter_program.so
pina profile ./target/deploy/counter_program.so --json
pina profile ./target/deploy/counter_program.so --json --output ./profile.json
```

The positional path remains supported for scripts and custom artifacts. When it is omitted, Pina discovers the nearest Cargo program and profiles the canonical `<cargo-target>/deploy/<lib-target>.so` artifact. Discovery fails with the exact expected path when the program has not been built.

JSON includes program and binary metadata, aggregate instruction/syscall/CU counts, and a per-function array with offsets, sizes, and estimates.

## Estimation model

The profiler reads the ELF text section, decodes SBF instructions, discovers functions, and applies the repository's static cost model. Regular instructions cost 1 estimated CU and recognized syscalls cost 100 estimated CU.

This is a deterministic comparison tool, not a replacement for runtime measurement. It cannot model data-dependent branches, invocation frequency, account state, CPI behavior, or runtime syscall variation. Use it to compare binaries and identify large functions, then validate important paths in an SVM or validator.

## Comparing against a baseline

`pina profile compare` answers "what changed between these two builds?" by profiling the current artifact and diffing it against a saved report.

```text
pina profile compare <BASELINE> [PROGRAM.SO] [OPTIONS]
```

| Input                      | Default  | Meaning                                                 |
| -------------------------- | -------- | ------------------------------------------------------- |
| `BASELINE`                 | required | Saved profile report used as the baseline.              |
| `PROGRAM.SO`               | detected | Compiled SBF ELF shared object for the current build.   |
| `--project <DIR>`          | `.`      | Start directory for artifact discovery.                 |
| `--json`                   | off      | Emit a stable, ordered JSON comparison document.        |
| `--fail-cu <CU>`           | `500`    | Absolute total-CU increase that fails the comparison.   |
| `--fail-percent <PERCENT>` | `10`     | Percentage total-CU increase that fails the comparison. |

Capture a baseline before changing the program, rebuild, then compare:

```bash
pina profile --json --output ./profile.json
# ... change and rebuild the program ...
pina profile compare ./profile.json
pina profile compare ./profile.json --json
pina profile compare ./profile.json --fail-cu 100 --fail-percent 5
```

The baseline may be any report written by `pina profile --json --output`, or a versioned baseline document that carries an explicit `schema_version` marker. Baselines declaring a different schema version are rejected instead of silently misread.

The text summary prints total deltas, then per-function deltas sorted by absolute CU change. Functions are matched by symbol name; added and removed functions are reported separately. The summary line classifies the build as unchanged, improved, a small regression, or a threshold regression.

### Exit status

| Code | Meaning                                                                 |
| ---- | ----------------------------------------------------------------------- |
| `0`  | Comparison completed without a threshold regression.                    |
| `1`  | Operational error: unreadable or malformed baseline, profiling failure. |
| `2`  | The total CU regression reached both `--fail-cu` and `--fail-percent`.  |

Both limits must be reached, mirroring the failure policy of the repository's CI compute-unit workflow, so a local `pina profile compare` reproduces the CI gate with the default thresholds.

### JSON comparison document

`--json` emits a deterministic document with `schema_version`, baseline and current program names, total snapshots and deltas, the applied threshold, an overall `status` (`unchanged`, `improved`, `regression`, or `threshold-regression`), and a `functions` array sorted by delta magnitude. Identical inputs always produce identical bytes, so the output is safe to diff or store.

## Safety and failures

The command compares filesystem identity and refuses an output that is the input binary, a hardlink to it, or below a symbolic-link/reparse-point path. Reports are published atomically, so a failed write cannot truncate an existing destination or the input binary. Profiling also fails for unreadable files, invalid ELF data, binaries without an SBF text section, output creation errors, and JSON serialization errors. `compare` additionally fails closed for missing, unreadable, or malformed baselines, documents that are not profile reports, and unsupported baseline schema versions.
