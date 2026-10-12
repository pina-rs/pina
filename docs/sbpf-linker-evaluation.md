# sbpf-linker build path evaluation

Status: re-evaluated on **0.2.3 (October 2026)**. Still not the default — but the upstream path now works and ships as an experimental opt-in: the pinned `sbpf-linker` 0.2.3 in `[workspace.metadata.bin]`, the fixed `[target.bpfel-unknown-none]` rustflags in `.cargo/config.toml`, the `examples/upstream_bpf_program` showcase, the `sbpf-linker = true` key in pina.toml that `pina init --sbpf-linker` records and plain `pina build` honors. The measured matrix and the caveats live in the re-evaluation below; the original 0.2.1 findings follow as history.

## Re-evaluation: sbpf-linker 0.2.3 (October 2026)

What changed since the 0.2.1 evaluation: sbpf-linker 0.2.x matured (relocation and rodata handling, an SBPF assembler stage, `--export` support, embedded libcall builtins), and — decisive for this repo — the recipe now pins **static syscalls**. The two `--cfg` flags in the root `.cargo/config.toml` switch pinocchio's syscall externs to murmur3-hash immediates with no relocation tables, which sidesteps the failure class that made the 0.2.1 `bpfel-unknown-none` artifacts silently corrupt. Every upstream artifact below passes its full Surfpool suite (the same Agave 4.2 runtime the CI benchmark uses) plus Mollusk log-content verification; the 0.2.1 "empty log" corruption is gone.

The other half of the 0.2.1 conclusion survives unchanged: with Agave's own rustc, swapping in the sbpf-linker binary buys nothing — the code is identical (`agave-bb-lto` counter: the same 1,218 instructions as `cargo build-sbf --lto`, plus ~2.2 KB of unstripped `.symtab`/`.strtab` overhead). The interesting path is upstream nightly Rust + LLVM 23 for `bpfel-unknown-none`, which also emits the eBPF v3 JMP32/ALU32 forms SIMD-0377 legalized on Solana.

### Comparison matrix

Measured on this repository, October 2026: `cargo build-sbf --lto` (platform-tools v1.54, rustc 1.89, LLVM 20) versus upstream nightly-2026-09-15 (LLVM 23.1) + sbpf-linker 0.2.3, no LTO. Runtime compute units come from the Surfpool transaction simulation the PR benchmark uses (max per instruction); static numbers come from `pina profile`.

**Runtime compute units** (parity — this is why the default stays `cargo build-sbf`):

| Program.instruction              |          build-sbf --lto |     upstream sbpf-linker |                   Δ |
| -------------------------------- | -----------------------: | -----------------------: | ------------------: |
| hello_solana.hello               |                      145 |                      150 |               +3.4% |
| upstream_bpf.hello               |                      144 |                      153 |               +6.3% |
| upstream_bpf.ping                |                      212 |                      195 |           **−8.0%** |
| counter.initialize               |                    1,704 |                    1,714 |               +0.6% |
| counter.increment                |                      379 |                      397 |               +4.7% |
| escrow.make / take / cancel      | 34,881 / 31,507 / 13,271 | 34,564 / 31,200 / 13,023 | −0.9 / −1.0 / −1.9% |
| staking_rewards (6 instructions) |           2,131 – 37,563 |               within ±2% |                 ≈0% |

Nothing here is a compute-unit win; the runtime column is why the default stays `cargo build-sbf`.

