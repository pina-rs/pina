---
pina_profile: feat
pina_cli: feat
pina_skill: docs
---

# Trace compute units per line with `pina profile trace`

`pina profile trace` answers where an instruction's compute units actually go. It builds the program with the production size profile plus DWARF line tables, runs the project's Mollusk tests with `SBF_TRACE_DIR` set, and attributes every executed SBF instruction (1 CU each) to a source line, a function, and a call stack, including code LTO inlined into `entrypoint`.

```sh
pina profile trace
pina profile trace --filter increment --instruction increment
pina profile trace --json -o trace.json
pina profile trace --folded > stacks.folded
pina profile trace --trace-dir ./target/pina/trace/traces
```

- The terminal summary lists each instruction's hottest lines with share bars, its top functions by self and inclusive cost, and its syscalls with their call sites. Recordings that followed the same path are reported once, and each profile is named after the instruction whose discriminator it loaded.
- A self-contained HTML report at `<target>/pina/trace/<program>.html` adds an instruction picker, a zoomable icicle chart, and the sampled source files annotated with per-line cost. It loads nothing from the network and works in light and dark mode.
- `--json` emits a versioned camelCase document (`schemaVersion: 1`), and `--folded` emits folded stacks for speedscope, inferno, or `flamegraph.pl`.
- The traced build is compared with the release build, and a warning reports how many instruction slots debug information changed.
- The program's `mollusk-svm` dev-dependency must enable `register-tracing`. When no trace is recorded, the error prints the exact dependency line for the program's manifest.

Syscall charges are made separately by the runtime and are not part of a trace, so syscalls are listed by name and invocation count but their charges are not included.

`pina_profile` gains the `trace`, `dwarf`, `syscalls`, `trace_report`, and `trace_output` modules that implement the analysis, and `ProfileError::DebugBuildMismatch` for an unstripped build whose `.text` differs from the executable it should describe. The counter example enables `register-tracing` so `pina profile trace --project examples/counter_program` works in the repository.
