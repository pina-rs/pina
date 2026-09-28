# Program size

Deployed program size determines rent, and rent is paid in SOL. A Pina program is built to be small: the framework's own overhead above a hand-written `pinocchio` program is a few hundred bytes once link-time optimization runs.

## Framework comparison

Same toolchain for every row: `cargo-build-sbf` (Agave 4.2.2), `sbpf-solana-solana` target, equivalent program semantics — a single-instruction hello world, and a PDA counter with `initialize`/`increment`.

| Framework                   | Hello world | Counter    |
| --------------------------- | ----------- | ---------- |
| Quasar                      | 2,520       | 7,808      |
| Pinocchio (hand-written)    | 3,160       | 6,512      |
| **Pina**                    | **4,680**   | **12,720** |
| Anchor v2 (`lang-v2`, rc.1) | 1,880       | 8,696      |
| Anchor (v1, 1.2.0)          | 55,752      | 122,160    |

[Framework comparison](./framework-comparison.md) holds the generated version of this table together with the compute units each instruction consumes, and `benchmark:frameworks` rebuilds and rewrites it. The headline: a v1 Anchor program is more than ten times the size of any of the others, and LTO makes it _larger_ rather than smaller.

Pina's remaining gap to Pinocchio is mostly framework surface: derive-generated validation and dispatch, plus the entrypoint wrapper. With no derive and no logs the framework floor is 3,352 bytes against Pinocchio's 3,160 — 192 bytes.

## What determines the size

Four decisions account for nearly all of it. `pina build` applies the first three automatically.

### 1. Link-time optimization

`lto = "fat"` with `codegen-units = 1` is worth 20-35% on its own.

