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

## Managed lint driver

`pina lint` runs `cargo check` — or `cargo fix` with `--fix` — with the driver as `RUSTC_WRAPPER`. Cargo calls the driver with the arguments it would have passed to `rustc`; the driver registers every lint statically linked into it, and compilation continues normally with the lints emitted as ordinary compiler diagnostics.

The CLI prepares the driver below Cargo home:

```text
$CARGO_HOME/pina/lint-driver/<pina-version>/<rustc-release>-<host>/bin/pina_lint_driver
```

- The first use installs it from crates.io with `cargo install --locked --root <that directory> --bin pina_lint_driver --version =<CLI version> pina_lints`. This step requires network access once; later runs reuse the cached binary.
- The driver release identity is the `pina_lints` version, which matches the installed CLI version. There is no separate tool release or revision selector to pick.
- The driver is built with the project's pinned nightly toolchain, the same compiler that builds the project, because `pina_lints` is nightly-only: its lint passes and the driver link against the compiler's unstable `rustc_private` crates. The toolchain must have the `rustc-dev` component installed (`rustup component add rustc-dev` if it is missing).
- Set `CARGO_HOME` to move the driver cache. `CARGO_TARGET_DIR` continues to control normal project build artifacts.

To run a driver you built yourself — typically the workspace driver while developing a lint — set `PINA_LINT_DRIVER_PATH` to an executable binary path and `pina lint` uses it instead of the managed cache. The repository's own `security:pina-lint` task uses this variable to run the workspace-built driver.

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

The lint driver is native executable code, not a passive rule file: it links against the compiler's unstable internals and runs with the same local permissions as the invoking user. Pina therefore pins the driver to the crates.io release of `pina_lints` matching the exact CLI version, installs it with `--locked` into a user-owned cache outside project-controlled target trees, and never loads lint libraries from project metadata. Obtain the CLI from a trusted channel and review CLI upgrades as executable tooling changes.

`PINA_LINT_DRIVER_PATH` executes whatever binary it names, so point it only at a driver you built yourself.

## Exit behavior

The command exits successfully only when driver preparation, compilation, and all enabled security lints succeed. A diagnostic at an error level, a compilation failure, a missing network connection on first install, or an invalid `PINA_LINT_DRIVER_PATH` produces a non-zero exit. Child Cargo output stays attached to the terminal; Pina prints a short completion summary only after success.
