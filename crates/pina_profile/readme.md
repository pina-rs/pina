# pina_profile

Static CU (Compute Unit) profiler for Solana SBF programs.

Analyzes compiled `.so` ELF binaries to estimate per-function compute unit costs without requiring a running validator.

## Usage

```sh
pina profile target/deploy/my_program.so
pina profile target/deploy/my_program.so --json
pina profile target/deploy/my_program.so --output report.json
```

## How it works

Solana's SBF instruction set has deterministic CU costs. This tool:

1. Parses the ELF binary to extract `.text` sections
2. Walks the SBF instruction stream, counting instructions per symbol
3. Estimates CU cost using a static per-instruction baseline model
4. Outputs a summary (text or JSON) with per-function breakdowns

## Limitations

- Static analysis only — does not account for runtime branching
- Best-effort symbol resolution (works best with unstripped binaries)
- CU estimates represent worst-case / single-path costs
