# `pina profile`

Estimate compute-unit costs in a compiled Solana SBF shared object without starting a validator, or measure them line by line from the project's Mollusk tests with [`pina profile trace`](#tracing-executed-compute-units).

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

### Function names

Function names are demangled Rust paths without the legacy `::h<hash>` suffix, so a function keeps its name across rebuilds and `compare` can match it. The deployed artifact is stripped down to its exported symbols, so when the profiled file sits in a directory directly below the Cargo target directory (such as `target/deploy/`), Pina reads function names from the unstripped linker output `cargo build-sbf` leaves at `target/sbpf-solana-solana/release/<lib-target>.so`. That file is only used when its `.text` section is byte-identical to the profiled one. Without it, the report falls back to the exported symbols, which is usually just `entrypoint`. A release profile with `strip = true` strips the intermediate too.

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

## Tracing executed compute units

`pina profile trace` answers "where did the compute units of _this_ instruction go, line by line?" by measuring instead of estimating. It builds the program, runs the project's Mollusk tests with register tracing, and attributes every executed SBF instruction to a source line and a call stack.

```text
pina profile trace [OPTIONS]
```

| Input                  | Default | Meaning                                                                       |
| ---------------------- | ------- | ----------------------------------------------------------------------------- |
| `--project <DIR>`      | `.`     | Start directory for project discovery.                                        |
| `--filter <TEST>`      | all     | Run only tests whose names contain `TEST`, as `cargo test <TEST>` does.       |
| `--instruction <NAME>` | all     | Report one instruction, in any case style, or a label such as `increment #2`. |
| `--trace-dir <DIR>`    | none    | Analyze existing traces without building or running tests.                    |
| `--json`               | off     | Emit the versioned JSON document.                                             |
| `--folded`             | off     | Emit folded stacks for speedscope, inferno, or `flamegraph.pl`.               |
| `-o, --output <FILE>`  | stdout  | Write the selected output to a file.                                          |

```bash
pina profile trace
pina profile trace --project ./programs/counter_program
pina profile trace --filter increment --instruction increment
pina profile trace --json --output ./trace.json
pina profile trace --folded > ./stacks.folded
pina profile trace --trace-dir ./target/pina/trace/traces
```

### Setup

Mollusk records traces only when its `register-tracing` feature is enabled, so declare the program's dev-dependency with it:

```toml
[dev-dependencies]
mollusk-svm = { version = "0.15", features = ["register-tracing"] }
```

Tests must load the program by name (for example `Mollusk::new(&program_id, "my_program")`), so the run can point `SBF_OUT_DIR` at the traced build. A copy of the program in `tests/fixtures/` takes precedence over `SBF_OUT_DIR` and is reported as traces from another build. When no trace is recorded, the error shows the exact dependency line for the program's manifest.

### What it runs

1. `cargo build-sbf` with the production size profile `pina build` uses, plus `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only` and `CARGO_PROFILE_RELEASE_STRIP=none`. The stripped program goes to `<target>/pina/trace/build/`, where the tests load it, and the unstripped linker output with DWARF is copied to `<target>/pina/trace/<lib-target>.so.debug` for attribution. Mollusk cannot load an ELF whose symbol table holds long Rust names while tracing, which is why the two are separate.
2. The same build without debug information into `<target>/pina/trace/release/`, to check whether debug information changed code generation.
3. `cargo test --manifest-path <program>/Cargo.toml [TEST]` with `SBF_OUT_DIR` and `SBF_TRACE_DIR=<target>/pina/trace/traces` set. Test output goes to stderr so `--json` and `--folded` keep stdout machine-readable. A failing test run exits with the test runner's status; the traces recorded before the failure stay in the trace directory for `--trace-dir`.

`--trace-dir` skips all three steps and attributes the traces with the traced build from the project's last run.

### Reading the results

Every executed SBF instruction costs 1 CU, so the trace counts exactly what the program executed. The runtime charges syscalls separately and those charges are not in the trace: each syscall is listed by name, invocation count, and calling line, and only its 1-CU `call` instruction is counted.

Recordings that followed exactly the same instruction path are reported once, with every trace id. A profile is named after the program instruction whose discriminator it loaded from the instruction data (the loader passes its address in `r2`), and otherwise `trace <id>`. Profiles are ordered by name and then by cost, and repeated names get a ` #2` suffix, so identical recordings always produce identical output.

- **Lines** are the innermost source line of each instruction. Instructions the compiler attributed to line 0 are reported as having no line information.
- **Functions** report self and inclusive cost. Code that LTO inlined into `entrypoint` is attributed to the function it was written in through DWARF's inlined-subroutine records; inlined frames carry DWARF's short names, such as `assert_writable` or `get<u8>`.
- **Stacks** are physical call frames rebuilt from the trace's `call`, `callx`, and `exit` instructions, each expanded into its inlined frames.

The text summary lists each profile's ten most expensive lines, top functions, and syscalls, then prints the path of the HTML report at `<target>/pina/trace/<lib-target>.html`. The report is a single self-contained file with an instruction picker, a zoomable icicle chart of the call stacks, the hottest lines, and the sampled source files annotated with per-line cost. Source is read when the report is written. Paths of workspace members resolve against the workspace root; dependencies from a registry record paths relative to their own package (`src/lib.rs`), so their lines show numbers without source.

### Debug information and code generation

Debug information can change SBF code generation, so the traced build is compared with the release build. When their `.text` sections differ the command warns with the number of differing instruction slots, and the JSON document records it under `releaseBuild`. Executed counts then describe the traced build and can differ slightly from the deployed program.

### JSON trace document

`--json` emits a camelCase document with `schemaVersion: 1`: the program name, the SHA-256 of the traced executable, `lineInfo`, `recordedTraces` and `skippedTraces`, `releaseBuild` (or `null`), and an `instructions` array. Each instruction carries its `name`, the matched `instruction` and observed `discriminator`, `traceIds`, `executedInstructions`, `syscallInvocations`, `unattributedInstructions`, and `lines`, `functions`, `syscalls`, and `stacks` arrays.

## Safety and failures

The command compares filesystem identity and refuses an output that is the input binary, a hardlink to it, or below a symbolic-link/reparse-point path. Reports are published atomically, so a failed write cannot truncate an existing destination or the input binary. Profiling also fails for unreadable files, invalid ELF data, binaries without an SBF text section, output creation errors, and JSON serialization errors. `compare` additionally fails closed for missing, unreadable, or malformed baselines, documents that are not profile reports, and unsupported baseline schema versions.
