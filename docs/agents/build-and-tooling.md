# Build and Tooling

## Environment setup

Use `devenv` for the reproducible development environment.

```sh
devenv shell
install:all
```

- Cargo binaries are managed via `cargo-run-bin` and pinned in `[workspace.metadata.bin]` in `Cargo.toml` (`cargo-expand` is the one exception; devenv installs it directly with `cargo install`).
- External binaries such as the Solana CLI/agave and surfpool are nix packages from the `ifiokjr-nixpkgs` flake input (`custom.agave`, `custom.surfpool` in `devenv.nix`).
- Snapshot, coverage, and test-runner binaries (`cargo-insta`, `cargo-llvm-cov`, `cargo-nextest`) are also nix packages from devenv.
- Kani is provided by the pinned `ifiokjr/nixpkgs` input through the dedicated devenv `kani` profile. Devenv links Kani's matching nightly Rust toolchain into the package, so proof tasks need no setup command or mutable `KANI_HOME`, while unrelated commands avoid realizing the large verifier closure.

## Common commands

### Build

```sh
cargo build --all-features
cargo build-escrow-program
build:pina:no-default
```

### Test

```sh
cargo test
cargo nextest run
cargo test -p pina
cargo test -p pina -- test_name
devenv --profile kani shell -- test:kani
devenv --profile kani shell -- test:kani:quick
devenv --profile kani shell -- test:kani:compact
```

### Lint and format

```sh
lint:all
lint:clippy
lint:format
fix:all
fix:clippy
fix:format
```

### Documentation

```sh
docs:sync
docs:check
verify:docs
```

### Coverage and semver

```sh
cargo llvm-cov
cargo semver-checks
```

## CI

Pull requests run a fast, high-confidence subset of CI; pushes to `main` run the full suite after merge. Jobs are gated on changed paths, and the slowest verification tiers only run on `main`:

- `kani (compact layouts)`, `program-e2e`, and the dedicated `build` job run post-merge only.
- `coverage` runs post-merge only.
- Everything else runs per pull request when the changed paths can affect it (for example, `surfpool` runs only for example or crate changes, and `zizmor` only for workflow changes).

Add the `ci-full` label to a pull request to force every tier, including the post-merge-only jobs, before merging. The `release` label keeps the benchmark jobs and the publish dry-run from running on release pull requests.

Path gating uses job-level conditions rather than workflow-level `paths:` filters because the jobs are required status checks: a skipped job reports as success, while a workflow that never runs leaves the check in `Expected` and blocks merging.

## Formatting

- Use `dprint` for formatting.
- Do not run `rustfmt` directly.
- Preferred commands:
  - `fix:format`
  - `dprint fmt`

`fix:format` also re-syncs mdt-managed docs.

## Style rules

- Hard tabs
- Max width: 100
- One import per line
- `imports_granularity = "Item"`
- Imports grouped by `StdExternalCrate`

## Useful aliases

Nix-provided binaries: `cargo-insta`, `cargo-llvm-cov`, `cargo-nextest`.

Defined in `.cargo/config.toml`:

- `cargo build-bpf` — SBF build for the `pina_bpf` crate
- `cargo build-escrow-program` (and the other `build-*-program` example aliases) — `cargo build-sbf` per example
- `cargo semver-checks`
- `cargo workspaces`
- `cargo kani`

## Notes

When using `devenv`, `pina ...` is available as a shortcut for:

```sh
cargo run -p pina_cli -- ...
```

Reusable docs providers live in `templates/*.t.md`.