A crate that declares `crate-type = ["cdylib", "lib"]` cannot be linked with LTO: rustc rejects `-C lto` when one invocation also emits an rlib, and the SBF toolchain silently drops the profile setting. Programs are therefore `crate-type = ["cdylib"]` only. See [Testing](#testing-a-cdylib-program) for how tests still reach the real code.

**Check the entrypoint's stack frame after switching.** LTO inlines every instruction handler into the entrypoint, and the SBF runtime allows 4 KB of stack per frame. A program with enough handlers can exceed that, and the failure is quiet: `cargo-build-sbf` prints

```
Error: Function entrypoint overflows the maximum allowed frame space by accessing
an offset 1088 bytes greater than the maximum of 4096. Estimated function frame
size: 5184 bytes.
```

to stderr but still **exits 0 and writes the `.so`**, so a build that looks successful can produce a program that faults at runtime. A real case: a 55-instruction program at 446 KB built fine with `["cdylib", "lib"]`, and switching to `["cdylib"]` alone — before any version change — produced a 5,184-byte frame.

When this happens, either keep `["cdylib", "lib"]` and forgo LTO, or raise the limit with `cargo build-sbf --sbf-stack-size <BYTES>`. Raising it is a runtime-budget decision, not a free change, so treat it the way you would any other resource limit.

### 2. Diagnostics

Failure paths are the largest avoidable cost. `log!("address: {} …", addr)` and caller locations pull all of `core::fmt` into the binary, which is far more expensive than the message strings themselves.

| Build                                    | Hello world | Counter |
| ---------------------------------------- | ----------- | ------- |
| Formatted diagnostics (pre-0.17 default) | 8,680       | 37,472  |
| Static diagnostics, `logs` on (default)  | 4,800       | 12,392  |
| `verbose-logs` on                        | 6,840       | 24,504  |
| `logs` off entirely                      | 4,696       | 18,680  |

The default `logs` feature now logs a fixed message per failure and keeps the descriptive text. Formatted detail and `file:line:column` locations moved to the opt-in `verbose-logs` feature.

```toml
[dependencies]
pina = { version = "0.17", features = ["logs", "derive"] }
```

Add `verbose-logs` while debugging:

```sh
pina build --features verbose-logs
```

Programs can also choose their own detail level per call:

```rust
use pina::*;

// A fixed message in every configuration.
log!("initialized");

// A formatted message. Only this call site pays for `core::fmt`.
log_verbose!("initialized counter {} for {}", index, authority.address());
```

The `log!` format arm collapses to the static [`DETAIL_POINTER_MESSAGE`] when `verbose-logs` is off, so the same source compiles to a small binary in production and detailed logs in development.

### 3. The release profile

```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
overflow-checks = false
```

`overflow-checks = false` lets arithmetic overflow wrap instead of panicking, which is why it is opt-in. Use `pina build --overflow-checks` when a program must fail loudly. `pina build --no-size-profile` skips every override.

An explicit `overflow-checks = true` in the workspace release profile outranks the size profile. Cargo profile environment variables beat `[profile.release]`, so without that precedence rule `pina build` would silently turn the manifest's opt-in into wrapping arithmetic. When the manifest opts in, `pina build` keeps the checks on, warns, and gives up only that part of the profile.

Deterministic verified builds (`pina build --verify`) never apply profile overrides. A verified artifact has to stay reproducible from its recorded Git revision, and an override that exists only on the command line cannot be reproduced by anyone rebuilding that revision. Declare the profile under `[profile.release]` in the workspace manifest — as `pina init` does — so the ordinary and verified backends agree. Pina warns when a requested size profile would make the two artifacts differ.

Every deploy path carries the profile: `pina build`, `pina test`, and `pina dev` all build through the default `Production` size profile, `pina init` writes the settings above into the generated workspace manifest, and the workspace's own example programs build with `--lto` through the `cargo build-<program>` aliases and the CU-measurement scripts. Raw `cargo build-sbf` without `--lto` remains the one way to build a `["cdylib"]`-only program 12-37% larger than every supported path.

### 4. Dependency features

Only enable what the program uses. `token`, `memo`, and `compact` each pull in their own dependencies. An unused dependency is removed entirely by the linker, so optional features save build time more than binary size — but a `token` program that also links `pinocchio-token-2022` pays for both.

## Measuring a program

```sh
pina build
wc -c target/deploy/my_program.so
pina profile target/deploy/my_program.so        # static CU estimates
pina profile target/deploy/my_program.so --json # machine-readable
```

Rent follows the ELF size directly: Loader v3 stores the raw ELF in the program data account, and rent is charged per byte. Shrinking a program by 10 KB saves roughly 0.07 SOL in one-time deployment rent.

## Testing a cdylib program

Cargo cannot link a `cdylib` into an integration test, and a separate test crate cannot depend on it either. Three patterns cover every test you need without giving up LTO.

**Unit tests live in `src/lib.rs`.** This is the coverage path: the tests compile from the same source that ships, so `cargo llvm-cov` measures real code. `#[cfg(test)] mod tests` is included in the `pina init` template.

```rust
// src/lib.rs
#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn decodes_initialized_instruction() {
		let mut data = [0u8; InitializeInstruction::SIZE];
		InitializeInstruction::initialize(&mut data, |args| {
			args.value = 7;
			Ok(())
		})
		.expect("initialize instruction storage");

		let decoded =
			InitializeInstruction::try_from_bytes(&data).expect("decode initialized instruction");
		assert_eq!(decoded.value, 7);
	}
}
```

```sh
cargo test --lib
cargo llvm-cov --lib --summary-only
```

**On-chain tests include the program source.** `tests/surfpool` is its own crate, so it can add `pina` as a dependency and pull in `src/lib.rs` directly. The instruction encoders and program ID it uses are the real ones.

```rust
// tests/surfpool/src/lib.rs
#[path = "../../../src/lib.rs"]
mod program;

use program::ID;
use program::InitializeInstruction;
```

**Integration tests keep artifact-level assertions.** `tests/integration.rs` cannot reference program types, so keep it to things that only need the build output.

!!! note "Why not `use my_program::*`" That requires an rlib, which costs 20-35% of deployed size. Including the source with `#[path = ...]` gives tests the real types without the rlib. The one trade-off: `#[path]` includes create a second copy that `cargo llvm-cov` reports as uncovered, so put behavioural tests in `src/lib.rs` where coverage is measured and use `#[path]` only where a test needs to drive the program's own types from another crate.

## Keeping size from regressing

`scripts/compare-compute-units.ts` records ELF size alongside compute units for every example program, and the benchmark report on each pull request includes a size column. Treat an increase the same way you treat a compute-unit increase: investigate it, or keep the pull request unmerged.

Every example program now builds as `["cdylib"]` only and is compiled with fat LTO everywhere it is built for deployment or measurement: the `cargo build-<program>` aliases carry `--lto`, and `scripts/build-runtime-compute-units.ts` derives the flag from each manifest through the example inventory's `ltoEligible` flag, so the PR benchmark measures the artifact users actually deploy. A program whose harness needs the crate as a library follows the same three steps: drop `"lib"` from `[lib] crate-type`, replace the `tests/surfpool` rlib dependency on the program with a `#[path = "../../../src/lib.rs"]` source include (mirroring the program's `pina` features in the test crate, and re-exporting `program::*` at the harness root when the program's submodules use `crate::` paths), and add `--lto` to the program's build alias.

Measured on the same commit, fat LTO compared with the plain `--features bpf-entrypoint` build. The first five were converted and measured first; the remaining twenty-two follow:

| Program                | Without LTO | With LTO | Δ size       |
| ---------------------- | ----------- | -------- | ------------ |
| escrow                 | 73,976      | 46,680   | −37%         |
| staking-rewards        | 87,264      | 59,280   | −32%         |
| vesting                | 57,632      | 46,856   | −19%         |
| multisig               | 195,752     | 166,792  | −15%         |
| hello-solana           | 5,656       | 4,712    | −17%         |
| counter                | 36,056      | 18,160   | −50%         |
| migrations             | 57,696      | 39,624   | −31%         |
| privacy-pool           | 165,064     | 130,512  | −21%         |
| profile                | 25,992      | 17,432   | −33%         |
| role-registry          | 32,768      | 22,784   | −31%         |
| validation             | 26,064      | 17,480   | −33%         |
| optional-accounts      | 24,768      | 15,824   | −36%         |
| todo                   | 22,536      | 13,856   | −39%         |
| account-realloc        | 29,720      | 20,848   | −30%         |
| compact-accounts       | 42,840      | 33,528   | −22%         |
| remaining ten programs |             |          | −7…−56% each |

Across the twenty-two programs converted in the second pass the aggregate is 551,208 → 397,912 bytes (−27.8%), with per-program cuts from −7.5% (custom-errors, already minimal) to −49.6% (counter) and −53.6% (float-accounts).

Runtime compute units were verified on escrow with Surfpool, three runs each, fully deterministic: Make 29,535 → 29,105 CU and Take 32,230 → 31,658 CU, and the framework-comparison fixtures re-measured byte- and CU-identical after the creation-builder dedup below (counter `initialize` 3,295 CU). Fat LTO does not trade compute units for size here; it removes them, because the single codegen unit lets inlining collapse cross-crate glue that the unoptimized link kept as call sequences. Every entrypoint frame stays within the 4 KB stack limit after the switch (deepest: vesting at 3,992; multisig reaches exactly 4,096 — no offset exceeds it, its manifest documents the re-check rule, and its full e2e + Surfpool suites pass against the LTO ELF).

The PDA-creation builders share one allocation spine (`CompactCreationTarget::allocate_zeroed`, `PdaCreationTarget::allocate`), so a program that creates several account types pays the seed marshalling, signer assembly, and rent computation once instead of once per generic instantiation — the multisig example keeps 5,252 bytes this way. Each spine keeps the shape its users measured best with: the compact-creation spine stays a real outlined call, which is what collapses multisig's three instantiations into one shared function, while the PDA-creation spine is `#[inline(always)]`, because a single-instantiation program has no duplicate to collapse and pays only the call boundary — outlining it measured +80 CU on the counter fixture's `initialize`.

## Bound the entrypoint account budget

`nostd_entrypoint!` accepts a second argument: the maximum number of accounts the entrypoint deserializes (the default is `pinocchio::MAX_TX_ACCOUNTS`, 255). Pinocchio's deserializer unrolls the account walk at compile time, so a program compiled with the default carries walking code for 255 accounts even when every instruction uses two. Passing the program's real bound — its widest instruction's account count plus headroom — removes that code:

| Fixture                                               | Default budget | Bounded budget | Δ size | Δ CU               |
| ----------------------------------------------------- | -------------: | -------------: | ------ | ------------------ |
| hello (`nostd_entrypoint!(process_instruction, 1)`)   |          4,680 |          2,736 | −41.5% | 145 → 151          |
| counter (`nostd_entrypoint!(process_instruction, 3)`) |         11,400 |          9,976 | −12.5% | 3,203 → 3,202 / +4 |

The budget is a program-level contract: accounts beyond the bound are ignored rather than rejected, so a program that accepts unbounded remaining accounts must not lower it. [ADR 0010](./adrs/0010-lean-entrypoint-strategy.md) measures this lever and builds the case for the lean dispatcher on top of it.

## Prefer exclusive slice bounds in seed and signer assembly

Every `[..=len]` slice over a seed or signer array monomorphizes its own 216-byte `RangeInclusive<usize>::index` copy plus panic plumbing; the equivalent exclusive `[..len + 1]` inlines to a few instructions. Pina's PDA-creation CPI spine carried three of them — the derivation-seed slice, the combined-seed signer slice, and the signer-list slice — so every program that creates a PDA paid ~1.3 KB of deployed size for them. They now use exclusive bounds (each guarded by the `len < MAX` check that already ran), which measured −1,320 bytes and −92 compute units on the counter fixture's `initialize` with byte-identical behavior. Generated code and user code should follow the same shape: exclusive ranges over arrays whose filled prefix is `len + 1`.

[`DETAIL_POINTER_MESSAGE`]: https://docs.rs/pina/latest/pina/constant.DETAIL_POINTER_MESSAGE.html