**Binary size** (the upstream path's real advantage):

| Program         | build-sbf --lto | upstream sbpf-linker |    Δ |
| --------------- | --------------: | -------------------: | ---: |
| hello_solana    |           3,720 |                2,784 | −25% |
| upstream_bpf    |           5,496 |                4,176 | −24% |
| counter         |          12,984 |                8,944 | −31% |
| escrow          |          38,912 |               36,632 |  −6% |
| staking_rewards |          50,192 |               51,592 |  +3% |
| multisig        |         158,992 |     ✗ does not build |    — |

**Static text** (`pina profile`): hello 317 → 324 instructions, upstream_bpf 518 → 487, counter 1,334 → 1,067 (−20%). Caveat on the static CU totals: the profiler's syscall heuristic (100 CU per detected call site) recognizes the upstream static-syscall encodings (counter: 13 detected) but not the baseline's relocation-based calls (0 detected), so static CU is **not** comparable across the two encodings — the runtime table above is the signal that counts.

**Build speed and determinism** (counter, warm cache): `build-sbf --lto` 2.3 s vs upstream 1.0 s; upstream warm rebuilds are byte-identical (the `build-sbf` rebuild was not). The first upstream build pays a one-time `-Z build-std=core,alloc` sysroot compile.

**Fat LTO** — measured, and the reason the upstream recipe never uses it:

- Runtime CU regression: counter.increment 397 → 671 (+77%), escrow.take +37%, hello 150 → 237 (+63%), despite smaller binaries.
- u128 division becomes unresolvable: rustc's fat LTO internalizes the build-std `__udivti3` definition, so the linker's `--export` has no archive member to pull. staking cannot be built upstream with LTO at all.
- The upstream build path therefore forces `CARGO_PROFILE_RELEASE_LTO=false` even over a scaffolded manifest that declares `lto = "fat"`.

### Caveats carried into the opt-in

1. **The static-syscall cfg flags are load-bearing.** Without `--cfg=target_os="solana"` and `--cfg=target_feature="static-syscalls"`, pinocchio syscalls fall back to relocation-based resolution that sbpf-linker mishandles on this target: programs load and return success while silently corrupting data (the 0.2.1 findings). The root `.cargo/config.toml`, the `pina init --sbpf-linker` scaffold, and the config-driven `pina build` path all carry them.
2. **u128 division needs `--export` flags.** The first build fails with `Symbol __udivti3 is missing. Export it with the --export=__udivti3 flag.`; the config-driven `pina build` path retries automatically with the linker's suggestion (up to 8 symbols); bare cargo builds must pass the flags by hand.
3. **Not every program builds upstream.** `multisig_program` fails twice over: its entrypoint frame exceeds the 4 KiB Solana stack ceiling under LLVM 23 codegen (the baseline sits at exactly 4,096), and with an 8 KiB budget sbpf-linker's assembler rejects opcode 0xd7 (the eBPF v4 64-bit byte-swap). Programs with large dispatch entrypoints are out of scope until the linker catches up.
4. **Do not add `solana-compiler-builtins`.** sbpf-linker 0.2.3 embeds its own builtins; the crate is redundant (counter builds byte-identically without it) and its strong libcall symbols collide with build-std's compiler-builtins under fat LTO (`__adddf3: symbol multiply defined`).
5. **The ELFs are section-less.** Static-syscall artifacts carry no section headers, so older tooling cannot read them. `pina profile` handles the layout since this change; `llvm-objdump` disassembles them as `PT_LOAD` segments.
6. **`Sysvar::get` is not testable under Surfpool.** The `sol_get_sysvar` runtime syscall is mainnet-valid but not implemented by the Surfpool (litesvm) runtime — `examples/upstream_bpf_program` reads the sysvar account instead, and programs that rely on the syscall cannot be functionally gated under Surfpool today.
7. **Windows.** sbpf-linker ships no Windows binaries.

## Original evaluation: v0.2.1 (September 2026) — history

`blueshift-gg/sbpf-linker` (v0.2.1) continues the upstream `bpf-linker` lineage rather than the `solana-labs` toolchain. The crates.io `sbpf-linker` 0.1.x versions that this repo and Agave already reference are the same project's older line.

Its distinguishing feature is a second stage: after LLVM links the objects, a custom `sbpf-assembler` pass reparses the linked ELF and re-emits SBPF bytecode. That enables `--arch=v0|v3` (default `v3`), `--sbpf-optimize`, and stack-argument fusion.

### The target decides correctness

This was the important 0.2.1 finding, and it is not obvious from the README. **Note: the 0.2.3 re-evaluation above shows the `bpfel-unknown-none` row was fixable with static syscalls — the recipe, not the target itself, was incomplete.**

| Target                                                        | Result (0.2.1, without static syscalls)                            |
| ------------------------------------------------------------- | ------------------------------------------------------------------ |
| `bpfel-unknown-none` (what the repo's `build-bpf` alias used) | Builds, loads, and passes the loader — but **functionally broken** |
| `sbpf-solana-solana` (Agave's target)                         | Correct programs, verified in the VM                               |

On `bpfel-unknown-none`, `target_os` is `none`, so the `cfg(target_os = "solana")` paths inside Pinocchio and the Solana SDK crates compile differently. The visible symptoms:

- **Hello world**: executes and returns success, but logs an empty string. The message constant is absent from the ELF. Dumping the post-link module shows `sol_log_` declared with zero arguments and the string deleted, so the syscall reads garbage registers. This is silent corruption — the program reports success.
- **Counter**: faults with `Access violation in unknown section at address 0x1 of size 32`; the 32-byte program-id constant is missing.

Reproducible across blueshift 0.2.1 and stock 0.1.8, with pina's rustflags and with the published template flags. Any size measured on this target is meaningless, because the binaries are small partly _because required data is missing_ — early measurements showed an 80% reduction that was an artifact of the defect.

### Verified results on `sbpf-solana-solana` (0.2.1)

Both artifacts below passed full Mollusk execution: the hello world logs `Hello, Solana!`, and the counter completes initialize + increment with `counter_flow_ok: true`.

```sh
PATH=<sbpf-linker-0.2.1>/bin:$PATH \
  cargo +1.89.0-sbpf-solana-v1.54 build --release \
  --target sbpf-solana-solana -p counter_pina -F bpf-entrypoint
```

| Build path                                | Counter size | Works   |
| ----------------------------------------- | -----------: | ------- |
| `cargo build-sbf`                         |       37,472 | yes     |
| `cargo build-sbf --lto`                   |       25,032 | yes     |
| blueshift, `sbpf-solana-solana`           |       28,720 | yes     |
| **blueshift, `sbpf-solana-solana` + LTO** |   **15,512** | **yes** |

The LTO combination is 38% smaller than `cargo build-sbf --lto` on the same lab fixture. Build times were comparable (clean builds around 22-32 s for both paths; the blueshift path is faster when warm). Clean rebuilds are byte-identical, so determinism is preserved. Note that this fixture-level win does not reproduce on the real `counter_program` example with 0.2.3 — see the matrix above: same code as the baseline, plus ELF overhead.

### Follow-ups from the 0.2.1 recommendation — all done in the 0.2.3 re-evaluation

1. The `bpfel-unknown-none` path is fixed: the root rustflags now carry the static-syscall cfgs, `sbpf-linker` 0.2.3 is pinned, and the readme's build command works as written.
2. The functional gate exists: `examples/upstream_bpf_program`'s Surfpool suite asserts observable behavior (the greeting and the sysvar marker) under both toolchains, so an alternative build path can no longer ship on build success alone.
3. Report upstream: the remaining 0.2.3 defects (fat-LTO internalized libcall definitions, the 0xd7 assembler rejection, the stack-limit diagnostics on large entrypoints) are worth filing against `blueshift-gg/sbpf-linker`.

Raw 0.2.3 measurements, disassembly, and repro scripts: `tmp/sbpf23-lab/` (`REPORT.md`, `measure-cu.sh`, `build-upstream*.sh`); the 0.2.1 lab remains in `tmp/bb-linker-lab/` with the Mollusk verifier in `tmp/bb-verify/`.
