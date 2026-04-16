# AGENTS.md

Pina is a Rust workspace for building performant, `no_std` Solana programs on top of `pinocchio`.

## Repo defaults

- Use `devenv` for the development shell and repo task runner.
- Use `cargo` for workspace tasks; use `pnpm` only for JS/Codama subprojects.
- Format with `fix:format` or `dprint fmt`; do not run `rustfmt` directly.
- Workspace code must preserve `no_std` compatibility where applicable.
- `unsafe_code` and `unstable_features` are denied workspace-wide.

## Common commands

- `devenv shell` — enter the dev environment
- `install:all` — install pinned cargo binaries and external tools
- `cargo build --all-features` — build the workspace
- `cargo test` — run the default test suite
- `build:pina:no-default` — verify `pina` across no-default feature subsets
- `lint:all` — run clippy, formatting, and docs verification
- `verify:docs` — validate reusable docs and mdBook output
- `fix:format` — format files and re-sync mdt-managed docs

## Task-specific guidance

- [Build and tooling](./docs/agents/build-and-tooling.md)
- [Coding style guide](./docs/agents/coding-style.md) — visual organization, whitespace patterns, and code aesthetics
- [Workspace architecture](./docs/agents/workspace-architecture.md)
- [Testing and SBF builds](./docs/agents/testing-and-sbf.md)
- [Release process and changesets](./docs/agents/release-and-changesets.md)
- [Git workflow](./docs/agents/git-workflow.md)
- [Security and code constraints](./docs/agents/security-and-code-constraints.md)
