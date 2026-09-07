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
