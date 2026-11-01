# Framework comparison

Two programs, built with five frameworks, measured two ways. Size decides what a deployment costs in rent; compute units decide how much of a transaction's budget the instruction spends. Both tables below are produced by one command:

```sh
devenv shell -- benchmark:frameworks
```

That command rebuilds every program and rewrites the generated region of this page and of the repository readme, so the published numbers cannot drift from the code that produced them.

## What is measured

Every program is a standalone crate under `benchmarks/framework-comparison/programs`. There are two of them:

- **Hello world** — one instruction and one static log line. Framework rows check a signer, except the hand-written Pinocchio row and the explicitly unchecked Pina variant.
- **Counter** — a PDA seeded by `b"counter" + authority`, a ten-byte account holding a discriminator, a bump and a `u64` count, with `initialize` and `increment` instructions.

Both are deliberately tiny. The difference between the frameworks is the framework's own dispatch, validation, and entrypoint code, not application logic — which is exactly the overhead the table is meant to expose.

Size is the deployed `.so` in bytes. Compute units are measured by executing each instruction in a Mollusk VM and reading the counter the runtime charged, so they include CPI costs the program triggers. A program that fails, or that does not leave the expected account state, aborts the run rather than reporting a fast number for work it never did.

### Build configuration

The comparison is meant to show framework overhead, not build settings, so every row is built as favourably as possible and identically:

- `cargo build-sbf --lto` (Agave 4.2.2, `sbpf-solana-solana` target)
- `lto = "fat"`, `codegen-units = 1`, `opt-level = 3`, overflow checks off
- `crate-type = ["cdylib"]` only, which is what lets LTO apply at all

Each program uses its framework's recommended entrypoint. For Pina that is `dispatch_entrypoint!` over the `#[discriminator(entrypoint)]` router: it reads the instruction through the loader's instruction-data pointer, then walks only the accounts the routed instruction reads, rejecting extra trailing accounts.

**The unchecked Pina hello is an explicit benchmark opt-in.** It enables the fixture's `benchmark-unchecked` feature, which selects `#[discriminator(entrypoint, skip_program_id_check)]` and omits the handler's signer and exact payload-length checks. It retains discriminator parsing and account-count checks. The default Pina row retains every check. These rows measure different validation policies; the unchecked row is not a claim of equivalent security to a framework that checks a signer.

See [Program size](./program-size.md) for why those settings matter and what each one is worth on its own.

## Results

<!-- BEGIN GENERATED: framework-comparison -->

### Hello world

| Framework                | Version       | Size (bytes) | `hello` CU | vs Pinocchio size |
| ------------------------ | ------------- | -----------: | ---------: | ----------------: |
| Pina                     | 0.23.0        |        1,616 |        136 |              −49% |
| Pina (unchecked hello)   | 0.23.0        |        1,392 |        117 |              −56% |
| Pinocchio (hand-written) | 0.11.2        |        3,160 |        111 |               +0% |
| Quasar                   | rev `863ee77` |        1,880 |        119 |              −41% |
| Anchor v1                | 1.2.2         |       74,416 |        421 |            +2255% |
| Anchor v2 (`lang-v2`)    | 2.0.0-rc.2    |        2,456 |        152 |              −22% |

### Counter

| Framework                | Version       | Size (bytes) | `initialize` CU | `increment` CU | vs Pinocchio size |
| ------------------------ | ------------- | -----------: | --------------: | -------------: | ----------------: |
| Pina                     | 0.23.0        |        7,592 |           1,694 |            360 |              +17% |
| Pina (SIMD-0312)         | 0.23.0        |        6,376 |           1,705 |            360 |               −2% |
| Pinocchio (hand-written) | 0.11.2        |        6,512 |           1,490 |          1,721 |               +0% |
| Quasar                   | rev `863ee77` |       12,056 |           3,474 |          1,134 |              +85% |
| Anchor v1                | 1.2.2         |      139,496 |          12,673 |         10,391 |            +2042% |
| Anchor v2 (`lang-v2`)    | 2.0.0-rc.2    |       10,752 |           3,518 |          2,165 |              +65% |

