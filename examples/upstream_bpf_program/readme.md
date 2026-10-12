# `upstream_bpf_program`

<br>

Showcase for the experimental upstream BPF toolchain.

## What it covers

<br>

- Building the same pina program with upstream nightly Rust and the pinned `sbpf-linker` 0.2.3 for `bpfel-unknown-none` (static syscalls), instead of Agave's `cargo build-sbf`.
- Reading the `Clock` sysvar account zero-copy (`Clock::from_account_view`). The `sol_get_sysvar` runtime syscall is mainnet-valid but not implemented by the Surfpool test runtime, so the example validates and reads the sysvar account instead — the pattern `sysvar_checks_program` uses.
- A Surfpool suite that asserts the greeting and the clock log line, so the upstream artifacts are functionally gated rather than merely compiled.

## Run

<br>

```bash
cd examples/upstream_bpf_program
pina test --unit
pina test
pina generate
```

## Upstream sbpf-linker build

<br>

```bash
# From the repository root, with the pinned tooling installed:
install:all   # pins sbpf-linker 0.2.3 under .bin/rust-nightly/sbpf-linker/0.2.3/bin
PATH="$PWD/.bin/rust-nightly/sbpf-linker/0.2.3/bin:$PATH" cargo build-upstream-bpf-program
# Artifact: target/bpfel-unknown-none/release/libupstream_bpf_program.so
```

Or install once with `cargo binstall sbpf-linker` and run `sbpf-linker` from `PATH`. This example's `pina.toml` sets `sbpf-linker = true`, so a plain `pina build` drives the same recipe; remove the key to go back to the `cargo build-sbf` driver.

## Caveats (measured on sbpf-linker 0.2.3, October 2026)

1. **Static syscalls are mandatory.** The two `--cfg` flags in the root `.cargo/config.toml` are load-bearing: without them the pinocchio syscall externs fall back to relocation-based resolution that sbpf-linker mishandles, producing programs that load but silently corrupt data.
2. **No LTO.** Fat LTO under sbpf-linker inflated runtime compute units by 17-77% across pina examples and makes u128 division unresolvable at link time, so the upstream build always runs without it.
3. **u128 division needs export flags.** Programs calling `__udivti3` (u128 division) must pass `--export=__udivti3,…` so the linker keeps the build-std compiler-builtins definitions. The config-driven `pina build` path retries automatically with the symbols the linker reports missing.
4. **Not every program can build upstream.** Programs whose entrypoint frames exceed the 4 KiB Solana stack ceiling (the multisig example) fail with a stack-limit error, and sbpf-linker's assembler also rejects some upstream eBPF v4 opcodes (the 64-bit byte-swap, 0xd7) that larger programs emit.
5. **Do not add `solana-compiler-builtins`.** sbpf-linker 0.2.3 embeds its own builtins; the crate is redundant, and its strong libcall symbols break fat-LTO builds with `__adddf3: symbol multiply defined`.
6. **The ELFs are section-less.** Static-syscall artifacts carry no section headers, so older tooling cannot read them; `pina profile` supports them.

See `docs/sbpf-linker-evaluation.md` for the full comparison matrix — compute units, binary size, and build time against `cargo build-sbf`.
