# sbpf-linker build path evaluation

Status: evaluated. **Not adopted as a default.** Viable as a future opt-in with caveats — see the recommendation.

`blueshift-gg/sbpf-linker` can produce working programs that are meaningfully smaller than `cargo build-sbf`, but only on the `sbpf-solana-solana` target and only with the Agave toolchain. The `bpfel-unknown-none` target that this repo's `.cargo/config.toml` currently uses produces **silently broken** artifacts.

## What it is

[`blueshift-gg/sbpf-linker`](https://github.com/blueshift-gg/sbpf-linker) (v0.2.1) continues the upstream `bpf-linker` lineage rather than the `solana-labs` toolchain. The crates.io `sbpf-linker` 0.1.x versions that this repo and Agave already reference are the same project's older line.

Its distinguishing feature is a second stage: after LLVM links the objects, a custom `sbpf-assembler` pass reparses the linked ELF and re-emits SBPF bytecode. That enables `--arch=v0|v3` (default `v3`), `--sbpf-optimize`, and stack-argument fusion.

## The target decides correctness

This is the important finding, and it is not obvious from the README.

| Target                                                        | Result                                                                |
| ------------------------------------------------------------- | --------------------------------------------------------------------- |
| `bpfel-unknown-none` (what the repo's `build-bpf` alias uses) | Builds, loads, and passes the loader — but is **functionally broken** |
| `sbpf-solana-solana` (Agave's target)                         | Correct programs, verified in the VM                                  |

On `bpfel-unknown-none`, `target_os` is `none`, so the `cfg(target_os =
"solana")` paths inside Pinocchio and the Solana SDK crates compile differently. The visible symptoms:

- **Hello world**: executes and returns success, but logs an empty string. The message constant is absent from the ELF. Dumping the post-link module shows `sol_log_` declared with zero arguments and the string deleted, so the syscall reads garbage registers. This is silent corruption — the program reports success.
- **Counter**: faults with `Access violation in unknown section at address 0x1
  of size 32`; the 32-byte program-id constant is missing.

Reproducible across blueshift 0.2.1 and stock 0.1.8, with pina's rustflags and with the published template flags. Any size measured on this target is meaningless, because the binaries are small partly _because required data is missing_ — early measurements showed an 80% reduction that was an artifact of the defect.

## Verified results on `sbpf-solana-solana`

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

The LTO combination is 38% smaller than `cargo build-sbf --lto` on the same program. Build times were comparable (clean builds around 22-32 s for both paths; the blueshift path is faster when warm). Clean rebuilds are byte-identical, so determinism is preserved.

## Pre-existing issue in this repo

The repo's `cargo build-bpf` alias (`.cargo/config.toml`) uses the `bpfel-unknown-none` target and therefore produces broken artifacts. `examples/pina_bpf_program` builds to 8,584 bytes and executes and returns success, but logs an empty string instead of `Hello, Solana!` (verified in Mollusk with the correct program ID). The `build-*-program` aliases that use `cargo build-sbf` are unaffected.

This class of bug is possible because a build-success check cannot detect it — the artifact is a valid ELF that the loader accepts. Any adoption of an alternative linker needs a functional gate that asserts observable behaviour.

## Recommendation

**Keep `cargo build-sbf` as the default and do not add a `pina init` option yet.** The correctness risk is manageable but the win is not yet compelling enough to justify a second build path:

- It requires Agave's pinned `1.89.0-sbpf-solana-v1.54` toolchain, so it is not a way to escape that dependency.
- The benefit grows with program size: it wins clearly on a counter (−38% with LTO) but is closer to parity on a one-instruction program.

If this is pursued:

1. Fix or remove the `bpfel-unknown-none` `build-bpf` path first. Either point it at `sbpf-solana-solana`, or delete it so nobody builds broken programs.
2. Add a functional gate for any SBF build path — a Mollusk run asserting the expected log text, return value, and account state — instead of asserting the file exists and starts with `\x7fELF`.
3. Report the `bpfel-unknown-none` defect upstream with the minimal reproduction (a hello world logging an empty string).

Raw variants and scripts: `tmp/bb-linker-lab/` (`RESULTS.tsv`, `build-variants.sh`, `verify-variants.sh`), verifier in `tmp/bb-verify/`.
