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
2. Decodes each 8-byte SBF instruction's opcode
3. Estimates CU cost using an opcode-aware cost model:
   - Regular instructions (ALU, memory, branch): 1 CU each
   - Syscall instructions (`call imm` with src_reg=0): 100 CU each
4. Outputs a summary (text or JSON) with per-function breakdowns

## Limitations

- **Static analysis only** — does not account for runtime branching or loops
- **Flat syscall cost** — all syscalls estimated at 100 CU regardless of actual cost
- **Best-effort symbol resolution** — works best with unstripped binaries
- **No path analysis** — CU is the sum of all instructions, not worst-case path
