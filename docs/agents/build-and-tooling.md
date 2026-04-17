# Build and Tooling

## Environment setup

Use `devenv` for the reproducible development environment.

```sh
devenv shell
install:all
```

- Cargo binaries are managed via `cargo-run-bin` and pinned in `[workspace.metadata.bin]` in `Cargo.toml`.
- External binaries such as Solana CLI/agave and surfpool are managed via `eget` with config in `.eget/.eget.toml`.

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

Defined in `.cargo/config.toml`:

- `cargo dylint`
- `cargo insta`
- `cargo llvm-cov`
- `cargo nextest`
- `cargo semver-checks`
- `cargo workspaces`

## Notes

When using `devenv`, `pina ...` is available as a shortcut for:

```sh
cargo run -p pina_cli -- ...
```

Reusable docs providers live in `templates/*.t.md`.
