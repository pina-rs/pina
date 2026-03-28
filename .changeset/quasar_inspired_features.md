---
default: major
---

### Pod Arithmetic (pina_pod_primitives)

Add full Quasar-style arithmetic, bitwise, ordering, and display traits to all Pod integer types (`PodU16`, `PodU32`, `PodU64`, `PodU128`, `PodI16`, `PodI32`, `PodI64`, `PodI128`).

**Arithmetic operators** (`Add`, `Sub`, `Mul`, `Div`, `Rem`) work between Pod types and between Pod + native types. Assign variants (`AddAssign`, `SubAssign`, etc.) allow ergonomic in-place mutation like `my_account.count += 1u64;`.

**Arithmetic semantics**: debug builds panic on overflow (checked), release builds use wrapping for CU efficiency on Solana.

**Bitwise operators**: `BitAnd`, `BitOr`, `BitXor`, `Shl`, `Shr`, `Not` with assign variants.

**Signed types** get `Neg` for unary negation.

**Checked arithmetic**: `checked_add`, `checked_sub`, `checked_mul`, `checked_div` return `Option` for explicit overflow detection.

**Saturating arithmetic**: `saturating_add`, `saturating_sub`, `saturating_mul` clamp at bounds.

**Constants**: `ZERO`, `MIN`, `MAX` for all types.

**Helpers**: `get()` method, `is_zero()`, improved `Debug` (e.g. `PodU64(42)`), `Display`, `Ord`, `PartialOrd`, `PartialEq<native>`, `PartialOrd<native>`.

**PodBool**: `Not` operator and `Display` added.

**Backward compatible**: all existing APIs preserved, no breaking changes.

### IDL Parser Hardening (pina_cli)

Add static validation to the IDL parser that runs after IR assembly:

- **Discriminator collision detection**: checks within accounts and within instructions for duplicate discriminator values. Three-way collisions produce all pairwise diagnostics.
- **Duplicate input field detection**: checks within each instruction for name collisions between account names and argument names.
- **Human-readable error formatting** for both collision types.

Validation is automatically run during `assemble_program_ir()`.

### Static CU Profiler (pina profile)

Add a new `pina profile` CLI command for static compute unit profiling of compiled SBF programs.

- `pina profile <path-to-so>` — text summary with per-function CU estimates
- `pina profile <path-to-so> --json` — JSON output for CI integration
- `pina profile <path-to-so> --output report.json` — write to file

The profiler parses ELF binaries to extract `.text` sections and symbol tables, counts SBF instructions per function, and estimates CU costs without requiring a running validator. Works best with unstripped binaries.

v1 scope: text/JSON output, per-function breakdown, best-effort symbol resolution. Flamegraph/browser UI planned for v2.