<!-- END GENERATED: framework-comparison -->

## Reading the numbers

**The account layouts are not identical in every row.** Pina, Pinocchio and Quasar store the counter as `discriminator, bump, count` — ten bytes. Anchor v1 prefixes an eight-byte discriminator and borsh-packs the payload, making its account seventeen bytes; Anchor v2 prefixes the same eight bytes but pads the payload out to its alignment, making it twenty-four. Both `initialize` rows pay more for the `create_account` CPI accordingly. That is inherent to each framework's account model rather than a tuning choice, and it is the main reason Anchor's counter numbers are not directly comparable instruction-for-instruction.

**Anchor v1's size is its dependency stack, not its logic.** The v1 line builds on `solana-program` with std, a heap, and borsh, so even a one-instruction hello world carries tens of kilobytes of runtime the SBF deployer uploads and rents. That is the cost the `no_std` frameworks in the other rows exist to avoid.

**Quasar 0.2.0's counter is larger than its hello because of its `init` path.** Creating a program-owned account links Quasar's floating-point rent calculation, which adds several kilobytes of compiler-builtins code, and its `init` also keeps the fallback that funds and allocates a pre-funded target. Its own vault benchmark funds a system-owned vault with a transfer and never runs that path, so the two numbers are not comparable. The counter table measures what the framework's `init` actually costs.

**Pina and Pinocchio receive the PDA bump as an instruction argument**; Quasar and both Anchor lines derive it from the declared seeds on-chain. Deriving a bump costs a PDA search the other two avoid, so `initialize` is not purely a framework overhead comparison.

**The Pinocchio row is a hand-written baseline.** Its hello only logs: it does not validate a signer, program ID, or instruction discriminator. Default Pina checks program identity, the exact account count, instruction-data length, and the signer. Quasar's hello checks a signer, omits the program-ID comparison, and accepts extra accounts. The unchecked Pina row exposes a reduced-check configuration separately. Pina's counter also supports a pre-funded target, which the hand-written fixture does not.

**Pina's `initialize` supports pre-funded targets.** The fixture uses `CreateProgramAccountWithUncheckedBump`, which checks one supplied-bump derivation with `sha256`. The runtime derives the CPI signer and rejects an on-curve address. The default row uses the original creation sequence. The separately labelled SIMD-0312 row enables the optional `create-account-allow-prefund` Cargo feature: one `CreateAccountAllowPrefund` CPI transfers the rent deficit, allocates, and assigns. That option requires SIMD-0312 on the deployment cluster and trades initialization CU for a smaller deployment. Quasar and both Anchor lines also support pre-funded targets; the hand-written Pinocchio fixture does not. The verifier exercises both Pina modes with one lamport, exact rent, and surplus rent, checking the owner, size, state, and exact funding balances.

`invoke_fresh_with` is the smaller opt-in path for a program that rejects a pre-funded target: it uses the original `CreateAccount` instruction. A third party sending a lamport to a deterministic PDA before initialization can block that path. Use it only where that griefing vector is acceptable or the caller already proved the target fresh.

The distinction between the canonical and unchecked builders is the other thing to keep in mind. The canonical builder proves the supplied bump is the highest valid one, so a seed namespace maps to exactly one address; the unchecked builder proves only that the supplied bump derives the account's address, which is about 9,200 CU cheaper per creation. A non-canonical bump creates a second valid address that canonical derivation will not find — harmless for a per-authority counter, wrong for a vault that another program derives by seed alone. Every PDA-creating example in this repository that supplies a bump uses the unchecked builder.

## Reproducing

```sh
devenv shell -- benchmark:frameworks
```

The command needs the Agave SBF toolchain (for `cargo build-sbf`) and network access on the first run, because the Quasar and Anchor programs depend on pinned revisions or versions of their upstream releases. The Anchor v1 fixtures pin the latest stable `anchor-lang` and the v2 fixture pins the published pre-release crate; Quasar pins a revision because it publishes no versioned crates. Every fixture is a standalone crate so those dependencies never enter the workspace lockfile.

Regenerating rewrites the tables in whatever alignment the script emits, so follow it with `fix:format` to restore dprint's column alignment.
