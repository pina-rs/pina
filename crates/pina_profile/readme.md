# `pina_profile`

<p align="center">
	<img
		src="https://raw.githubusercontent.com/pina-rs/pina/main/.github/assets/logo.png"
		alt="Pina: a woven pineapple with an S at its centre"
		width="140"
	>
</p>

Static CU (Compute Unit) profiler for Solana SBF programs.

Analyzes compiled `.so` ELF binaries to estimate per-function compute unit costs without requiring a running validator.

<!-- {=crateReadmeBadgeRow:"pina_profile"} -->

[![Crates.io](https://img.shields.io/badge/crates.io-pina__profile-orange?logo=rust)](https://crates.io/crates/pina_profile) [![Docs.rs](https://img.shields.io/badge/docs.rs-pina__profile-1f425f?logo=docs.rs)](https://docs.rs/pina_profile/) [![CI](https://github.com/pina-rs/pina/actions/workflows/ci.yml/badge.svg)](https://github.com/pina-rs/pina/actions/workflows/ci.yml) [![Coverage](https://codecov.io/gh/pina-rs/pina/branch/main/graph/badge.svg)](https://codecov.io/gh/pina-rs/pina) [![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://opensource.org/license/apache-2.0)

<!-- {/crateReadmeBadgeRow} -->

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

Function names are demangled without the legacy `::h<hash>` suffix, so a function keeps its name across rebuilds. The stripped copy `cargo build-sbf --sbf-out-dir` publishes keeps only exported symbols, so the profiler borrows the full symbol table from the unstripped linker output under `<target>/sbpf-solana-solana/release/` when its `.text` is byte-identical, and otherwise falls back to the exported `.dynsym` entries.

## Trace-driven profiles

`pina profile trace` uses the dynamic half of this crate. Mollusk's `register-tracing` feature records one register set per executed SBF instruction, and each instruction costs exactly 1 CU:

- `trace` reads the `.regs`/`.insns` files Mollusk writes to `SBF_TRACE_DIR`.
- `dwarf` maps a program counter to its function (symbol table, demangled) and to the inlined frames and source line DWARF records for it.
- `trace_report` rebuilds call stacks from `call`, `callx`, and `exit`, names syscalls from their murmur3 call keys (`syscalls`), reads the instruction discriminator through the SIMD-0321 `r2` pointer, and aggregates executed instructions per line, function, and stack.
- `trace_output` renders the report as text, versioned JSON, or folded stacks.

Syscall charges are made separately by the runtime and are not part of a trace, so syscalls are reported as invocation counts at their call sites.

## Static profile limitations

- **Static analysis only** — does not account for runtime branching or loops
- **Flat syscall cost** — all syscalls estimated at 100 CU regardless of actual cost
- **Best-effort symbol resolution** — a stripped binary without its unstripped intermediate reports only its exported symbols (usually `entrypoint`)
- **No path analysis** — CU is the sum of all instructions, not worst-case path
