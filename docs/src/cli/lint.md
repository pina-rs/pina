# `pina lint`

Run Pina's official security lint set against the discovered program.

The lints live in the `pina_lints` crate, which is published to crates.io and statically compiled into the `pina_lint_driver` binary, a `rustc` wrapper. No external lint tooling is downloaded, no precompiled lint bundles exist, and the project itself never supplies or configures lint libraries.

## Synopsis

```text
pina lint [OPTIONS]
```

| Input                 | Default | Meaning                                                       |
| --------------------- | ------- | ------------------------------------------------------------- |
| `-p, --project <DIR>` | `.`     | Directory inside the Pina or Cargo project to discover.       |
| `--fix`               | off     | Apply machine-applicable suggestions, then rerun diagnostics. |

## Examples

```bash
pina lint
pina lint --fix
pina lint --project ./programs/counter
```

`pina lint` discovers the nearest `pina.toml` or unambiguous Cargo package. It checks only that program package and does not lint workspace dependencies.

## Prebuilt lint driver

`pina lint` runs `cargo check` — or `cargo fix` with `--fix` — with the driver as `RUSTC_WORKSPACE_WRAPPER`. Cargo calls the driver with the arguments it would have passed to `rustc`; the driver registers every lint statically linked into it, and compilation continues normally with the lints emitted as ordinary compiler diagnostics. If `RUSTC_WRAPPER` already selects a compiler cache such as `sccache`, Cargo preserves it as the outer wrapper.

The lint code is ordinary Rust compiled into a `pina_lint_driver` binary that ships prebuilt next to the `pina` CLI itself: every release archive contains both binaries side by side, and the npm platform packages carry them into their `bin/` directory. The CLI never builds or installs anything — it runs the driver beside its own executable.

- The driver is built in Pina's release pipeline with the pinned nightly toolchain from `rust-toolchain.toml`, the same release `pina init` scaffolds into new projects, because `pina_lints` is nightly-only: its lint passes link against the compiler's unstable `rustc_private` crates. A CLI installed with `cargo install pina_cli` does not include the driver; install the prebuilt CLI or point `PINA_LINT_DRIVER_PATH` at a locally built driver.
- The driver loads the active toolchain's `librustc_driver` at runtime. The CLI prepends the active sysroot's library directory to the dynamic-library search path — `DYLD_LIBRARY_PATH` and `LD_LIBRARY_PATH` on macOS, `LD_LIBRARY_PATH` on other Unix, `PATH` on Windows — for the lint run, so the driver loads regardless of how the toolchain was installed. Because the compiler internals are keyed to the exact nightly build, the project's active toolchain must be the pinned nightly release; anything else fails the driver load with an error naming the required toolchain.
- The bundled driver is started once before the lint run to confirm it loads against the active toolchain. When it does not, the error names the required nightly and the `PINA_LINT_DRIVER_PATH` escape hatch instead of failing deep inside cargo with a loader exit.
- `CARGO_TARGET_DIR` continues to control normal project build artifacts.

To run a driver you built yourself — typically the workspace driver while developing a lint — set `PINA_LINT_DRIVER_PATH` to an executable binary path and `pina lint` uses it without further checks. The repository's own `security:pina-lint` task uses this variable to run the workspace-built driver.

## Driver environment variables

The driver reads a few environment variables:

| Variable            | Meaning                                                        |
| ------------------- | -------------------------------------------------------------- |
| `PINA_LINT_NO_DEPS` | Set to `1` to lint only the primary package, not dependencies. |
| `PINA_LINT_LEVELS`  | Comma-separated `lint=level` (allow/warn/deny) overrides.      |
| `PINA_LINT_ONLY`    | Restrict linting to a single named lint.                       |
| `PINA_LINT_LIST`    | Print the lint catalog instead of compiling.                   |

`pina lint` sets `PINA_LINT_NO_DEPS` and forwards `PINA_LINT_LEVELS` from the project's `[lints]` table. `PINA_LINT_NO_DEPS`, `PINA_LINT_LEVELS`, and `PINA_LINT_ONLY` are recorded in dep-info, so changing them invalidates cargo's cached check results.

## Configuring lint levels

Lint levels are configured in the project's `pina.toml` under the `[lints]` table. Each entry maps a lint name to `allow`, `warn`, or `deny`; lints that are not listed keep their built-in default level.

```toml
[lints]
deny_heap_allocations_in_onchain_instruction_handlers = "deny"
require_explicit_discriminators_and_seed_namespaces = "allow"
```

Unknown lint names are rejected with the list of known lints. Deny-level security lints should not be disabled at crate scope; when a finding is a false positive, scope an `#[allow(...)]` to the smallest item and document the invariant.

## Fix mode

```bash
pina lint --fix
git diff
```

With `--fix`, `pina lint` runs `cargo fix` instead of `cargo check`. Pina supplies `--allow-dirty`, `--allow-staged`, and `--allow-no-vcs` because requesting `--fix` is explicit permission to edit the current working tree, including a newly initialized project that has not entered version control yet. Only diagnostics carrying machine-applicable suggestions can be changed automatically; findings without a safe rewrite remain diagnostics. Always inspect and test the resulting diff.

## Security boundary

The lint driver is native executable code, not a passive rule file: it links against the compiler's unstable internals and runs with the same local permissions as the invoking user. Pina therefore ships the driver prebuilt next to the CLI from the same attested release pipeline, never loads lint libraries from project metadata, and starts the bundled driver once before the lint run so a toolchain mismatch fails fast with a clear error. Obtain the CLI from a trusted channel and review CLI upgrades as executable tooling changes.

`PINA_LINT_DRIVER_PATH` executes whatever binary it names, so point it only at a driver you built yourself.

## Exit behavior

The command exits successfully only when the prebuilt driver resolves and loads, compilation finishes, and all enabled security lints succeed. A diagnostic at an error level, a compilation failure, a missing bundled driver, a toolchain mismatch, or an invalid `PINA_LINT_DRIVER_PATH` produces a non-zero exit. Child Cargo output stays attached to the terminal; Pina prints a short completion summary only after success.
