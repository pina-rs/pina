# `pina profile`

Estimate compute-unit costs in a compiled Solana SBF shared object without starting a validator.

## Synopsis

```text
pina profile [OPTIONS] <PROGRAM.SO>
```

| Input                 | Default  | Meaning                                       |
| --------------------- | -------- | --------------------------------------------- |
| `PROGRAM.SO`          | required | Compiled SBF ELF shared object.               |
| `--json`              | off      | Emit structured JSON instead of a text table. |
| `-o, --output <FILE>` | stdout   | Write the selected format to a file.          |

## Examples

```bash
pina profile ./target/deploy/counter_program.so
pina profile ./target/deploy/counter_program.so --json
pina profile ./target/deploy/counter_program.so --json --output ./profile.json
```

JSON includes program and binary metadata, aggregate instruction/syscall/CU counts, and a per-function array with offsets, sizes, and estimates.

## Estimation model

The profiler reads the ELF text section, decodes SBF instructions, discovers functions, and applies the repository's static cost model. Regular instructions cost 1 estimated CU and recognized syscalls cost 100 estimated CU.

This is a deterministic comparison tool, not a replacement for runtime measurement. It cannot model data-dependent branches, invocation frequency, account state, CPI behavior, or runtime syscall variation. Use it to compare binaries and identify large functions, then validate important paths in an SVM or validator.

## Safety and failures

The command refuses to use the input binary itself as `--output`. It also fails for unreadable files, invalid ELF data, binaries without an SBF text section, output creation errors, and JSON serialization errors.
