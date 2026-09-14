# Program size

Deployed program size determines rent, and rent is paid in SOL. A Pina program is built to be small: the framework's own overhead above a hand-written `pinocchio` program is a few hundred bytes once link-time optimization runs.

## Framework comparison

Same toolchain for every row: `cargo-build-sbf` (Agave 4.2.2), `sbpf-solana-solana` target, equivalent program semantics — a single-instruction hello world, and a PDA counter with `initialize`/`increment`.

| Framework                   | Hello world | Counter    |
| --------------------------- | ----------- | ---------- |
| Quasar                      | 2,520       | 7,808      |
| Pinocchio (hand-written)    | 3,160       | 6,512      |
| **Pina**                    | **4,680**   | **12,688** |
| Anchor v2 (`lang-v2`, rc.1) | 1,880       | 8,696      |
| Anchor (v1, 1.2.0)          | 55,752      | 122,160    |

[Framework comparison](./framework-comparison.md) holds the generated version of this table together with the compute units each instruction consumes, and `benchmark:frameworks` rebuilds and rewrites it. The headline: a v1 Anchor program is more than ten times the size of any of the others, and LTO makes it _larger_ rather than smaller.

Pina's remaining gap to Pinocchio is mostly framework surface: derive-generated validation and dispatch, plus the entrypoint wrapper. With no derive and no logs the framework floor is 3,352 bytes against Pinocchio's 3,160 — 192 bytes.

## What determines the size

Four decisions account for nearly all of it. `pina build` applies the first three automatically.

### 1. Link-time optimization

`lto = "fat"` with `codegen-units = 1` is worth 20-35% on its own.

A crate that declares `crate-type = ["cdylib", "lib"]` cannot be linked with LTO: rustc rejects `-C lto` when one invocation also emits an rlib, and the SBF toolchain silently drops the profile setting. Programs are therefore `crate-type = ["cdylib"]` only. See [Testing](#testing-a-cdylib-program) for how tests still reach the real code.

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

[`DETAIL_POINTER_MESSAGE`]: https://docs.rs/pina/latest/pina/constant.DETAIL_POINTER_MESSAGE.html
