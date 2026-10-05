---
pina_profile: breaking
pina_cli: fix
---

# Name functions in stripped SBF artifacts

`pina profile` reported a deployed artifact such as `target/deploy/<program>.so` as a single `<entire .text>` function, because the profiler read only `.symtab` and `cargo build-sbf` strips the published copy down to `.dynsym`. The CI function diffs and the per-step estimates in `pina migrations status` inherited the same collapse.

The profiler now falls back to the exported `.dynsym` symbols, and recovers the full symbol table from the unstripped linker output under `<target>/sbpf-solana-solana/release/` when its `.text` is byte-identical to the profiled file. Function names are demangled without the legacy `::h<hash>` suffix so they stay stable across rebuilds, and `pina profile compare` merges functions that share a demangled name instead of keeping only the last one.

Migrating `pina_profile` library code:

- `elf::ElfInfo` gains `symbol_table: elf::SymbolTable`, which records whether `symbols` came from `.symtab` (`Full`) or `.dynsym` (`Dynamic`). Code that builds an `ElfInfo` literal must set it.
- `ProfileError` is now `#[non_exhaustive]`, so later variants are not breaking. Add a wildcard arm to any exhaustive `match` on it.
- `elf::Symbol::name`, `FunctionProfile::name`, and the names in comparison reports are demangled without the `::h<hash>` suffix. A baseline saved before this release reports its functions as removed and the current ones as added; capture a new baseline to compare function by function.
