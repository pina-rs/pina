# `pina profile`

Estimate compute-unit costs in a compiled Solana SBF shared object without starting a validator.

## Synopsis

```text
pina profile [OPTIONS] [PROGRAM.SO]
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

## Safety and failures

The command compares filesystem identity and refuses an output that is the input binary, a hardlink to it, or below a symbolic-link/reparse-point path. Reports are published atomically, so a failed write cannot truncate an existing destination or the input binary. Profiling also fails for unreadable files, invalid ELF data, binaries without an SBF text section, output creation errors, and JSON serialization errors.
