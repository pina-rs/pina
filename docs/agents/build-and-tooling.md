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
- Kani is provided by the pinned `ifiokjr/nixpkgs` input through the dedicated devenv `kani` profile. Devenv links Kani's matching nightly Rust toolchain into the package, so proof tasks need no setup command or mutable `KANI_HOME`, while unrelated commands avoid realizing the large verifier closure. CI does not use that profile: the `kani (quick)` and `kani (compact layouts)` jobs install Kani through `model-checking/kani-github-action`, because the proofs need nothing else from devenv and the nix setup costs roughly five minutes per job. Kani's version is pinned in `.github/workflows/ci.yml`, so raise it there and in the nixpkgs input together.

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

GitHub Actions workflows do **not** run Nix or `devenv` on runners. Instead, jobs compose lightweight, pinned setup actions and delegate shared build/test/coverage orchestration to `scripts/ci-tasks.ts` (which `devenv.nix` also delegates to so local `devenv shell` tasks stay in sync with CI):

- `.github/actions/setup-rust` — installs the minimal pinned Rust toolchain, configures `mold` and `CARGO_PROFILE_DEV_DEBUG=0`, enables S3-compatible `sccache` when configured, and falls back to `Swatinem/rust-cache` with shared keys and `main`-only writes.
- `.github/actions/install-tools` — installs only the pinned prebuilt CLI binaries a job needs (`dprint`, `shfmt`, `mdbook`, `zizmor`, `cargo-deny`, `cargo-audit`, `cargo-llvm-cov`, `monochange`, `mdt`, `nixfmt`, `cargo-fuzz`, `surfpool`, `sbpf-linker`, cached `cargo-expand`).
- `.github/actions/setup-sbf` — caches pinned Agave SBF binaries and preinstalled `platform-tools` (`v1.54`) so SBF jobs skip `--force-tools-install`.

To offload Rust compilation caching from GitHub's 10 GB cache limit to an S3-compatible bucket (Backblaze B2, Cloudflare R2, or AWS S3), configure the repository variables `SCCACHE_BUCKET`, `SCCACHE_ENDPOINT`, and `SCCACHE_REGION` alongside the repository secrets `SCCACHE_ACCESS_KEY_ID` and `SCCACHE_SECRET_ACCESS_KEY`. When present, `setup-rust` automatically activates `sccache` and shrinks `Swatinem/rust-cache` to crates.io registry downloads only.

Pull requests run a fast, high-confidence subset of CI; pushes to `main` run the full suite after merge. Jobs are gated on changed paths, and the slowest verification tiers only run on `main`:

- `kani (compact layouts)`, `program-e2e`, `test`, and the dedicated `build` job run post-merge only.
- `coverage` runs on pull requests as the Rust test and patch-coverage gate.
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

- `cargo build-pina-bpf-program` — SBF build for the `pina_bpf_program` crate
- `cargo build-escrow-program` (and the other `build-*-program` example aliases) — `cargo build-sbf` per example
- `cargo semver-checks`
- `cargo workspaces`
- `cargo kani`

## Notes

When using `devenv`, `pina ...` is available as a shortcut for:

```sh
cargo run -p pina_cli -- ...
```

Reusable docs providers live in `templates/*.t.md`. They are consumed by `docs/src/**`, readmes, and CLI scaffold templates through `{=blockName}` / `{/blockName}` markers.

`mdt.toml` configures the `[[formatters]]` integration, which runs each consumer through `dprint fmt --stdin`. Consumers are therefore stored dprint-canonical, while producers stay hand-formatted because dprint excludes `**/*.t.md`. Without the integration, a table in a provider and its consumer disagree on padding, and `mdt update` and `dprint fmt` overwrite each other until `docs:check` fails on a file that is already correctly formatted. Edit the provider with ordinary markdown and run `docs:sync`; there is no need to hand-copy a padded table out of a consumer.
