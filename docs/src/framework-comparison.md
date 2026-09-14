# Framework comparison

Two programs, built with four frameworks, measured two ways. Size decides what a deployment costs in rent; compute units decide how much of a transaction's budget the instruction spends. Both tables below are produced by one command:

```sh
devenv shell -- benchmark:frameworks
```

That command rebuilds every program and rewrites the generated region of this page, so the published numbers cannot drift from the code that produced them.

## What is measured

Every program is a standalone crate under `benchmarks/framework-comparison/programs`. There are two of them:

- **Hello world** — one instruction, one signer check, one static log line.
- **Counter** — a PDA seeded by `b"counter" + authority`, a ten-byte account holding a discriminator, a bump and a `u64` count, with `initialize` and `increment` instructions.

Both are deliberately tiny. The difference between the frameworks is the framework's own dispatch, validation, and entrypoint code, not application logic — which is exactly the overhead the table is meant to expose.

Size is the deployed `.so` in bytes. Compute units are measured by executing each instruction in a Mollusk VM and reading the counter the runtime charged, so they include CPI costs the program triggers. A program that fails, or that does not leave the expected account state, aborts the run rather than reporting a fast number for work it never did.

### Build configuration

The comparison is meant to show framework overhead, not build settings, so every row is built as favourably as possible and identically:

- `cargo build-sbf --lto` (Agave 4.2.2, `sbpf-solana-solana` target)
- `lto = "fat"`, `codegen-units = 1`, `opt-level = 3`, overflow checks off
- `crate-type = ["cdylib"]` only, which is what lets LTO apply at all

See [Program size](./program-size.md) for why those settings matter and what each one is worth on its own.

## Results

<!-- BEGIN GENERATED: framework-comparison -->

### Hello world

| Framework                   | Size (bytes) | `hello` CU | vs Pinocchio size |
| --------------------------- | -----------: | ---------: | ----------------: |
| Pina                        |        4,680 |        145 |              +48% |
| Pinocchio (hand-written)    |        3,160 |        111 |               +0% |
| Quasar                      |        2,520 |        115 |              −20% |
| Anchor v2 (`lang-v2`, rc.1) |        1,880 |        127 |              −41% |

### Counter

| Framework                   | Size (bytes) | `initialize` CU | `increment` CU | vs Pinocchio size |
| --------------------------- | -----------: | --------------: | -------------: | ----------------: |
| Pina                        |       12,376 |          10,719 |          1,753 |              +90% |
| Pinocchio (hand-written)    |        6,512 |           1,490 |          1,721 |               +0% |
| Quasar                      |        7,808 |           3,488 |            330 |              +20% |
| Anchor v2 (`lang-v2`, rc.1) |        8,696 |           3,458 |          2,117 |              +34% |

<!-- END GENERATED: framework-comparison -->

## Reading the numbers

**The account layouts are not identical in every row.** Pina, Pinocchio and Quasar store the counter as `discriminator, bump, count` — ten bytes. Anchor v2 prefixes an eight-byte discriminator, which makes its account twenty-four bytes after alignment, so its `initialize` pays more for the `create_account` CPI. That is inherent to the framework's account model rather than a tuning choice, and it is the main reason Anchor's counter numbers are not directly comparable instruction-for-instruction.

**Pina and Pinocchio receive the PDA bump as an instruction argument**; Quasar and Anchor derive it from the declared seeds on-chain. Deriving a bump costs a PDA search the other two avoid, so `initialize` is not purely a framework overhead comparison.

**The Pinocchio row is the floor.** It is hand-written `pinocchio` with no framework at all, and it is the number a framework has to justify. Pina's gap to it is the cost of derive-generated dispatch and validation.

**Pina's `initialize` is the one number that looks like a defect rather than a design cost.** Creating the counter account costs 10,719 CU against Pinocchio's 1,490 for the same `create_account` CPI in the same instruction. The difference is [`CreateProgramAccountWithBump`](../crates/pina/src/cpi.rs), which validates the PDA with `try_find_program_address` — a search over up to 256 candidate bumps — even though the caller has already supplied the canonical bump as an argument. That is about 9,200 CU, roughly 5% of the default 200,000 CU a non-builtin instruction is allocated, spent re-deriving a value the program was handed.

Every PDA-creating example in this repository uses that helper, so the cost is not specific to the counter. Replacing the search with a single `create_program_address` when a bump is supplied would remove it, but that is a security-relevant change — canonicity is what the search proves — so it belongs in its own pull request rather than in the harness that measured it.

## Reproducing

```sh
devenv shell -- benchmark:frameworks
```

The command needs the Agave SBF toolchain (for `cargo build-sbf`) and network access on the first run, because the Quasar and Anchor v2 programs depend on pinned revisions of their upstream repositories. Both revisions are pinned in the fixture manifests, and each fixture is a standalone crate so those dependencies never enter the workspace lockfile.

Regenerating rewrites the tables in whatever alignment the script emits, so follow it with `fix:format` to restore dprint's column alignment.
