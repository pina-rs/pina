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
| `--build-driver`      | off     | Build the lint driver with the active toolchain.              |
| `--explain <LINT>`    | —       | Print one lint's reference and exit without linting.          |

## Examples

```bash
pina lint
pina lint --fix
pina lint --project ./programs/counter
pina lint --explain require_zeroed_before_close
pina lint --build-driver
```

`pina lint` discovers the nearest `pina.toml` or unambiguous Cargo package. It checks only that program package and does not lint workspace dependencies.

## Toolchain negotiation

A lint driver links the compiler's unstable `rustc_private` crates, so a driver only loads against the **exact compiler revision it was built with**. Two dated nightlies that share a release line expose incompatible compiler libraries, and nothing can make a driver built for one load against another. Pina therefore resolves a driver for whatever toolchain the project activates, instead of requiring one pinned nightly.

`pina lint` reads the active toolchain from `rustc -vV` and tries, in order:

1. `PINA_LINT_DRIVER_PATH`, the escape hatch for a driver you built yourself.
2. A driver already cached for this CLI release and this exact compiler revision.
3. The driver bundled next to the CLI, when it loads against the active toolchain.
4. A download from the Pina release matching this CLI version.

Asking a candidate driver to start is the version check. A driver built for another compiler cannot load `librustc_driver`, so a driver that starts is by construction the right one. That keeps the negotiation honest — it tests the property that actually matters instead of trusting a version string — and it means a driver built for a different nightly is skipped rather than producing an opaque loader failure deep inside cargo.

The cache lives below the platform's per-user cache directory (`$XDG_CACHE_HOME` or `~/.cache` on Linux, `~/Library/Caches` on macOS, `%LOCALAPPDATA%` on Windows) under `pina/lint-driver/<pina-version>/<host>-<commit-hash>/`. Both the host triple and the full compiler commit hash are part of the path, so two nightlies installed side by side each keep their own driver and a cached driver is never reused for a compiler it was not built with. Set `PINA_LINT_CACHE_DIR` to relocate it.

A download is addressed by name: `pina-lint-driver-<host>-<commit-hash>`. Because a release builds its driver with the nightly that release pins, a project on any other nightly asks for a name the release does not publish and gets a clear miss. That is the correct outcome — no release can publish a driver for every nightly — and the CLI reports it with the remedy rather than handing cargo a binary it cannot load.

## Building the driver

```bash
pina lint --build-driver
```

Use this on a nightly Pina publishes no prebuilt driver for. The build compiles the `pina_lints` release matching this CLI version, so the driver always runs exactly the lint set the CLI ships, and it compiles with **your** active toolchain. It requires the `rustc-dev` and `rust-src` components:

```bash
rustup component add rustc-dev rust-src
```

Cargo installs the driver into a staging root, the CLI copies it into the cache, and the staging root is removed. Later runs resolve the cached driver without invoking cargo, so the build cost is paid once per toolchain.

## Diagnosing driver resolution

```bash
pina doctor
pina doctor --json
```

`pina lint` failing to find a driver is the one failure a user cannot debug from the message alone, so `pina doctor` reports the whole state: the active toolchain, the expected release, the resolved driver and how it was obtained, both search paths, and the one-line remedy. It reports without downloading, because a diagnostic that populates a cache cannot be run to find out what is wrong.

Every `pina lint` success line also names the driver that ran — `(bundled)`, `(cached)`, `(downloaded)`, `(built from source)`, or `(PINA_LINT_DRIVER_PATH)` — so a surprising result is traceable to the binary that produced it.

## How lints run

`pina lint` runs `cargo check` — or `cargo fix` with `--fix` — with the resolved driver as `RUSTC_WORKSPACE_WRAPPER`. Cargo calls the driver with the arguments it would have passed to `rustc`; the driver registers every lint statically linked into it, and compilation continues normally with the lints emitted as ordinary compiler diagnostics. If `RUSTC_WRAPPER` already selects a compiler cache such as `sccache`, Cargo preserves it as the outer wrapper.

The CLI prepends the active sysroot's library directory to the dynamic-library search path — `DYLD_LIBRARY_PATH` and `LD_LIBRARY_PATH` on macOS, `LD_LIBRARY_PATH` on other Unix, `PATH` on Windows — for the lint run, so the driver loads regardless of how the toolchain was installed. `CARGO_TARGET_DIR` continues to control normal project build artifacts.

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

Every lint's contract, rationale, and sanctioned blessing pattern is in the [Lint Reference](../lint-reference.md), and `pina lint --explain <LINT>` prints one entry without leaving the terminal. Start there rather than guessing at an `#[allow]`: each entry names the API or restructure that satisfies the lint.

## Fix mode

```bash
pina lint --fix
git diff
```

With `--fix`, `pina lint` runs `cargo fix` instead of `cargo check`. Pina supplies `--allow-dirty`, `--allow-staged`, and `--allow-no-vcs` because requesting `--fix` is explicit permission to edit the current working tree, including a newly initialized project that has not entered version control yet. Only diagnostics carrying machine-applicable suggestions can be changed automatically; findings without a safe rewrite remain diagnostics. Always inspect and test the resulting diff.

## Security boundary

The lint driver is native executable code, not a passive rule file: it links against the compiler's unstable internals and runs with the same local permissions as the invoking user. Pina therefore ships the driver prebuilt next to the CLI from the same attested release pipeline, never loads lint libraries from project metadata, and starts a candidate driver once before the lint run so a toolchain mismatch fails fast with a clear error. Every downloaded driver comes from the same release and the same assets that carry the CLI itself, so it is covered by the release's build provenance attestation.

Downloads only ever come from Pina's GitHub release for this CLI version. A project's manifest cannot redirect the driver source, and no project metadata selects a lint library, so linting an untrusted repository does not execute code that repository chose. `PINA_LINT_DRIVER_BASE_URL`, `PINA_LINT_DRIVER_REPO`, and `PINA_LINT_DRIVER_RELEASE` exist for the release pipeline and the CLI's own tests; treat setting them as equivalent to installing a driver yourself.

`PINA_LINT_DRIVER_PATH` executes whatever binary it names, so point it only at a driver you built yourself. `pina lint --build-driver` compiles from the matching `pina_lints` release on crates.io with your own toolchain.

## Exit behavior

The command exits successfully only when a driver resolves for the active toolchain, it loads, compilation finishes, and all enabled security lints succeed. Without `PINA_LINT_DRIVER_PATH` the run negotiates a driver and additionally fails when none can be obtained; a diagnostic at an error level, a compilation failure, or an invalid `PINA_LINT_DRIVER_PATH` produces a non-zero exit in every mode. When no driver matches, cargo never runs, so a toolchain problem cannot be mistaken for a lint failure. Child Cargo output stays attached to the terminal; Pina prints a short completion summary naming the driver it used only after success.
