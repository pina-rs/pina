# Framework comparison

Builds the same two programs — a hello world and a PDA counter — with Pina, hand-written Pinocchio, Quasar, and Anchor v2, then records what each costs on-chain: deployed size in bytes and the compute units the instruction actually consumes.

The published tables live in [docs/src/framework-comparison.md](../../docs/src/framework-comparison.md). Regenerate them with one command:

```sh
devenv shell -- benchmark:frameworks
```

## Layout

- `programs/<program>/<framework>/` — one standalone crate per row of the table. Each declares its own `[workspace]` so the foreign framework revisions they pin stay out of the root workspace lockfile.
- `verifier/` — a host-side Mollusk harness that loads a compiled `.so`, runs the instruction flow, and reports the charged compute units. It also proves the flow worked, so a fast number from a program that errors out can never be recorded as a result.

`scripts/benchmark-frameworks.ts` drives both: it builds every fixture with `cargo build-sbf --lto` under one shared release profile, runs the verifier, and rewrites the generated region of the docs page Both cargo invocations pass `--locked`, so what gets built is exactly what the committed lockfiles describe.

## Adding a framework or a program

1. Add the crate under `programs/<program>/<framework>/`, with its own `[workspace]` and a pinned dependency revision.
2. Add an entry to `frameworksFor()` in `scripts/benchmark-frameworks.ts`: the directory, the label shown in the table, the artifact stem, and the instruction data each instruction expects.

Instruction data is per framework because the frameworks disagree about how to name instructions. Pina, Pinocchio, and Quasar number them from zero; Anchor hashes the handler name into an eight-byte discriminator. Two details are worth knowing before editing those entries:

- **The bump.** The Pina and Pinocchio counter programs read the PDA bump from instruction data, so the verifier appends it. Quasar and Anchor derive their own bump from the declared seeds, so they must not receive it.
- **The log line.** The verifier can assert that a program logged an expected string, which is what stops a hello world that returns early from being measured as a valid result. It applies only to the single-instruction case; the counter is proven by the account state it leaves behind.

## Why the numbers are comparable

Every row is built with the same toolchain, the same target, and the same release profile, and every instruction is measured in the same VM. The remaining differences are documented on the docs page — chiefly Anchor's eight-byte account discriminator, which makes its counter account larger and its `initialize` more expensive, and the bump each framework receives.
